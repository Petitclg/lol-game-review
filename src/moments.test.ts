import { describe, expect, it } from "vitest";
import { THEMES } from "./clips";
import {
  NO_FILTERS,
  championIds,
  filterMoments,
  fold,
  hasFilters,
  momentKey,
  momentYear,
  playableMoments,
  tagCounts,
  themesOf,
  yearsOf,
  type MomentFilters,
} from "./moments";
import type { AnnotatedMoment, GameSummary } from "./types";

const at = (year: number) => new Date(year, 5, 15, 12).getTime();

function game(extra: Partial<GameSummary> = {}): GameSummary {
  return {
    gameId: 1,
    createdMs: at(2026),
    durationSecs: 1800,
    queueId: 420,
    championId: 103,
    kills: 1,
    deaths: 2,
    assists: 3,
    win: true,
    ...extra,
  };
}

function moment(partId: string, extra: Partial<AnnotatedMoment> = {}): AnnotatedMoment {
  return {
    sessionId: "s1",
    partId,
    theme: "death",
    label: "Mort",
    gameTimeSecs: 100,
    tags: [],
    clipId: "seq-0",
    file: "C:\\clips\\seq-0.mp4",
    game: game(),
    ...extra,
  };
}

const f = (extra: Partial<MomentFilters>): MomentFilters => ({ ...NO_FILTERS, ...extra });

const M = [
  moment("death-1.0", { tags: ["face_check", "greed", "custom:Trop avancé sans vision"], game: game({ championId: 103, win: false }) }),
  moment("death-2.0", { tags: ["face_check"], sessionId: "s2", game: game({ gameId: 2, championId: 222, win: false, createdMs: at(2025) }) }),
  moment("kill-3.0", { theme: "kill", label: "Kill", tags: ["good_outplay"], sessionId: "s3", game: game({ gameId: 3, championId: 103, win: true, createdMs: at(2025) }) }),
  moment("death-4.0", { tags: ["outnumbered"], sessionId: "s4", game: null, file: null, clipId: null }),
];

describe("filterMoments", () => {
  it("sans filtre, tout est gardé", () => {
    expect(filterMoments(M, NO_FILTERS)).toHaveLength(4);
    expect(hasFilters(NO_FILTERS)).toBe(false);
  });

  it("par champion", () => {
    expect(filterMoments(M, f({ championId: 103 })).map((m) => m.partId)).toEqual(["death-1.0", "kill-3.0"]);
    expect(filterMoments(M, f({ championId: 999 }))).toEqual([]);
  });

  it("par année de la partie (heure locale)", () => {
    expect(filterMoments(M, f({ year: 2025 })).map((m) => m.partId)).toEqual(["death-2.0", "kill-3.0"]);
    expect(filterMoments(M, f({ year: 2026 })).map((m) => m.partId)).toEqual(["death-1.0"]);
    expect(momentYear(M[0] as AnnotatedMoment)).toBe(2026);
    expect(momentYear(M[3] as AnnotatedMoment)).toBeNull();
  });

  it("par issue : victoire ou défaite", () => {
    expect(filterMoments(M, f({ outcome: "win" })).map((m) => m.partId)).toEqual(["kill-3.0"]);
    expect(filterMoments(M, f({ outcome: "loss" })).map((m) => m.partId)).toEqual(["death-1.0", "death-2.0"]);
  });

  it("une partie jamais listée est écartée par champion, année ou issue, mais pas par le reste", () => {
    for (const filter of [f({ championId: 103 }), f({ year: 2026 }), f({ outcome: "loss" }), f({ outcome: "win" })]) {
      expect(filterMoments(M, filter).some((m) => m.partId === "death-4.0")).toBe(false);
    }
    expect(filterMoments(M, f({ tag: "outnumbered" })).map((m) => m.partId)).toEqual(["death-4.0"]);
    expect(filterMoments(M, f({ theme: "death" }))).toHaveLength(3);
  });

  it("par étiquette et par type de moment", () => {
    expect(filterMoments(M, f({ tag: "face_check" })).map((m) => m.partId)).toEqual(["death-1.0", "death-2.0"]);
    expect(filterMoments(M, f({ theme: "kill" })).map((m) => m.partId)).toEqual(["kill-3.0"]);
  });

  it("par texte : libellé, nom des étiquettes (y compris personnalisées), type ; sans accents ni casse", () => {
    expect(filterMoments(M, f({ text: "AVANCE" })).map((m) => m.partId)).toEqual(["death-1.0"]);
    expect(filterMoments(M, f({ text: "face-check" })).map((m) => m.partId)).toEqual(["death-1.0", "death-2.0"]);
    expect(filterMoments(M, f({ text: "infériorité" })).map((m) => m.partId)).toEqual(["death-4.0"]);
    expect(filterMoments(M, f({ text: "  kill " })).map((m) => m.partId)).toEqual(["kill-3.0"]);
    expect(filterMoments(M, f({ text: "zzz" }))).toEqual([]);
  });

  it("par étiquette : un tag personnalisé se filtre comme les autres", () => {
    expect(filterMoments(M, f({ tag: "custom:Trop avancé sans vision" })).map((m) => m.partId)).toEqual(["death-1.0"]);
  });

  it("les filtres se combinent (et)", () => {
    const combined = f({ championId: 103, outcome: "loss", tag: "greed", year: 2026, theme: "death", text: "vision" });
    expect(filterMoments(M, combined).map((m) => m.partId)).toEqual(["death-1.0"]);
    expect(filterMoments(M, f({ championId: 103, outcome: "win", tag: "greed" }))).toEqual([]);
    expect(hasFilters(combined)).toBe(true);
    expect(hasFilters(f({ text: "   " }))).toBe(false);
  });

  it("ne modifie pas la liste d'origine", () => {
    const copy = [...M];
    filterMoments(M, f({ outcome: "win" }));
    expect(M).toEqual(copy);
  });
});

describe("décomptes et facettes", () => {
  it("tagCounts : plus fréquentes d'abord, puis par libellé", () => {
    expect(tagCounts(M)).toEqual([
      { tag: "face_check", count: 2 },
      { tag: "good_outplay", count: 1 }, // « Bon outplay »
      { tag: "greed", count: 1 }, // « Greed »
      { tag: "outnumbered", count: 1 }, // « Infériorité numérique »
      { tag: "custom:Trop avancé sans vision", count: 1 },
    ]);
    expect(tagCounts([])).toEqual([]);
  });

  it("le décompte suit les filtres : face-check sur les défaites de 2025", () => {
    const filtered = filterMoments(M, f({ outcome: "loss", year: 2025 }));
    expect(tagCounts(filtered)).toEqual([{ tag: "face_check", count: 1 }]);
  });

  it("championIds, yearsOf, themesOf", () => {
    expect(championIds(M)).toEqual([103, 222]);
    expect(yearsOf(M)).toEqual([2026, 2025]);
    expect(themesOf(M, THEMES)).toEqual(["kill", "death"]);
  });

  it("playableMoments ne garde que les clips qui existent encore", () => {
    expect(playableMoments(M).map((m) => m.partId)).toEqual(["death-1.0", "death-2.0", "kill-3.0"]);
  });

  it("momentKey distingue deux parties ayant le même moment", () => {
    expect(momentKey({ sessionId: "a", partId: "kill-1.0" })).not.toBe(momentKey({ sessionId: "b", partId: "kill-1.0" }));
  });

  it("fold retire accents et casse", () => {
    expect(fold("Présence d'un ENNEMI imprévue")).toBe("presence d'un ennemi imprevue");
  });
});
