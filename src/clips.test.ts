import { describe, expect, it } from "vitest";
import {
  THEME_TABS,
  clipParts,
  clipThemes,
  countByTheme,
  filterClips,
  groupByTheme,
  isTheme,
  sortClips,
} from "./clips";
import { SAMPLE_CLIPS, makeClip } from "./testUtils";
import type { Clip, Theme } from "./types";
import { buildMockClips } from "./mock";

describe("thème « ultimate »", () => {
  const clips = [...SAMPLE_CLIPS, makeClip("u2", "ultimate", 800), makeClip("u1", "ultimate", 150)];

  it("onglet « Ultimates » présent, après Assists", () => {
    const ids = THEME_TABS.map((t) => t.id);
    expect(ids).toContain("ultimate");
    expect(ids.indexOf("ultimate")).toBe(ids.indexOf("assist") + 1);
    expect(THEME_TABS.find((t) => t.id === "ultimate")?.label).toBe("Ultimates");
  });
  it("compte, filtre et groupe les ultimates (ordre chronologique)", () => {
    expect(countByTheme(clips)).toMatchObject({ all: 8, ultimate: 2, kill: 2 });
    expect(filterClips(clips, "ultimate").map((c) => c.id)).toEqual(["u1", "u2"]);
    expect(groupByTheme(clips).ultimate.map((c) => c.id)).toEqual(["u1", "u2"]);
    expect(filterClips(clips, "all")).toHaveLength(8);
  });
  it("isTheme accepte les thèmes connus et rejette le reste", () => {
    for (const t of ["kill", "death", "assist", "ultimate", "flash", "key_moment", "objective", "turret"])
      expect(isTheme(t)).toBe(true);
    for (const t of ["inconnu", "", null, undefined, 3]) expect(isTheme(t)).toBe(false);
  });
  it("un thème inconnu du backend ne casse ni les compteurs ni les groupes", () => {
    const weird = [...clips, makeClip("x", "futur" as never, 10)];
    const counts = countByTheme(weird);
    expect(counts.all).toBe(9);
    expect(Object.values(counts).some(Number.isNaN)).toBe(false);
    expect(groupByTheme(weird).ultimate).toHaveLength(2);
  });
  it("les données simulées contiennent 3 ultimates dont un fusionné, lanceur seul", () => {
    const game = {
      gameId: 7001, createdMs: 0, durationSecs: 1812, queueId: 420, championId: 103, kills: 9,
      deaths: 3, assists: 11, win: true, sessionId: "s", clipsReady: false, hasRecording: true,
    };
    const ults = filterClips(buildMockClips(game), "ultimate");
    expect(ults).toHaveLength(3);
    expect(ults.map((c) => c.label).filter((l) => l === "Ultimate ×2")).toHaveLength(1);
    for (const c of ults) expect(c.involved).toEqual([{ name: "Invocateur#EUW", roles: ["caster"] }]);
  });
});

describe("sortClips", () => {
  it("trie par gameTimeSecs sans muter l'entrée", () => {
    const input = [...SAMPLE_CLIPS];
    const sorted = sortClips(input);
    expect(sorted.map((c) => c.gameTimeSecs)).toEqual([300, 300, 450, 600, 900, 1200]);
    expect(input).toEqual(SAMPLE_CLIPS);
  });
  it("départage les égalités de façon stable par id", () => {
    expect(sortClips(SAMPLE_CLIPS).slice(0, 2).map((c) => c.id)).toEqual(["a", "f"]);
  });
});

describe("filterClips", () => {
  it("'all' renvoie tout, trié", () => {
    expect(filterClips(SAMPLE_CLIPS, "all")).toHaveLength(6);
  });
  it("filtre par thème et garde l'ordre chronologique", () => {
    expect(filterClips(SAMPLE_CLIPS, "kill").map((c) => c.id)).toEqual(["a", "b"]);
    expect(filterClips(SAMPLE_CLIPS, "turret").map((c) => c.id)).toEqual(["e"]);
  });
  it("thème vide → tableau vide", () => {
    expect(filterClips([], "death")).toEqual([]);
  });
});

