// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { parseItemPrices } from "../ddragon";
import { makeClip } from "../testUtils";
import type { PlayerSnapshot } from "../types";
import { renderInsights } from "./insightsBlock";

function p(name: string, team: "ORDER" | "CHAOS", extra: Partial<PlayerSnapshot> = {}): PlayerSnapshot {
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
  data: { "3031": { gold: { total: 3400 } }, "1055": { gold: { total: 450 } }, "2003": { gold: { total: 50 }, consumed: true } },
});

const players = [
  p("Moi", "ORDER", {
    isMe: true,
    level: 13,
    items: [
      { id: 3031, name: "IE" },
      { id: 2003, name: "Potion" },
    ],
  }),
  p("Ami", "ORDER", { level: 12, items: [{ id: 1055, name: "Doran" }] }),
  p("Ennemi", "CHAOS", {
    level: 10,
    items: [{ id: 1055, name: "Doran" }],
    spells: ["Ignite", "Heal"],
    spellKeys: ["SummonerDot", "SummonerHeal"],
  }),
];

describe("bloc « Analyse du moment »", () => {
  const clip = makeClip("c", "kill", 300, {
    players,
    myGold: 1250,
    involved: [
      { name: "Ennemi", roles: ["victim"] },
      { name: "Moi", roles: ["killer", "assist"] },
      { name: "Ami", roles: ["assist"] },
      { name: "Inconnu", roles: ["assist"] },
    ],
  });

  it("tableau : alliés d'abord, moi surligné, rôles, valeurs et Flash", () => {
    const block = renderInsights(clip, null, PRICES);
    const rows = [...block.querySelectorAll<HTMLElement>("tbody tr")];
    expect(rows).toHaveLength(3);
    expect(rows.map((r) => r.classList.contains("ally"))).toEqual([true, true, false]);
    expect(block.querySelectorAll("tr.me")).toHaveLength(1);
    const meRow = rows[0] as HTMLElement;
    expect(meRow.classList.contains("me")).toBe(true);
    expect(rows[1]?.classList.contains("me")).toBe(false);
    expect([...meRow.querySelectorAll(".role")].map((r) => r.textContent)).toEqual(["Tueur", "Assist"]);
    expect(meRow.querySelector(".involved-value")?.textContent).toBe("3\u202f400 or");
    expect(meRow.querySelector(".involved-flash")?.textContent).toBe("✓");
    expect(rows[2]?.querySelector(".involved-flash")?.textContent).toBe("✗");
    expect(rows[2]?.querySelector(".role")?.textContent).toBe("Victime");
  });

  it("résumé : niveaux, items, Flash avec tons et infobulle", () => {
    const block = renderInsights(clip, null, PRICES);
    const badges = [...block.querySelectorAll<HTMLElement>(".insight")];
    expect(badges.map((b) => b.querySelector(".insight-value")?.textContent)).toEqual([
      "+2,5 niveaux",
      "+1\u202f475 or d'items",
      "Avantage Flash",
    ]);
    expect(badges.every((b) => b.classList.contains("insight-good"))).toBe(true);
    expect(badges[1]?.title).toContain("Total alliés");
    expect(badges[2]?.title).toBe("Flash équipé, pas forcément disponible : Riot n'expose pas les cooldowns.");
  });

  it("ton rouge si défavorable et gris à 0", () => {
    const bad = makeClip("c", "kill", 300, {
      players: [p("Moi", "ORDER", { isMe: true, level: 8 }), p("E", "CHAOS", { level: 10 })],
      involved: [
        { name: "Moi", roles: ["victim"] },
        { name: "E", roles: ["killer"] },
      ],
    });
    const levels = renderInsights(bad, null, PRICES).querySelector(".insight");
    expect(levels?.classList.contains("insight-bad")).toBe(true);
    expect(levels?.querySelector(".insight-value")?.textContent).toBe("\u22122 niveaux");
    const flat = makeClip("c", "kill", 300, {
      players: [p("Moi", "ORDER", { isMe: true }), p("E", "CHAOS")],
      involved: [
        { name: "Moi", roles: ["killer"] },
        { name: "E", roles: ["victim"] },
      ],
    });
    const badges = [...renderInsights(flat, null, PRICES).querySelectorAll(".insight")];
    expect(badges.every((b) => b.classList.contains("insight-neutral"))).toBe(true);
  });

  it("or non dépensé affiché seulement si myGold est non nul", () => {
    expect(renderInsights(clip, null, PRICES).querySelector(".my-gold")?.textContent).toBe(
      "Or non dépensé : 1\u202f250 or",
    );
    const noGold = { ...clip, myGold: null };
    expect(renderInsights(noGold, null, PRICES).querySelector(".my-gold")).toBeNull();
  });

  it("prix indisponibles : « — » et pas d'avance d'items", () => {
    const block = renderInsights(clip, null, null);
    expect([...block.querySelectorAll(".involved-value")].map((c) => c.textContent)).toEqual(["—", "—", "—"]);
    const values = [...block.querySelectorAll(".insight")].map((b) => b.querySelector(".insight-value")?.textContent);
    expect(values).toContain("—");
    expect(values.some((v) => v?.includes("or d'items"))).toBe(false);
    expect(values).toContain("+2,5 niveaux");
  });

  it("note de bas de bloc", () => {
    expect(renderInsights(clip, null, PRICES).textContent).toContain(
      "L'or total des joueurs n'est pas fourni par Riot : la valeur des items est affichée à la place.",
    );
  });

  it("ultimate (lanceur seul) : ligne du joueur, aucun badge d'avantage, pas de message d'indisponibilité", () => {
    const ult = makeClip("u", "ultimate", 300, {
      players,
      myGold: 800,
      involved: [{ name: "Moi", roles: ["caster"] }],
    });
    for (const prices of [PRICES, null]) {
      const block = renderInsights(ult, null, prices);
      expect(block.querySelectorAll("tbody tr")).toHaveLength(1);
      expect(block.querySelector("tr.me .role-caster")?.textContent).toBe("Lanceur");
      expect(block.querySelector(".insight-list")).toBeNull();
      expect(block.querySelector(".insight")).toBeNull();
      expect(block.querySelector(".insights-unavailable")).toBeNull();
      expect(block.textContent).not.toContain("Analyse indisponible");
      expect(block.querySelector(".insights-note")?.textContent).toContain("L'or total des joueurs");
    }
  });

  it("alliés seuls : pas de comparaison, pas de crash", () => {
    const solo = makeClip("c", "turret", 300, {
      players,
      involved: [
        { name: "Moi", roles: ["killer"] },
        { name: "Ami", roles: ["assist"] },
      ],
    });
    const block = renderInsights(solo, null, PRICES);
    expect(block.querySelectorAll("tbody tr")).toHaveLength(2);
    expect(block.querySelector(".insight-list")).toBeNull();
  });

  it.each([[[]], [undefined], [[{ name: "Inconnu", roles: ["killer"] }]]])(
    "involved vide/absent/inconnu : message d'indisponibilité (%j)",
    (involved) => {
      const old = makeClip("c", "kill", 300, { players, ...(involved ? { involved: involved as never } : {}) });
      const block = renderInsights(old, null, PRICES);
      expect(block.querySelector(".insights-unavailable")?.textContent).toBe(
        "Analyse indisponible pour ce clip (partie enregistrée avant cette fonctionnalité)",
      );
      expect(block.querySelector("table")).toBeNull();
    },
  );

  it("n'interprète pas le HTML des noms", () => {
    const evil = '<img src=x onerror="alert(1)">';
    const c = makeClip("c", "kill", 1, {
      players: [p("Moi", "ORDER", { isMe: true, champion: evil }), p(evil, "CHAOS", { champion: evil })],
      involved: [
        { name: "Moi", roles: ["killer"] },
        { name: evil, roles: ["victim"] },
      ],
    });
    const block = renderInsights(c, null, PRICES);
    expect(block.querySelector("img")).toBeNull();
    expect(block.textContent).toContain(evil);
  });
});
