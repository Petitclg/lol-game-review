import type { Backend, Unlisten } from "./api";
import { DEFAULT_SETTINGS, normalizeSettings } from "./settings";
import type {
  Account,
  Clip,
  ClipsProgress,
  GameEntry,
  Involved,
  PlayerSnapshot,
  Settings,
  Status,
  StorageUsage,
  Theme,
  UltimateStatus,
} from "./types";

const MINUTE = 60_000;

const CHAMPIONS: Record<number, string> = {
  103: "Ahri",
  222: "Jinx",
  64: "LeeSin",
  157: "Yasuo",
  412: "Thresh",
  81: "Ezreal",
  62: "MonkeyKing",
  238: "Zed",
  22: "Ashe",
  54: "Malphite",
};

const ITEMS = [
  { id: 6672, name: "Kraken Slayer" },
  { id: 3031, name: "Infinity Edge" },
  { id: 3006, name: "Berserker's Greaves" },
  { id: 3153, name: "Blade of The Ruined King" },
  { id: 3089, name: "Rabadon's Deathcap" },
  { id: 3020, name: "Sorcerer's Shoes" },
  { id: 6653, name: "Liandry's Torment" },
  { id: 3157, name: "Zhonya's Hourglass" },
  { id: 3047, name: "Plated Steelcaps" },
  { id: 3075, name: "Thornmail" },
  { id: 1055, name: "Doran's Blade" },
  { id: 3072, name: "Bloodthirster" },
];

const SPELLS = ["Flash", "Ignite", "Teleport", "Heal", "Smite", "Barrier", "Exhaust"];

const ENEMY_POOL = [62, 238, 22, 54, 412, 81, 157, 64, 222, 103];

const ME_NAME = "Invocateur#EUW";

/** Générateur pseudo-aléatoire déterministe (mulberry32). */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function buildGames(now: number): GameEntry[] {
  const base = {
    clipsReady: false,
    hasRecording: true,
    sessionId: null as string | null,
  };
  const list: GameEntry[] = [
    {
      ...base,
      gameId: 7001,
      createdMs: now - 25 * MINUTE,
      durationSecs: 1812,
      queueId: 420,
      championId: 103,
      kills: 9,
      deaths: 3,
      assists: 11,
      win: true,
      sessionId: "mock-session-7001",
    },
    {
      ...base,
      gameId: 7000,
      createdMs: now - 5 * 60 * MINUTE,
      durationSecs: 2204,
      queueId: 440,
      championId: 222,
      kills: 4,
      deaths: 8,
      assists: 6,
      win: false,
      sessionId: "mock-session-7000",
      clipsReady: true,
    },
    {
      ...base,
      gameId: 6999,
      createdMs: now - 26 * 60 * MINUTE,
      durationSecs: 1490,
      queueId: 450,
      championId: 64,
      kills: 14,
      deaths: 10,
      assists: 22,
      win: true,
    },
    {
      ...base,
      gameId: 6998,
      createdMs: now - 3 * 24 * 60 * MINUTE,
      durationSecs: 1975,
      queueId: 400,
      championId: 157,
      kills: 7,
      deaths: 7,
      assists: 3,
      win: false,
      sessionId: "mock-session-6998",
      // clips déjà générés, vidéo source supprimée
      clipsReady: true,
      hasRecording: false,
    },
    {
      ...base,
      gameId: 6997,
      createdMs: now - 6 * 24 * 60 * MINUTE,
      durationSecs: 1620,
      queueId: 490,
      championId: 412,
      kills: 1,
      deaths: 5,
      assists: 19,
      win: true,
    },
    {
      ...base,
      gameId: 6996,
      createdMs: now - 40 * 24 * 60 * MINUTE,
      durationSecs: 2410,
      queueId: 1700,
      championId: 81,
      kills: 11,
      deaths: 2,
      assists: 8,
      win: true,
    },
  ];
  // sans session, il n'y a jamais eu de vidéo
  return list.map((g) => (g.sessionId ? g : { ...g, hasRecording: false }));
}

const SPELL_KEYS: Record<string, string> = {
  Flash: "SummonerFlash",
  Ignite: "SummonerDot",
  Teleport: "SummonerTeleport",
  Heal: "SummonerHeal",
  Smite: "SummonerSmite",
  Barrier: "SummonerBarrier",
  Exhaust: "SummonerExhaust",
};

/** Joueurs impliqués : 1v1 (kill, mort), 2v1 (assist), moi seul (ultimate), alliés seuls (objectif, tour). */
function mockInvolved(theme: Theme, n: number, players: PlayerSnapshot[]): Involved[] {
  const name = (i: number) => players[i]?.name ?? "";
  const enemy = 5 + (n % 5);
  switch (theme) {
    case "kill":
      return [
        { name: name(0), roles: ["killer"] },
        { name: name(enemy), roles: ["victim"] },
      ];
    case "death":
      return [
        { name: name(enemy), roles: ["killer"] },
        { name: name(0), roles: ["victim"] },
      ];
    case "assist":
      return [
        { name: name(1), roles: ["killer"] },
        { name: name(0), roles: ["assist"] },
        { name: name(enemy), roles: ["victim"] },
      ];
    case "ultimate":
      return [{ name: name(0), roles: ["caster"] }];
    default:
      return [
        { name: name(0), roles: ["killer"] },
        { name: name(2), roles: ["assist"] },
      ];
  }
}

