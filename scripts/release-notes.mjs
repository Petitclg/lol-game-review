// Release announcement for Discord: builds release notes from the commits of a release
// and posts them to a webhook. Pure functions are exported for tests; the network, git
// and file system are injected into `announce`.

const SECTIONS = [
  { types: ["feat"], title: "✨ Nouveautés" },
  { types: ["fix"], title: "🐛 Corrections" },
  { types: ["perf"], title: "⚡ Performances" },
];

const COMMIT_RE =
  /^(?<type>[a-z]+)(?:\((?<scope>[^)]+)\))?(?<breaking>!)?:\s*(?<description>\S.*)$/;
const TAG_RE = /^v(\d+)\.(\d+)\.(\d+)$/;
const WEBHOOK_RE =
  /^https:\/\/(?:(?:canary|ptb)\.)?discord(?:app)?\.com\/api\/webhooks\/\d+\/[\w-]+$/;

const EMBED_DESCRIPTION_MAX = 4096;
const EMBED_TITLE_MAX = 256;
const COLOR = 0x5865f2;

/** @returns {{type: string, scope: string | null, breaking: boolean, description: string} | null} */
export function parseCommit(subject) {
  const m = COMMIT_RE.exec(subject.trim());
  if (!m?.groups) return null;
  return {
    type: m.groups.type,
    scope: m.groups.scope ?? null,
    breaking: m.groups.breaking === "!",
    description: m.groups.description.trim(),
  };
}

