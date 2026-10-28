import { describe, expect, it } from "vitest";
import { countByTheme, filterClips, groupByTheme, sortClips } from "./clips";
import { SAMPLE_CLIPS } from "./testUtils";

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

describe("countByTheme / groupByTheme", () => {
  it("compte chaque thème", () => {
    expect(countByTheme(SAMPLE_CLIPS)).toEqual({
      all: 6,
      kill: 2,
      death: 1,
      assist: 1,
      objective: 1,
      turret: 1,
    });
  });
  it("groupe chaque thème trié", () => {
    const groups = groupByTheme(SAMPLE_CLIPS);
    expect(groups.kill.map((c) => c.id)).toEqual(["a", "b"]);
    expect(groups.death).toHaveLength(1);
  });
});
