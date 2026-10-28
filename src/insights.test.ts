import { describe, expect, it } from "vitest";
import { parseItemPrices } from "./ddragon";
import {
  formatGold,
  formatItemAdvantage,
  formatLevelAdvantage,
  groupThousands,
  involvedPlayers,
  isFlashEquipped,
  itemValue,
  summarize,
  toneOf,
} from "./insights";
import { makeClip } from "./testUtils";
import type { Involved, PlayerSnapshot } from "./types";

function player(name: string, team: "ORDER" | "CHAOS", extra: Partial<PlayerSnapshot> = {}): PlayerSnapshot {
  return {
    name,
    champion: "Ahri",
    team,
    isMe: false,
    level: 10,
    kills: 0,
    deaths: 0,
    assists: 0,
    items: [],
    spells: ["Flash", "Ignite"],
    spellKeys: ["SummonerFlash", "SummonerDot"],
    ...extra,
  };
}

const PRICES = parseItemPrices({
  data: {
    "1055": { gold: { total: 450 }, consumed: false },
    "3031": { gold: { total: 3400 } },
    "2003": { gold: { total: 50 }, consumed: true },
    "3340": { gold: { total: 0 } },
    "bad": { gold: { total: 99 } },
    "1001": { gold: {} },
  },
});

function clipWith(players: PlayerSnapshot[], involved: Involved[] | undefined) {
  return makeClip("c", "kill", 100, { players, ...(involved ? { involved } : {}) });
}

describe("parseItemPrices / itemValue", () => {
  it("exclut consommables, babioles (0 or) et entrées invalides", () => {
    expect([...PRICES.entries()].sort()).toEqual([
      [1055, 450],
      [3031, 3400],
    ]);
  });
  it("additionne les prix et ignore consommables/babioles/inconnus", () => {
    const items = [
      { id: 1055, name: "Doran" },
      { id: 3031, name: "IE" },
      { id: 2003, name: "Potion" },
      { id: 3340, name: "Trinket" },
      { id: 9999, name: "?" },
    ];
    expect(itemValue(items, PRICES)).toBe(3850);
    expect(itemValue([], PRICES)).toBe(0);
    expect(itemValue(undefined, PRICES)).toBe(0);
  });
  it("prix indisponibles → null", () => {
    expect(itemValue([{ id: 1055, name: "Doran" }], null)).toBeNull();
    expect(itemValue([{ id: 1055, name: "Doran" }], undefined)).toBeNull();
  });
});

describe("isFlashEquipped", () => {
  it("via spellKeys", () => {
    expect(isFlashEquipped({ spells: ["???"], spellKeys: ["SummonerFlash"] })).toBe(true);
    expect(isFlashEquipped({ spells: ["Ignite", "Heal"], spellKeys: ["SummonerDot", "SummonerHeal"] })).toBe(false);
  });
  it("anciennes parties : noms localisés, insensibles à la casse", () => {
    for (const name of ["Flash", "flash", "Saut éclair", "BLITZ", "Destello", "Salto rápido", " Flash "]) {
      expect(isFlashEquipped({ spells: [name, "Ignite"], spellKeys: [] })).toBe(true);
    }
  });
  it("spellKeys absent ou aucun nom correspondant", () => {
    expect(isFlashEquipped({ spells: ["Flash"] })).toBe(true);
    expect(isFlashEquipped({ spells: ["Ignite", "Téléportation"], spellKeys: [] })).toBe(false);
    expect(isFlashEquipped({ spells: [] })).toBe(false);
  });
});

