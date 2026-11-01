import { describe, expect, it } from "vitest";
import {
  DEATH_TAGS,
  PLAY_TAGS,
  TEAMFIGHT_TAGS,
  isEmptyAnnotation,
  tagDef,
  tagsFor,
  toggleTag,
  withAnnotation,
} from "./annotations";

describe("catalogue d'étiquettes", () => {
  const all = [...DEATH_TAGS, ...PLAY_TAGS, ...TEAMFIGHT_TAGS];

  it("a des identifiants uniques, acceptés par le backend (a-z, 0-9, _ ; 48 caractères au plus)", () => {
    const ids = all.map((t) => t.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of ids) expect(id).toMatch(/^[a-z0-9_]{1,48}$/);
  });

  it("couvre les erreurs de mort demandées", () => {
    const labels = DEATH_TAGS.map((t) => t.label);
    for (const wanted of [
      "Face-check",
      "Erreur mécanique",
      "Infériorité numérique",
      "Catch",
      "Bonne mort",
      "Mauvais jungle tracking",
      "Présence d'un ennemi imprévue",
    ]) {
      expect(labels).toContain(wanted);
    }
    expect(TEAMFIGHT_TAGS.map((t) => t.label)).toEqual(["Bon teamfight", "Mauvais teamfight"]);
  });

  it("propose les erreurs aux morts et les bons réflexes aux autres moments, teamfight pour tous", () => {
    const death = tagsFor("death").map((t) => t.id);
    expect(death).toContain("face_check");
    expect(death).not.toContain("good_outplay");
    expect(death).toContain("bad_teamfight");
    for (const theme of ["kill", "assist", "ultimate", "flash", "objective", "turret", "key_moment"] as const) {
      const ids = tagsFor(theme).map((t) => t.id);
      expect(ids).toContain("good_outplay");
      expect(ids).not.toContain("face_check");
      expect(ids).toContain("good_teamfight");
    }
  });

  it("garde lisible un identifiant inconnu", () => {
    expect(tagDef("greed").label).toBe("Greed");
    expect(tagDef("future_tag")).toEqual({ id: "future_tag", label: "future tag", tone: "neutral" });
  });
});

describe("annotations", () => {
  it("toggleTag ajoute en fin et retire sans réordonner", () => {
    expect(toggleTag(["a", "b"], "c")).toEqual(["a", "b", "c"]);
    expect(toggleTag(["a", "b", "c"], "b")).toEqual(["a", "c"]);
  });

  it("une annotation sans étiquette ni note (hors blancs) est vide", () => {
    expect(isEmptyAnnotation(undefined)).toBe(true);
    expect(isEmptyAnnotation({ tags: [], note: "  " })).toBe(true);
    expect(isEmptyAnnotation({ tags: ["greed"], note: "" })).toBe(false);
    expect(isEmptyAnnotation({ tags: [], note: "x" })).toBe(false);
  });

  it("withAnnotation ajoute, remplace et retire sans modifier l'original", () => {
    const base = { "death-1.0": { tags: ["greed"], note: "" } };
    const added = withAnnotation(base, "kill-2.0", { tags: [], note: "joli" });
    expect(Object.keys(added)).toEqual(["death-1.0", "kill-2.0"]);
    expect(withAnnotation(added, "death-1.0", { tags: [], note: "" })).toEqual({
      "kill-2.0": { tags: [], note: "joli" },
    });
    expect(Object.keys(base)).toEqual(["death-1.0"]);
  });
});