/** @returns {[number, number, number] | null} */
export function parseTag(tag) {
  const m = TAG_RE.exec(tag);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

function compareVersions(a, b) {
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

/** The greatest release tag lower than `current`, or `null` for the first release. */
export function previousTag(tags, current) {
  const cur = parseTag(current);
  if (!cur) return null;
  const older = tags
    .map((t) => ({ tag: t, version: parseTag(t) }))
    .filter((t) => t.version && compareVersions(t.version, cur) < 0)
    .sort((x, y) => compareVersions(y.version, x.version));
  return older[0]?.tag ?? null;
}

function capitalize(text) {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/**
 * Groups commit subjects into the user-facing sections. Other kinds (chore, docs, test,
 * ci, refactor...) and subjects that are not conventional commits are left out.
 */
export function groupCommits(subjects) {
  const sections = SECTIONS.map((s) => ({ title: s.title, items: [] }));
  for (const subject of subjects) {
    const commit = parseCommit(subject);
    if (!commit) continue;
    const index = SECTIONS.findIndex((s) => s.types.includes(commit.type));
    if (index < 0) continue;
    const text = capitalize(commit.description);
    sections[index].items.push(commit.breaking ? `**Changement important** : ${text}` : text);
  }
  return sections.filter((s) => s.items.length > 0);
}

/** Markdown of the sections, cut at `max` characters on a line boundary. */
export function renderSections(sections, max = EMBED_DESCRIPTION_MAX) {
  const lines = [];
  for (const section of sections) {
    if (lines.length > 0) lines.push("");
    lines.push(`**${section.title}**`);
    for (const item of section.items) lines.push(`• ${item}`);
  }
  return truncateLines(lines, max);
}

function truncateLines(lines, max) {
  const full = lines.join("\n");
  if (full.length <= max) return full;
  const kept = [];
  let used = 0;
  for (const line of lines) {
    const next = used + line.length + 1;
    if (next > max - 40) break;
    kept.push(line);
    used = next;
  }
  const hidden = lines.length - kept.length;
  return `${kept.join("\n")}\n… et ${hidden} autre${hidden > 1 ? "s" : ""} (voir la release)`;
}

const NO_USER_FACING_CHANGE = "Mises à jour internes et corrections mineures.";

/**
 * Discord webhook payload for one release.
 * `description` is the notes text (curated by hand, or generated from the commits).
 */
export function buildPayload({ tag, repo, serverUrl, description, date }) {
  const body = description.trim() || NO_USER_FACING_CHANGE;
  return {
    username: "LoL Game Review",
    // Commit messages are not trusted: they must never ping @everyone or roles.
    allowed_mentions: { parse: [] },
    embeds: [
      {
        title: `LoL Game Review ${tag}`.slice(0, EMBED_TITLE_MAX),
        url: `${serverUrl}/${repo}/releases/tag/${tag}`,
        description: truncateLines(body.split("\n"), EMBED_DESCRIPTION_MAX),
        color: COLOR,
        footer: { text: "L'application se met à jour automatiquement au démarrage." },
        timestamp: date.toISOString(),
      },
    ],
  };
}

export function isDiscordWebhookUrl(url) {
  return WEBHOOK_RE.test(url);
}

const defaultSleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Posts to the webhook. Retries on rate limiting (honouring `retry_after`) and on server
 * errors; any other refusal is final. The URL is a secret and never appears in errors.
 */
export async function postWebhook(url, payload, options = {}) {
  const { fetchImpl = fetch, sleep = defaultSleep, attempts = 4 } = options;
  let lastStatus = 0;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const response = await fetchImpl(`${url}?wait=true`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (response.ok) return;
    lastStatus = response.status;
    const retryable = response.status === 429 || response.status >= 500;
    if (!retryable || attempt === attempts) break;
    await sleep(await retryDelayMs(response, attempt));
  }
  throw new Error(`Discord a refusé le message (HTTP ${lastStatus}).`);
}

async function retryDelayMs(response, attempt) {
  if (response.status === 429) {
    try {
      const { retry_after: seconds } = await response.json();
      if (typeof seconds === "number") return Math.ceil(seconds * 1000) + 100;
    } catch {
      // fall through to the default backoff
    }
  }
  return 1000 * 2 ** attempt;
}

/** Hand-written notes of a release, read from the tag's own tree; empty when absent. */
function curatedNotes(git, tag) {
  try {
    return git(["show", `${tag}:docs/release-notes/${tag}.md`]).trim();
  } catch {
    return "";
  }
}

/**
 * Builds and posts the announcement of `tag`.
 * @param {object} deps
 * @param {string} deps.tag release tag, e.g. `v0.2.0`
 * @param {Record<string, string | undefined>} deps.env
 * @param {(args: string[]) => string} deps.git runs git and returns its output
 * @param {boolean} [deps.dryRun] print the payload instead of posting it
 */
export async function announce({
  tag,
  env,
  git,
  dryRun = false,
  log = console.log,
  fetchImpl,
  sleep,
  now = () => new Date(),
}) {
  if (!parseTag(tag)) throw new Error(`Tag de release invalide : ${tag}`);
  const webhook = env.DISCORD_WEBHOOK_URL?.trim();
  if (!dryRun) {
    if (!webhook) {
      log("DISCORD_WEBHOOK_URL n'est pas défini : annonce ignorée.");
      return { posted: false };
    }
    if (!isDiscordWebhookUrl(webhook)) {
      throw new Error("DISCORD_WEBHOOK_URL n'est pas une URL de webhook Discord valide.");
    }
  }

  let description = curatedNotes(git, tag);
  if (!description) {
    const tags = git(["tag", "--list", "v*"]).split("\n").filter(Boolean);
    const previous = previousTag(tags, tag);
    const range = previous ? `${previous}..${tag}` : tag;
    const subjects = git(["log", "--no-merges", "--format=%s", range])
      .split("\n")
      .filter(Boolean);
    description = renderSections(groupCommits(subjects));
  }

  const payload = buildPayload({
    tag,
    repo: env.GITHUB_REPOSITORY ?? "Petitclg/lol-game-review",
    serverUrl: env.GITHUB_SERVER_URL ?? "https://github.com",
    description,
    date: now(),
  });
  if (dryRun) {
    log(JSON.stringify(payload, null, 2));
    return { posted: false, payload };
  }
  await postWebhook(webhook, payload, { fetchImpl, sleep });
  log(`Annonce de ${tag} publiée sur Discord.`);
  return { posted: true, payload };
}