function buildPlayers(game: GameEntry, rand: () => number, progress: number): PlayerSnapshot[] {
  const mine = CHAMPIONS[game.championId] ?? "Ahri";
  const allies = Object.values(CHAMPIONS).filter((n) => n !== mine);
  const players: PlayerSnapshot[] = [];
  for (let i = 0; i < 10; i++) {
    const isMe = i === 0;
    const ally = i < 5;
    const champion = isMe
      ? mine
      : ally
        ? (allies[i % allies.length] ?? "Ashe")
        : (CHAMPIONS[ENEMY_POOL[i - 5] ?? 22] ?? "Ashe");
    const itemCount = Math.min(6, Math.floor(progress * 7 * (0.6 + rand() * 0.6)));
    const items = Array.from({ length: itemCount }, (_, k) => ITEMS[(i * 3 + k) % ITEMS.length]!);
    const secondSpell = SPELLS[1 + Math.floor(rand() * (SPELLS.length - 1))] ?? "Ignite";
    // quelques joueurs sans Flash pour varier les verdicts
    const spells = i === 6 || i === 2 ? ["Ignite", "Teleport"] : ["Flash", secondSpell];
    players.push({
      name: isMe ? ME_NAME : `Joueur${i}#EUW`,
      champion,
      team: ally ? "ORDER" : "CHAOS",
      isMe,
      level: Math.max(1, Math.min(18, Math.round(1 + progress * 17 - rand() * 2))),
      kills: Math.floor(progress * rand() * 9),
      deaths: Math.floor(progress * rand() * 7),
      assists: Math.floor(progress * rand() * 10),
      items,
      spells,
      spellKeys: spells.map((s) => SPELL_KEYS[s] ?? s),
    });
  }
  return players;
}

const LABELS: Record<Theme, string[]> = {
  kill: ["Kill", "Double kill", "Triple kill"],
  death: ["Mort"],
  assist: ["Assist"],
  ultimate: ["Ultimate"],
  objective: ["Dragon", "Baron Nashor", "Héraut"],
  turret: ["Tour", "Inhibiteur"],
};

export function buildMockClips(game: GameEntry): Clip[] {
  const rand = rng(game.gameId);
  // « ultimate » en dernier pour ne pas modifier la séquence pseudo-aléatoire des autres clips
  const themes: Theme[] = ["kill", "death", "assist", "objective", "turret", "ultimate"];
  const counts: Record<Theme, number> = {
    kill: Math.max(1, Math.min(6, game.kills)),
    death: Math.max(1, Math.min(5, game.deaths)),
    assist: Math.max(1, Math.min(6, Math.round(game.assists / 2))),
    ultimate: 3,
    objective: 3,
    turret: 2,
  };
  const clips: Clip[] = [];
  for (const theme of themes) {
    for (let n = 0; n < counts[theme]; n++) {
      const t = Math.round(120 + rand() * (game.durationSecs - 180));
      const progress = t / game.durationSecs;
      const labels = LABELS[theme];
      // un ultimate fusionné (deux lancers rapprochés)
      const label =
        theme === "ultimate" && n === 1
          ? "Ultimate ×2"
          : (labels[Math.floor(rand() * labels.length)] ?? "Évènement");
      const players = buildPlayers(game, rand, progress);
      const myGold = Math.round(100 + rand() * 1400);
      // le premier clip « objectif » imite une partie enregistrée avant l'analyse
      const oldStyle = theme === "objective" && n === 0;
      const involved = oldStyle ? [] : mockInvolved(theme, n, players);
      clips.push({
        id: `${game.gameId}-${theme}-${n}`,
        theme,
        label,
        gameTimeSecs: t,
        startSecs: Math.max(0, t - 12),
        endSecs: t + 6,
        file: `C:\\mock\\clips\\${game.gameId}-${theme}-${n}.mp4`,
        players: oldStyle ? players.map((p) => ({ ...p, spellKeys: [] })) : players,
        involved,
        myGold: oldStyle ? null : myGold,
      });
    }
  }
  return clips;
}

const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

