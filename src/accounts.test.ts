import { describe, expect, it } from "vitest";
import { accountKey, accountOptions, defaultAccountKey } from "./accounts";
import type { GameEntry } from "./types";

function game(account: string | null, createdMs: number): GameEntry {
  return {
    gameId: createdMs,
    createdMs,
    durationSecs: 1800,
    queueId: 420,
    championId: 1,
    kills: 0,
    deaths: 0,
    assists: 0,
    win: true,
    account,
    sessionId: null,
    clipsReady: false,
    hasRecording: false,
  };
}

describe("accountKey", () => {
  it("ignore la casse et les espaces autour", () => {
    expect(accountKey(" Nom#EUW ")).toBe("nom#euw");
  });
  it("une partie sans compte a la clé vide", () => {
    expect(accountKey(null)).toBe("");
    expect(accountKey(undefined)).toBe("");
  });
});

describe("accountOptions", () => {
  it("regroupe les casses et compte les parties, le plus récent d'abord", () => {
    const options = accountOptions([game("A#EUW", 10), game("B#EUW", 30), game("a#euw", 20)], null);
    expect(options).toEqual([
      { key: "b#euw", label: "B#EUW", games: 1 },
      { key: "a#euw", label: "a#euw", games: 2 },
    ]);
  });
  it("affiche l'écriture de la partie la plus récente", () => {
    const [a] = accountOptions([game("Nom#EUW", 10), game("NOM#EUW", 20)], null);
    expect(a?.label).toBe("NOM#EUW");
  });
  it("ajoute le compte connecté sans partie, après les autres", () => {
    const options = accountOptions([game("A#EUW", 10)], "Moi#EUW");
    expect(options.map((o) => o.key)).toEqual(["a#euw", "moi#euw"]);
    expect(options[1]).toMatchObject({ games: 0, label: "Moi#EUW" });
  });
  it("met les parties de compte inconnu en dernier", () => {
    const options = accountOptions([game(null, 99), game("A#EUW", 10)], null);
    expect(options.map((o) => o.label)).toEqual(["A#EUW", "Compte inconnu"]);
  });
});

describe("defaultAccountKey", () => {
  const options = accountOptions([game("A#EUW", 10), game("B#EUW", 5)], null);
  it("préfère le compte connecté", () => {
    expect(defaultAccountKey(options, "b#EUW")).toBe("b#euw");
  });
  it("sinon prend le premier, ou rien", () => {
    expect(defaultAccountKey(options, "Absent#EUW")).toBe("a#euw");
    expect(defaultAccountKey([], null)).toBeNull();
  });
});
