import { describe, expect, it, vi } from "vitest";
import {
  announce,
  buildPayload,
  groupCommits,
  isDiscordWebhookUrl,
  parseCommit,
  parseTag,
  postWebhook,
  previousTag,
  renderSections,
} from "./release-notes.mjs";

const WEBHOOK = "https://discord.com/api/webhooks/123456789/abc-DEF_ghi";
const NOW = new Date("2026-11-01T12:00:00Z");

function response(status, body = {}) {
  return { ok: status >= 200 && status < 300, status, json: async () => body };
}

describe("parseCommit", () => {
  it("reads type, scope, breaking flag and description", () => {
    expect(parseCommit("feat(ui): add a tab")).toEqual({
      type: "feat",
      scope: "ui",
      breaking: false,
      description: "add a tab",
    });
    expect(parseCommit("fix!: drop the old format")).toMatchObject({
      type: "fix",
      scope: null,
      breaking: true,
    });
  });

  it("rejects subjects that are not conventional commits", () => {
    expect(parseCommit("Merge branch 'main'")).toBeNull();
    expect(parseCommit("feat:")).toBeNull();
    expect(parseCommit("")).toBeNull();
  });
});

describe("tags", () => {
  it("only accepts plain vX.Y.Z tags", () => {
    expect(parseTag("v0.1.7")).toEqual([0, 1, 7]);
    expect(parseTag("0.1.7")).toBeNull();
    expect(parseTag("v1.2")).toBeNull();
    expect(parseTag("v1.2.3-rc1")).toBeNull();
  });

  it("finds the previous release by version, not by listing order", () => {
    const tags = ["v0.1.10", "v0.1.2", "v0.1.9", "v0.2.0", "latest"];
    expect(previousTag(tags, "v0.1.10")).toBe("v0.1.9");
    expect(previousTag(tags, "v0.2.0")).toBe("v0.1.10");
  });

  it("has no previous release for the first one", () => {
    expect(previousTag(["v0.1.0"], "v0.1.0")).toBeNull();
    expect(previousTag([], "v0.1.0")).toBeNull();
    expect(previousTag(["v0.1.0"], "nope")).toBeNull();
  });
});

describe("groupCommits", () => {
  it("keeps features, fixes and performance, and drops internal commits", () => {
    const sections = groupCommits([
      "feat: calibration auto",
      "fix(ui): clips overlap",
      "perf: faster analysis",
      "chore: version 0.1.7",
      "docs: update readme",
      "test: more cases",
      "Merge pull request #3",
    ]);
    expect(sections.map((s) => s.title)).toEqual([
      "✨ Nouveautés",
      "🐛 Corrections",
      "⚡ Performances",
    ]);
    expect(sections[0]?.items).toEqual(["Calibration auto"]);
  });

  it("flags breaking changes", () => {
    const [section] = groupCommits(["feat!: new storage layout"]);
    expect(section?.items[0]).toBe("**Changement important** : New storage layout");
  });

  it("is empty when nothing is user-facing", () => {
    expect(groupCommits(["chore: bump", "ci: tweak"])).toEqual([]);
  });
});

describe("renderSections", () => {
  it("renders titled bullet lists separated by a blank line", () => {
    const text = renderSections([
      { title: "✨ Nouveautés", items: ["A", "B"] },
      { title: "🐛 Corrections", items: ["C"] },
    ]);
    expect(text).toBe("**✨ Nouveautés**\n• A\n• B\n\n**🐛 Corrections**\n• C");
  });

  it("cuts on a line boundary and says how many entries are hidden", () => {
    const items = Array.from({ length: 50 }, (_, i) => `Entrée numéro ${i} ${"x".repeat(40)}`);
    const text = renderSections([{ title: "T", items }], 500);
    expect(text.length).toBeLessThanOrEqual(500);
    expect(text).toMatch(/… et \d+ autres \(voir la release\)$/);
    expect(text.split("\n").every((l) => l.length <= 100)).toBe(true);
  });
});