export function createMockBackend(): Backend {
  const now = Date.now();
  const games = buildGames(now);
  // sessions dont l'interface du jeu n'est pas reconnue : aucun ultimate généré
  const unrecognized = new Set(["mock-session-7000"]);
  const clipsFor = (game: GameEntry): Clip[] => {
    const clips = buildMockClips(game);
    return game.sessionId && unrecognized.has(game.sessionId)
      ? clips.filter((c) => c.theme !== "ultimate")
      : clips;
  };
  const generated = new Map<string, Clip[]>();
  for (const g of games) {
    if (g.sessionId && g.clipsReady) generated.set(g.sessionId, clipsFor(g));
  }
  let settings: Settings = { ...DEFAULT_SETTINGS };

  const MIB = 1024 ** 2;
  const GIB = 1024 ** 3;
  function usage(): StorageUsage {
    let recordingsBytes = 0;
    let clipsBytes = 0;
    let count = 0;
    for (const g of games) {
      if (!g.sessionId) continue;
      count += 1;
      if (g.hasRecording) recordingsBytes += g.durationSecs * 0.9 * MIB;
      if (g.clipsReady) {
        clipsBytes += (generated.get(g.sessionId)?.length ?? 0) * (settings.compressClips ? 6 : 18) * MIB;
      }
    }
    return {
      totalBytes: Math.round(recordingsBytes + clipsBytes),
      recordingsBytes: Math.round(recordingsBytes),
      clipsBytes: Math.round(clipsBytes),
      games: count,
    };
  }
  // plafond de disque : les vidéos des parties les plus anciennes disparaissent en premier
  function enforceRetention(): void {
    if (settings.retentionGb === null) return;
    const cap = settings.retentionGb * GIB;
    const oldestFirst = games.filter((g) => g.hasRecording).sort((a, b) => a.createdMs - b.createdMs);
    for (const g of oldestFirst) {
      if (usage().totalBytes <= cap) break;
      g.hasRecording = false;
    }
  }
  function ultimateStatus(sessionId: string): UltimateStatus {
    const game = games.find((g) => g.sessionId === sessionId);
    const clips = generated.get(sessionId);
    if (!game || !game.clipsReady || !clips) return { state: "notAnalyzed", casts: 0, hudScale: null };
    if (unrecognized.has(sessionId)) return { state: "unrecognized", casts: 0, hudScale: null };
    const casts = clips
      .filter((c) => c.theme === "ultimate")
      .reduce((sum, c) => sum + (c.label.includes("×2") ? 2 : 1), 0);
    return { state: "ok", casts, hudScale: 1 };
  }

  const offline =
    typeof location !== "undefined" && new URLSearchParams(location.search).get("mock") === "offline";
  const noAccount =
    typeof location !== "undefined" && new URLSearchParams(location.search).get("mock") === "noaccount";

  let status: Status = { state: "idle", recording: false, ffmpegAvailable: true };
  const statusListeners = new Set<(s: Status) => void>();
  const progressListeners = new Set<(p: ClipsProgress) => void>();

  return {
    isMock: true,
    async getAccount(): Promise<Account | null> {
      await delay(250);
      if (offline) throw "Client League of Legends injoignable (mode simulé : ?mock=offline).";
      return noAccount ? null : { riotId: ME_NAME, level: 312 };
    },
    async listGames(count) {
      await delay(300);
      if (offline) throw "Client League of Legends injoignable.";
      return games.slice(0, count).map((g) => ({ ...g }));
    },
    async generateClips(sessionId) {
      const game = games.find((g) => g.sessionId === sessionId);
      if (!game) throw "Session inconnue.";
      if (!game.hasRecording) throw "La vidéo source a été supprimée.";
      const clips = clipsFor(game);
      const total = clips.length;
      for (let done = 0; done <= total; done++) {
        for (const l of progressListeners) l({ sessionId, done, total });
        await delay(120);
      }
      generated.set(sessionId, clips);
      game.clipsReady = true;
      if (settings.deleteSourceAfterClips) game.hasRecording = false;
      enforceRetention();
      return clips;
    },
    async getClips(sessionId) {
      await delay(150);
      return generated.get(sessionId) ?? [];
    },
    async deleteRecording(sessionId) {
      await delay(200);
      const game = games.find((g) => g.sessionId === sessionId);
      if (!game) throw "Session inconnue.";
      game.hasRecording = false;
    },
    async getSettings() {
      return { ...settings };
    },
    async setSettings(next) {
      await delay(150);
      settings = normalizeSettings(next);
      enforceRetention();
      return { ...settings };
    },
    async getStorageUsage() {
      await delay(100);
      return usage();
    },
    async getUltimateStatus(sessionId) {
      await delay(100);
      return ultimateStatus(sessionId);
    },
    async getStatus() {
      return status;
    },
    async onStatusChanged(cb): Promise<Unlisten> {
      statusListeners.add(cb);
      // simule un changement d'état pour la démo
      const timer = setTimeout(() => {
        status = { state: "inGame", recording: true, ffmpegAvailable: true };
        for (const l of statusListeners) l(status);
      }, 6000);
      const timer2 = setTimeout(() => {
        status = { state: "idle", recording: false, ffmpegAvailable: true };
        for (const l of statusListeners) l(status);
      }, 12000);
      return () => {
        clearTimeout(timer);
        clearTimeout(timer2);
        statusListeners.delete(cb);
      };
    },
    async onClipsProgress(cb): Promise<Unlisten> {
      progressListeners.add(cb);
      return () => progressListeners.delete(cb);
    },
    videoSrc: () => null,
  };
}
