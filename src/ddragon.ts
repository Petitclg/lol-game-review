export const DDRAGON_BASE = "https://ddragon.leagueoflegends.com";

export interface ChampionRef {
  /** Identifiant DDragon (ex. "MonkeyKing"), utilisé dans les URL. */
  id: string;
  /** Nom affiché (ex. "Wukong"). */
  name: string;
  /** Clé numérique (ex. 62). */
  key: number;
}

export interface ChampionIndex {
  byKey: Map<number, ChampionRef>;
  byId: Map<string, ChampionRef>;
  byName: Map<string, ChampionRef>;
}

export interface Ddragon {
  version: string;
  champions: ChampionIndex;
}

const LS_PREFIX = "lgr.ddragon.";

function storage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

function readCache<T>(key: string): T | null {
  try {
    const raw = storage()?.getItem(LS_PREFIX + key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

function writeCache(key: string, value: unknown): void {
  try {
    storage()?.setItem(LS_PREFIX + key, JSON.stringify(value));
  } catch {
    // quota dépassé ou stockage indisponible : le cache est facultatif
  }
}

async function fetchJson(url: string): Promise<unknown> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return (await res.json()) as unknown;
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}

/** Normalise un nom de champion pour la recherche ("Kai'Sa" → "kaisa"). */
export function normalizeName(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]/g, "");
}

export function parseChampionIndex(json: unknown): ChampionIndex {
  const index: ChampionIndex = { byKey: new Map(), byId: new Map(), byName: new Map() };
  if (!isRecord(json) || !isRecord(json["data"])) return index;
  for (const entry of Object.values(json["data"])) {
    if (!isRecord(entry)) continue;
    const { id, name, key } = entry;
    if (typeof id !== "string" || typeof name !== "string") continue;
    const numKey = Number(key);
    if (!Number.isInteger(numKey)) continue;
    const ref: ChampionRef = { id, name, key: numKey };
    index.byKey.set(numKey, ref);
    index.byId.set(normalizeName(id), ref);
    index.byName.set(normalizeName(name), ref);
  }
  return index;
}

/** Résout la valeur du backend (identifiant DDragon, ex. "MonkeyKing") : id d'abord, puis nom affiché. */
export function findChampion(index: ChampionIndex | null, idOrName: string): ChampionRef | null {
  if (!index) return null;
  const norm = normalizeName(idOrName);
  return index.byId.get(norm) ?? index.byName.get(norm) ?? null;
}

export function findChampionByKey(index: ChampionIndex | null, key: number): ChampionRef | null {
  return index?.byKey.get(key) ?? null;
}

export function championIconUrl(version: string, championId: string): string {
  return `${DDRAGON_BASE}/cdn/${encodeURIComponent(version)}/img/champion/${encodeURIComponent(championId)}.png`;
}

export function itemIconUrl(version: string, itemId: number): string {
  return `${DDRAGON_BASE}/cdn/${encodeURIComponent(version)}/img/item/${Math.trunc(itemId)}.png`;
}

let ddragonPromise: Promise<Ddragon | null> | null = null;

export function resetDdragonCache(): void {
  ddragonPromise = null;
}

async function loadVersion(): Promise<string | null> {
  try {
    const versions = await fetchJson(`${DDRAGON_BASE}/api/versions.json`);
    const first = Array.isArray(versions) ? (versions as unknown[])[0] : undefined;
    if (typeof first === "string") {
      writeCache("version", first);
      return first;
    }
  } catch {
    // hors ligne : repli sur le cache
  }
  return readCache<string>("version");
}

async function loadDdragon(): Promise<Ddragon | null> {
  const version = await loadVersion();
  if (!version) return null;
  const cacheKey = `champions.${version}`;
  let json: unknown = readCache<unknown>(cacheKey);
  if (!json) {
    try {
      json = await fetchJson(`${DDRAGON_BASE}/cdn/${version}/data/en_US/champion.json`);
      writeCache(cacheKey, json);
    } catch {
      return null;
    }
  }
  const champions = parseChampionIndex(json);
  if (champions.byKey.size === 0) return null;
  return { version, champions };
}

/** Version courante + index des champions ; null si Data Dragon est indisponible. */
export function getDdragon(): Promise<Ddragon | null> {
  if (!ddragonPromise) {
    ddragonPromise = loadDdragon().then((d) => {
      // ne pas mémoriser un échec : permettre une nouvelle tentative
      if (!d) ddragonPromise = null;
      return d;
    });
  }
  return ddragonPromise;
}