describe("involvedPlayers", () => {
  const me = player("Moi#1", "ORDER", { isMe: true });
  const ally = player("Ami#2", "ORDER");
  const foe = player("Ennemi#3", "CHAOS");
  const foe2 = player("Ennemi#4", "CHAOS");
  const players = [me, ally, foe, foe2];

  it("alliés d'abord, puis ennemis, avec « moi » repéré", () => {
    const res = involvedPlayers(
      clipWith(players, [
        { name: "Ennemi#3", roles: ["victim"] },
        { name: "Ami#2", roles: ["assist"] },
        { name: "Moi#1", roles: ["killer"] },
      ]),
    );
    expect(res.map((r) => r.player.name)).toEqual(["Ami#2", "Moi#1", "Ennemi#3"]);
    expect(res.map((r) => r.isAlly)).toEqual([true, true, false]);
    expect(res.find((r) => r.player.name === "Moi#1")?.isMe).toBe(true);
  });
  it("rôles multiples conservés dans l'ordre tueur/victime/assist, sans doublon", () => {
    const res = involvedPlayers(
      clipWith(players, [
        { name: "Moi#1", roles: ["assist", "killer", "killer"] },
        { name: "Moi#1", roles: ["victim"] },
      ]),
    );
    expect(res).toHaveLength(1);
    expect(res[0]?.roles).toEqual(["killer", "victim", "assist"]);
  });
  it("noms inconnus et rôles invalides ignorés", () => {
    const res = involvedPlayers(
      clipWith(players, [
        { name: "Fantôme#9", roles: ["killer"] },
        { name: "Ami#2", roles: ["boss" as never, "assist"] },
      ]),
    );
    expect(res.map((r) => [r.player.name, r.roles])).toEqual([["Ami#2", ["assist"]]]);
  });
  it("involved vide ou absent → []", () => {
    expect(involvedPlayers(clipWith(players, []))).toEqual([]);
    expect(involvedPlayers(clipWith(players, undefined))).toEqual([]);
  });
  it("si je joue CHAOS, mes alliés sont CHAOS", () => {
    const meChaos = player("Moi#1", "CHAOS", { isMe: true });
    const res = involvedPlayers(
      clipWith([meChaos, ally], [
        { name: "Ami#2", roles: ["victim"] },
        { name: "Moi#1", roles: ["killer"] },
      ]),
    );
    expect(res.map((r) => r.player.name)).toEqual(["Moi#1", "Ami#2"]);
    expect(res.map((r) => r.isAlly)).toEqual([true, false]);
  });
});

