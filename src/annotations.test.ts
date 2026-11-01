import { describe, expect, it } from "vitest";
import {
  DEATH_TAGS,
  PLAY_TAGS,
  TEAMFIGHT_TAGS,
  cleanLabel,
  customTagId,
  isCustomTag,
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

  it("un tag personnalisé se lit par son texte, sans nuance", () => {
    expect(tagDef("custom:mal placé")).toEqual({ id: "custom:mal placé", label: "mal placé", tone: "neutral" });
    expect(isCustomTag("custom:x")).toBe(true);
    expect(isCustomTag("greed")).toBe(false);
    expect(customTagId("mal placé")).toBe("custom:mal placé");
  });
});

describe("cleanLabel (texte d'un tag personnalisé)", () => {
  it("retire les blancs de bord et réduit les blancs internes", () => {
    expect(cleanLabel("  mal   placé \n")).toBe("mal placé");
    expect(cleanLabel("Éclair ⚡ 100 %")).toBe("Éclair ⚡ 100 %");
  });

  it("refuse le vide, les caractères de contrôle et plus de 40 caractères (comptés comme le backend)", () => {
    expect(cleanLabel("")).toBeNull();
    expect(cleanLabel("   \t ")).toBeNull();
    expect(cleanLabel("a\u0007b")).toBeNull();
    expect(cleanLabel("x".repeat(41))).toBeNull();
    expect(cleanLabel("x".repeat(40))).not.toBeNull();
    expect(cleanLabel("é".repeat(40))).not.toBeNull();
    expect(cleanLabel("😀".repeat(40))).not.toBeNull();
    expect(cleanLabel("😀".repeat(41))).toBeNull();
  });
});

describe("annotations", () => {
  it("toggleTag ajoute en fin et retire sans réordonner", () => {
    expect(toggleTag(["a", "b"], "c")).toEqual(["a", "b", "c"]);
    expect(toggleTag(["a", "b", "c"], "b")).toEqual(["a", "c"]);
  });

  it("une annotation sans étiquette est vide", () => {
    expect(isEmptyAnnotation(undefined)).toBe(true);
    expect(isEmptyAnnotation({ tags: [] })).toBe(true);
    expect(isEmptyAnnotation({ tags: ["greed"] })).toBe(false);
    expect(isEmptyAnnotation({ tags: ["custom:à revoir"] })).toBe(false);
  });

  it("withAnnotation ajoute, remplace et retire sans modifier l'original", () => {
    const base = { "death-1.0": { tags: ["greed"] } };
    const added = withAnnotation(base, "kill-2.0", { tags: ["custom:joli"] });
    expect(Object.keys(added)).toEqual(["death-1.0", "kill-2.0"]);
    expect(withAnnotation(added, "death-1.0", { tags: [] })).toEqual({
      "kill-2.0": { tags: ["custom:joli"] },
    });
    expect(Object.keys(base)).toEqual(["death-1.0"]);
  });
});
