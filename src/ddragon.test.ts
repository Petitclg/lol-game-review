import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  championIconUrl,
  findChampionByKey,
  findChampionByName,
  getChampionSpells,
  getDdragon,
  itemIconUrl,
  normalizeName,
  parseChampionIndex,
  parseSpells,
  resetDdragonCache,
  spellBaseCooldown,
} from "./ddragon";

const CHAMPION_JSON = {
  data: {
    MonkeyKing: { id: "MonkeyKing", key: "62", name: "Wukong" },
    Kaisa: { id: "Kaisa", key: "145", name: "Kai'Sa" },
    Ahri: { id: "Ahri", key: "103", name: "Ahri" },
    Broken: { id: "Broken", key: "abc", name: "Broken" },
  },
};

const AHRI_DETAIL = {
  data: {
    Ahri: {
      id: "Ahri",
      spells: [
        { id: "AhriQ", name: "Orb of Deception", cooldown: [7, 7, 7, 7, 7] },
        { id: "AhriW", name: "Fox-Fire", cooldown: [9, 8, 7, 6, 5] },
        { id: "AhriE", name: "Charm", cooldown: [14, 13, 12, 11, 10] },
        { id: "AhriR", name: "Spirit Rush", cooldown: [130, 105, 80] },
      ],
    },
  },
};

function jsonResponse(body: unknown, ok = true, status = 200) {
  return Promise.resolve({ ok, status, json: () => Promise.resolve(body) } as Response);
}

function memoryStorage(): Storage {
  const data = new Map<string, string>();
  return {
    get length() {
      return data.size;
    },
    clear: () => data.clear(),
    getItem: (k) => data.get(k) ?? null,
    key: (i) => [...data.keys()][i] ?? null,
    removeItem: (k) => void data.delete(k),
    setItem: (k, v) => void data.set(k, v),
  };
}

describe("parseChampionIndex", () => {
  const index = parseChampionIndex(CHAMPION_JSON);
  it("indexe par clé numérique", () => {
    expect(findChampionByKey(index, 62)).toEqual({ id: "MonkeyKing", name: "Wukong", key: 62 });
  });
  it("recherche par nom affiché ou identifiant, insensible à la casse/ponctuation", () => {
    expect(findChampionByName(index, "Wukong")?.id).toBe("MonkeyKing");
    expect(findChampionByName(index, "monkeyking")?.id).toBe("MonkeyKing");
    expect(findChampionByName(index, "Kai'Sa")?.id).toBe("Kaisa");
  });
  it("ignore les entrées invalides", () => {
    expect(index.byKey.size).toBe(3);
    expect(findChampionByName(index, "Broken")).toBeNull();
  });
  it("entrée malformée → index vide", () => {
    expect(parseChampionIndex(null).byKey.size).toBe(0);
    expect(parseChampionIndex({ data: 3 }).byKey.size).toBe(0);
  });
  it("normalizeName", () => {
    expect(normalizeName("Cho'Gath")).toBe("chogath");
  });
});

describe("URLs", () => {
  it("construit les URL d'icônes", () => {
    expect(championIconUrl("14.1.1", "Ahri")).toBe(
      "https://ddragon.leagueoflegends.com/cdn/14.1.1/img/champion/Ahri.png",
    );
    expect(itemIconUrl("14.1.1", 3031)).toBe(
      "https://ddragon.leagueoflegends.com/cdn/14.1.1/img/item/3031.png",
    );
  });
});

describe("parseSpells / spellBaseCooldown", () => {
  const spells = parseSpells(AHRI_DETAIL, "Ahri");
  it("extrait 4 sorts", () => {
    expect(spells.map((s) => s.id)).toEqual(["AhriQ", "AhriW", "AhriE", "AhriR"]);
  });
  it("cooldown par slot et rang", () => {
    expect(spellBaseCooldown(spells, 1, 3)).toBe(7);
    expect(spellBaseCooldown(spells, 3, 1)).toBe(130);
    expect(spellBaseCooldown(spells, 3, 0)).toBeNull();
    expect(spellBaseCooldown(spells, 9, 1)).toBeNull();
  });
  it("champion absent → []", () => {
    expect(parseSpells(AHRI_DETAIL, "Zed")).toEqual([]);
  });
});

describe("getDdragon (fetch mocké)", () => {
  beforeEach(() => {
    resetDdragonCache();
    vi.stubGlobal("localStorage", memoryStorage());
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("charge la version et l'index, avec mise en cache mémoire", async () => {
    const fetchMock = vi.fn((url: string) => {
      if (url.endsWith("/api/versions.json")) return jsonResponse(["14.2.1", "14.1.1"]);
      if (url.endsWith("/14.2.1/data/en_US/champion.json")) return jsonResponse(CHAMPION_JSON);
      return jsonResponse({}, false, 404);
    });
    vi.stubGlobal("fetch", fetchMock);
    const dd = await getDdragon();
    expect(dd?.version).toBe("14.2.1");
    expect(findChampionByKey(dd?.champions ?? null, 103)?.name).toBe("Ahri");
    await getDdragon();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("hors ligne sans cache → null, sans exception", async () => {
    vi.stubGlobal("fetch", () => Promise.reject(new TypeError("Failed to fetch")));
    await expect(getDdragon()).resolves.toBeNull();
  });

  it("hors ligne avec cache localStorage → utilise le cache", async () => {
    vi.stubGlobal("fetch", (url: string) => {
      if (url.endsWith("/api/versions.json")) return jsonResponse(["14.2.1"]);
      if (url.includes("champion.json")) return jsonResponse(CHAMPION_JSON);
      return jsonResponse({}, false, 404);
    });
    expect(await getDdragon()).not.toBeNull();
    resetDdragonCache();
    vi.stubGlobal("fetch", () => Promise.reject(new TypeError("offline")));
    const dd = await getDdragon();
    expect(dd?.version).toBe("14.2.1");
    expect(dd?.champions.byKey.size).toBe(3);
  });

  it("HTTP 500 sur versions.json → null", async () => {
    vi.stubGlobal("fetch", () => jsonResponse({}, false, 500));
    await expect(getDdragon()).resolves.toBeNull();
  });

  it("charge les sorts d'un champion, puis les sert depuis le cache hors ligne", async () => {
    vi.stubGlobal("fetch", () => jsonResponse(AHRI_DETAIL));
    const spells = await getChampionSpells("14.2.1", "Ahri");
    expect(spells).toHaveLength(4);
    resetDdragonCache();
    vi.stubGlobal("fetch", () => Promise.reject(new TypeError("offline")));
    const cached = await getChampionSpells("14.2.1", "Ahri");
    expect(cached?.[3]?.cooldown).toEqual([130, 105, 80]);
  });

  it("sorts indisponibles hors ligne et sans cache → null", async () => {
    vi.stubGlobal("fetch", () => Promise.reject(new TypeError("offline")));
    await expect(getChampionSpells("14.2.1", "Zed")).resolves.toBeNull();
  });
});