describe("summarize", () => {
  const inv = (players: PlayerSnapshot[]) =>
    involvedPlayers(clipWith(players, players.map((p) => ({ name: p.name, roles: ["assist" as const] }))));

  it("involved vide → null", () => {
    expect(summarize([], PRICES)).toBeNull();
  });

  it("niveaux : signe et arrondi à 0,1", () => {
    const up = summarize(
      inv([player("Moi", "ORDER", { isMe: true, level: 12 }), player("E", "CHAOS", { level: 10 })]),
      PRICES,
    );
    expect(up?.levelAdvantage).toBe(2);
    const down = summarize(
      inv([
        player("Moi", "ORDER", { isMe: true, level: 9 }),
        player("E1", "CHAOS", { level: 10 }),
        player("E2", "CHAOS", { level: 11 }),
      ]),
      PRICES,
    );
    expect(down?.levelAdvantage).toBe(-1.5);
    const frac = summarize(
      inv([
        player("Moi", "ORDER", { isMe: true, level: 11 }),
        player("A", "ORDER", { level: 10 }),
        player("A2", "ORDER", { level: 10 }),
        player("E", "CHAOS", { level: 10 }),
      ]),
      PRICES,
    );
    expect(frac?.levelAdvantage).toBe(0.3);
    const zero = summarize(
      inv([player("Moi", "ORDER", { isMe: true, level: 10 }), player("E", "CHAOS", { level: 10 })]),
      PRICES,
    );
    expect(zero?.levelAdvantage).toBe(0);
    expect(Object.is(zero?.levelAdvantage, -0)).toBe(false);
  });

  it("niveaux et items : null si un camp est vide", () => {
    const s = summarize(inv([player("Moi", "ORDER", { isMe: true }), player("A", "ORDER")]), PRICES);
    expect(s).toEqual({ levelAdvantage: null, itemAdvantage: null, itemTotals: null, flash: null });
  });

  it("avance d'items : moyennes, signe, totaux", () => {
    const big = [{ id: 3031, name: "IE" }];
    const small = [{ id: 1055, name: "Doran" }];
    const s = summarize(
      inv([
        player("Moi", "ORDER", { isMe: true, items: big }),
        player("E1", "CHAOS", { items: small }),
        player("E2", "CHAOS", { items: small }),
      ]),
      PRICES,
    );
    expect(s?.itemAdvantage).toBe(2950);
    expect(s?.itemTotals).toEqual({ ally: 3400, enemy: 900 });
    const neg = summarize(
      inv([player("Moi", "ORDER", { isMe: true, items: small }), player("E", "CHAOS", { items: big })]),
      PRICES,
    );
    expect(neg?.itemAdvantage).toBe(-2950);
  });

  it("prix manquants : pas d'avance d'items, le reste est calculé", () => {
    const s = summarize(
      inv([player("Moi", "ORDER", { isMe: true, level: 11 }), player("E", "CHAOS", { level: 10 })]),
      null,
    );
    expect(s?.itemAdvantage).toBeNull();
    expect(s?.itemTotals).toBeNull();
    expect(s?.levelAdvantage).toBe(1);
  });

  describe("verdict Flash", () => {
    const noFlash = { spells: ["Ignite", "Heal"], spellKeys: ["SummonerDot", "SummonerHeal"] };
    const flash = (mine: PlayerSnapshot[], theirs: PlayerSnapshot[]) =>
      summarize(inv([...mine, ...theirs]), PRICES)?.flash;
    const me = (extra: Partial<PlayerSnapshot> = {}) => player("Moi", "ORDER", { isMe: true, ...extra });

    it("avantage : tous mes impliqués l'ont, un ennemi non", () => {
      expect(flash([me()], [player("E", "CHAOS", noFlash)])).toBe("advantage");
      expect(flash([me(), player("A", "ORDER")], [player("E1", "CHAOS"), player("E2", "CHAOS", noFlash)])).toBe("advantage");
    });
    it("désavantage : tous les ennemis l'ont, un des miens non", () => {
      expect(flash([me(noFlash)], [player("E", "CHAOS")])).toBe("disadvantage");
    });
    it("neutre : des deux côtés / aucun / inégal", () => {
      expect(flash([me()], [player("E", "CHAOS")])).toBe("both");
      expect(flash([me(noFlash)], [player("E", "CHAOS", noFlash)])).toBe("none");
      expect(flash([me(noFlash), player("A", "ORDER")], [player("E", "CHAOS", noFlash)])).toBe("partial");
    });
    it("anciennes parties : noms localisés sans spellKeys", () => {
      const old = (spells: string[]) => ({ spells, spellKeys: [] });
      expect(flash([me(old(["Saut éclair", "Ignite"]))], [player("E", "CHAOS", old(["Ignite", "Soin"]))])).toBe("advantage");
      expect(flash([me(old(["Blitz", "Ignite"]))], [player("E", "CHAOS", old(["Destello", "Ignite"]))])).toBe("both");
    });
    it("pas de comparaison si un camp est vide", () => {
      expect(flash([me()], [])).toBeNull();
    });
  });
});

describe("formats", () => {
  it("groupThousands / formatGold (espace fine insécable)", () => {
    expect(groupThousands(3250)).toBe("3\u202f250");
    expect(groupThousands(999)).toBe("999");
    expect(groupThousands(1234567)).toBe("1\u202f234\u202f567");
    expect(formatGold(3250)).toBe("3\u202f250 or");
    expect(formatGold(0)).toBe("0 or");
  });
  it("formatItemAdvantage", () => {
    expect(formatItemAdvantage(850)).toBe("+850 or d'items");
    expect(formatItemAdvantage(-1200)).toBe("\u22121\u202f200 or d'items");
    expect(formatItemAdvantage(0)).toBe("0 or d'items");
  });
  it("formatLevelAdvantage", () => {
    expect(formatLevelAdvantage(2)).toBe("+2 niveaux");
    expect(formatLevelAdvantage(1)).toBe("+1 niveau");
    expect(formatLevelAdvantage(-1.5)).toBe("\u22121,5 niveau");
    expect(formatLevelAdvantage(0)).toBe("0 niveau");
    expect(formatLevelAdvantage(-3.2)).toBe("\u22123,2 niveaux");
  });
  it("toneOf", () => {
    expect(toneOf(1)).toBe("good");
    expect(toneOf(-1)).toBe("bad");
    expect(toneOf(0)).toBe("neutral");
    expect(toneOf(null)).toBe("neutral");
  });
});