describe("clips fusionnés", () => {
  const merged = makeClip("seq-0", "death", 150, {
    themes: ["death", "kill", "ultimate"],
    parts: [
      { id: "kill-150.0", theme: "kill", label: "Kill", gameTimeSecs: 150 },
      { id: "ultimate-155.5", theme: "ultimate", label: "Ultimate", gameTimeSecs: 155.5 },
      { id: "death-160.0", theme: "death", label: "Mort", gameTimeSecs: 160 },
    ],
  });
  const plain = makeClip("kill-1", "kill", 400);

  it("un clip fusionné apparaît dans chacun de ses thèmes", () => {
    const clips = [plain, merged];
    expect(filterClips(clips, "death").map((c) => c.id)).toEqual(["seq-0"]);
    expect(filterClips(clips, "kill").map((c) => c.id)).toEqual(["seq-0", "kill-1"]);
    expect(filterClips(clips, "ultimate").map((c) => c.id)).toEqual(["seq-0"]);
    expect(filterClips(clips, "flash")).toEqual([]);
    expect(filterClips(clips, "all")).toHaveLength(2);
  });

  it("il est compté une fois par thème, et une seule fois dans « Tous »", () => {
    expect(countByTheme([plain, merged])).toMatchObject({ all: 2, kill: 2, death: 1, ultimate: 1 });
    expect(groupByTheme([plain, merged]).kill.map((c) => c.id)).toEqual(["seq-0", "kill-1"]);
  });

  it("clipThemes : thèmes connus, sinon le thème du clip (ancien clip ou thème inconnu ignoré)", () => {
    expect(clipThemes(merged)).toEqual(["death", "kill", "ultimate"]);
    expect(clipThemes(plain)).toEqual(["kill"]);
    expect(clipThemes(makeClip("x", "kill", 1, { themes: [] }))).toEqual(["kill"]);
    const future: Clip = { ...merged, themes: ["death", "futur" as Theme] };
    expect(clipThemes(future)).toEqual(["death"]);
  });

  it("clipParts : les moments du clip, ou un moment unique reconstruit pour un ancien clip", () => {
    expect(clipParts(merged).map((p) => p.id)).toEqual(["kill-150.0", "ultimate-155.5", "death-160.0"]);
    expect(clipParts(makeClip("old", "death", 160.04))).toEqual([
      { id: "death-160.0", theme: "death", label: "death", gameTimeSecs: 160.04 },
    ]);
  });
});

describe("countByTheme / groupByTheme", () => {
  it("compte chaque thème", () => {
    expect(countByTheme(SAMPLE_CLIPS)).toEqual({
      all: 6,
      kill: 2,
      death: 1,
      assist: 1,
      ultimate: 0,
      flash: 0,
      key_moment: 0,
      objective: 1,
      turret: 1,
    });
  });
  it("compte et groupe les flashs et les moments clés", () => {
    const clips = [
      makeClip("f1", "flash", 500),
      makeClip("f2", "flash", 100),
      makeClip("m1", "key_moment", 900),
    ];
    expect(countByTheme(clips)).toMatchObject({ all: 3, flash: 2, key_moment: 1 });
    expect(filterClips(clips, "flash").map((c) => c.id)).toEqual(["f2", "f1"]);
    expect(groupByTheme(clips).key_moment.map((c) => c.id)).toEqual(["m1"]);
    expect(THEME_TABS.map((t) => t.id)).toEqual(expect.arrayContaining(["flash", "key_moment"]));
  });
  it("les données simulées contiennent des flashs (lanceur seul) et des moments clés", () => {
    const game = {
      gameId: 7001, createdMs: 0, durationSecs: 1812, queueId: 420, championId: 103, kills: 9,
      deaths: 3, assists: 11, win: true, sessionId: "s", clipsReady: false, hasRecording: true,
    };
    const mock = buildMockClips(game);
    const flashes = filterClips(mock, "flash");
    expect(flashes.length).toBeGreaterThan(0);
    for (const c of flashes) expect(c.involved).toEqual([{ name: "Invocateur#EUW", roles: ["caster"] }]);
    expect(filterClips(mock, "key_moment").length).toBeGreaterThan(0);
  });
  it("groupe chaque thème trié", () => {
    const groups = groupByTheme(SAMPLE_CLIPS);
    expect(groups.kill.map((c) => c.id)).toEqual(["a", "b"]);
    expect(groups.death).toHaveLength(1);
  });
});
