import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  championIconUrl,
  findChampionByKey,
  findChampion,
  getDdragon,
  itemIconUrl,
  normalizeName,
  parseChampionIndex,
  resetDdragonCache,
} from "./ddragon";

const CHAMPION_JSON = {
  data: {
    MonkeyKing: { id: "MonkeyKing", key: "62", name: "Wukong" },
    Kaisa: { id: "Kaisa", key: "145", name: "Kai'Sa" },
    Ahri: { id: "Ahri", key: "103", name: "Ahri" },
    Broken: { id: "Broken", key: "abc", name: "Broken" },
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
  it("recherche par identifiant d'abord (insensible à la casse), puis par nom affiché", () => {
    expect(findChampion(index, "MonkeyKing")?.name).toBe("Wukong");
    expect(findChampion(index, "monkeyking")?.id).toBe("MonkeyKing");
    expect(findChampion(index, "Kaisa")?.id).toBe("Kaisa");
    expect(findChampion(index, "Wukong")?.id).toBe("MonkeyKing");
    expect(findChampion(index, "Kai'Sa")?.id).toBe("Kaisa");
  });
  it("l'identifiant l'emporte sur un nom identique d'un autre champion", () => {
    const idx = parseChampionIndex({
      data: {
        Nunu: { id: "Nunu", key: "20", name: "Nunu & Willump" },
        Other: { id: "Other", key: "21", name: "Nunu" },
      },
    });
    expect(findChampion(idx, "Nunu")?.key).toBe(20);
  });
  it("ignore les entrées invalides", () => {
    expect(index.byKey.size).toBe(3);
    expect(findChampion(index, "Broken")).toBeNull();
    expect(findChampion(null, "Ahri")).toBeNull();
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
});