describe("buildPayload", () => {
  const base = {
    tag: "v0.2.0",
    repo: "Petitclg/lol-game-review",
    serverUrl: "https://github.com",
    date: NOW,
  };

  it("links to the release and never allows mentions", () => {
    const p = buildPayload({ ...base, description: "@everyone **Nouveautés**" });
    expect(p.allowed_mentions).toEqual({ parse: [] });
    expect(p.embeds).toHaveLength(1);
    expect(p.embeds[0]).toMatchObject({
      title: "LoL Game Review v0.2.0",
      url: "https://github.com/Petitclg/lol-game-review/releases/tag/v0.2.0",
      timestamp: "2026-11-01T12:00:00.000Z",
    });
  });

  it("explains when a release has no user-facing change", () => {
    const p = buildPayload({ ...base, description: "  " });
    expect(p.embeds[0]?.description).toMatch(/internes/);
  });

  it("never exceeds the embed description limit", () => {
    const long = Array.from({ length: 400 }, (_, i) => `ligne ${i} ${"y".repeat(30)}`).join("\n");
    const p = buildPayload({ ...base, description: long });
    expect(p.embeds[0]?.description.length).toBeLessThanOrEqual(4096);
  });
});

describe("isDiscordWebhookUrl", () => {
  it("accepts Discord webhook URLs only", () => {
    expect(isDiscordWebhookUrl(WEBHOOK)).toBe(true);
    expect(isDiscordWebhookUrl("https://ptb.discord.com/api/webhooks/1/a_b")).toBe(true);
    expect(isDiscordWebhookUrl("http://discord.com/api/webhooks/1/a")).toBe(false);
    expect(isDiscordWebhookUrl("https://evil.example/api/webhooks/1/a")).toBe(false);
    expect(isDiscordWebhookUrl("https://discord.com.evil.example/api/webhooks/1/a")).toBe(false);
    expect(isDiscordWebhookUrl("https://discord.com/api/webhooks/1/a?x=1")).toBe(false);
    expect(isDiscordWebhookUrl("")).toBe(false);
  });
});

describe("postWebhook", () => {
  it("posts the JSON payload and waits for the confirmation", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(response(200));
    await postWebhook(WEBHOOK, { a: 1 }, { fetchImpl });
    expect(fetchImpl).toHaveBeenCalledTimes(1);
    const [url, init] = fetchImpl.mock.calls[0];
    expect(url).toBe(`${WEBHOOK}?wait=true`);
    expect(init.method).toBe("POST");
    expect(init.headers["Content-Type"]).toBe("application/json");
    expect(JSON.parse(init.body)).toEqual({ a: 1 });
  });

  it("waits for retry_after when rate limited, then succeeds", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(response(429, { retry_after: 1.5 }))
      .mockResolvedValueOnce(response(200));
    const sleep = vi.fn().mockResolvedValue(undefined);
    await postWebhook(WEBHOOK, {}, { fetchImpl, sleep });
    expect(sleep).toHaveBeenCalledWith(1600);
    expect(fetchImpl).toHaveBeenCalledTimes(2);
  });

  it("retries server errors with a growing delay", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(response(503))
      .mockResolvedValueOnce(response(502))
      .mockResolvedValueOnce(response(200));
    const sleep = vi.fn().mockResolvedValue(undefined);
    await postWebhook(WEBHOOK, {}, { fetchImpl, sleep });
    expect(sleep.mock.calls.map((c) => c[0])).toEqual([2000, 4000]);
  });

  it("gives up after the last attempt", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(response(500));
    const sleep = vi.fn().mockResolvedValue(undefined);
    await expect(postWebhook(WEBHOOK, {}, { fetchImpl, sleep, attempts: 3 })).rejects.toThrow(
      "HTTP 500",
    );
    expect(fetchImpl).toHaveBeenCalledTimes(3);
  });

  it("does not retry a refusal such as an unknown webhook", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(response(404));
    await expect(postWebhook(WEBHOOK, {}, { fetchImpl })).rejects.toThrow("HTTP 404");
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });

  it("never leaks the webhook URL in an error", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(response(401));
    const error = await postWebhook(WEBHOOK, {}, { fetchImpl }).catch((e) => e);
    expect(String(error.message)).not.toContain("abc-DEF_ghi");
  });
});

