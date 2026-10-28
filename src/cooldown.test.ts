import { describe, expect, it } from "vitest";
import { baseCooldownForRank, effectiveCooldown, formatCooldown } from "./cooldown";

describe("effectiveCooldown", () => {
  it("renvoie la base sans haste", () => {
    expect(effectiveCooldown(10, 0)).toBe(10);
  });
  it("applique base * 100 / (100 + haste)", () => {
    expect(effectiveCooldown(10, 100)).toBe(5);
    expect(effectiveCooldown(12, 50)).toBeCloseTo(8);
  });
  it("ignore un haste négatif", () => {
    expect(effectiveCooldown(10, -50)).toBe(10);
  });
});

describe("baseCooldownForRank", () => {
  const cds = [10, 9, 8, 7, 6];
  it("rang 0 → null", () => {
    expect(baseCooldownForRank(cds, 0)).toBeNull();
  });
  it("rang n → n-ième valeur", () => {
    expect(baseCooldownForRank(cds, 1)).toBe(10);
    expect(baseCooldownForRank(cds, 5)).toBe(6);
  });
  it("borne le rang au dernier élément", () => {
    expect(baseCooldownForRank([120, 100, 80], 5)).toBe(80);
  });
  it("tableau absent ou vide, rang invalide → null", () => {
    expect(baseCooldownForRank(undefined, 1)).toBeNull();
    expect(baseCooldownForRank([], 1)).toBeNull();
    expect(baseCooldownForRank(cds, -1)).toBeNull();
    expect(baseCooldownForRank(cds, 1.5)).toBeNull();
  });
});

describe("formatCooldown", () => {
  it("formate avec une décimale si nécessaire", () => {
    expect(formatCooldown(8)).toBe("8 s");
    expect(formatCooldown(6.6667)).toBe("6.7 s");
  });
});