describe("announce", () => {
  function deps(overrides = {}) {
    const calls = [];
    return {
      calls,
      tag: "v0.1.8",
      env: { DISCORD_WEBHOOK_URL: WEBHOOK, GITHUB_REPOSITORY: "Petitclg/lol-game-review" },
      git: (args) => {
        calls.push(args);
        if (args[0] === "tag") return "v0.1.6\nv0.1.7\nv0.1.8\n";
        return "feat: détection du flash\nfix: clips décalés\nchore: version 0.1.8\n";
      },
      readFile: () => null,
      log: vi.fn(),
      fetchImpl: vi.fn().mockResolvedValue(response(204)),
      sleep: vi.fn(),
      now: () => NOW,
      ...overrides,
    };
  }

  it("lists the commits since the previous release and posts them", async () => {
    const d = deps();
    const result = await announce(d);
    expect(result.posted).toBe(true);
    expect(d.calls[1]).toEqual(["log", "--no-merges", "--format=%s", "v0.1.7..v0.1.8"]);
    const body = JSON.parse(d.fetchImpl.mock.calls[0][1].body);
    expect(body.embeds[0].description).toContain("• Détection du flash");
    expect(body.embeds[0].description).toContain("• Clips décalés");
    expect(body.embeds[0].description).not.toContain("version 0.1.8");
  });

  it("uses the hand-written notes of the release when they exist", async () => {
    const d = deps({
      readFile: (path) =>
        path === "docs/release-notes/v0.1.8.md" ? "**Le Flash est détecté !**" : null,
    });
    await announce(d);
    const body = JSON.parse(d.fetchImpl.mock.calls[0][1].body);
    expect(body.embeds[0].description).toBe("**Le Flash est détecté !**");
    expect(d.calls).toEqual([]);
  });

  it("covers the whole history for the first release", async () => {
    const d = deps({
      tag: "v0.1.0",
      git: (args) => (args[0] === "tag" ? "v0.1.0\n" : "feat: premier jet\n"),
    });
    const calls = [];
    const git = d.git;
    d.git = (args) => {
      calls.push(args);
      return git(args);
    };
    await announce(d);
    expect(calls[1]).toEqual(["log", "--no-merges", "--format=%s", "v0.1.0"]);
  });

  it("skips quietly when no webhook is configured", async () => {
    const d = deps({ env: {} });
    const result = await announce(d);
    expect(result.posted).toBe(false);
    expect(d.fetchImpl).not.toHaveBeenCalled();
    expect(d.log).toHaveBeenCalledWith(expect.stringContaining("ignorée"));
  });

  it("refuses a webhook that is not a Discord URL", async () => {
    const d = deps({ env: { DISCORD_WEBHOOK_URL: "https://evil.example/hook" } });
    await expect(announce(d)).rejects.toThrow("webhook Discord valide");
    expect(d.fetchImpl).not.toHaveBeenCalled();
  });

  it("refuses an invalid tag", async () => {
    await expect(announce(deps({ tag: "main" }))).rejects.toThrow("Tag de release invalide");
  });

  it("prints the payload and posts nothing in dry-run mode, even without a webhook", async () => {
    const d = deps({ env: {}, dryRun: true });
    const result = await announce(d);
    expect(result.posted).toBe(false);
    expect(d.fetchImpl).not.toHaveBeenCalled();
    expect(d.log).toHaveBeenCalledWith(expect.stringContaining('"embeds"'));
  });
});
