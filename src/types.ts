export type Theme =
  | "kill"
  | "death"
  | "assist"
  | "ultimate"
  | "flash"
  | "key_moment"
  | "objective"
  | "turret";

export interface Account {
  riotId: string;
  level: number;
}

export interface GameEntry {
  gameId: number;
  createdMs: number;
  durationSecs: number;
  queueId: number;
  championId: number;
  kills: number;
  deaths: number;
  assists: number;
  win: boolean;
  sessionId: string | null;
  clipsReady: boolean;
  /** La vidéo source existe encore ; absent traité comme vrai (anciens backends). */
  hasRecording: boolean;
}

export interface Settings {
  deleteSourceAfterClips: boolean;
  compressClips: boolean;
  /** Plafond de disque en Go ; null = illimité. */
  retentionGb: number | null;
  /** Supprimer les données les plus anciennes pour pouvoir enregistrer la partie qui démarre. */
  makeRoomForRecording: boolean;
  autoGenerateClips: boolean;
}

export interface StorageUsage {
  totalBytes: number;
  recordingsBytes: number;
  clipsBytes: number;
  games: number;
}

export interface UltimateStatus {
  state: "ok" | "unrecognized" | "notAnalyzed";
  casts: number;
  hudScale: number | null;
}

export interface ItemInfo {
  id: number;
  name: string;
}

export interface PlayerSnapshot {
  name: string;
  champion: string;
  team: "ORDER" | "CHAOS";
  isMe: boolean;
  level: number;
  kills: number;
  deaths: number;
  assists: number;
  items: ItemInfo[];
  spells: string[];
  /** Identifiants stables des sorts (ex. "SummonerFlash") ; absent ou [] pour les anciennes parties. */
  spellKeys?: string[];
}

export type Role = "killer" | "victim" | "assist" | "caster";

export interface Involved {
  name: string;
  roles: Role[];
}

export interface Clip {
  id: string;
  theme: Theme;
  label: string;
  gameTimeSecs: number;
  startSecs: number;
  endSecs: number;
  file: string | null;
  players: PlayerSnapshot[];
  /** Joueurs impliqués ; absent ou [] pour les anciens clips. */
  involved?: Involved[];
  /** Or non dépensé du joueur local ; absent ou null si non enregistré. */
  myGold?: number | null;
  /**
   * Tous les thèmes du clip, `theme` en premier : plusieurs quand des clips de thèmes
   * différents ont été fusionnés. Absent ou [] pour les anciens clips (voir `clipThemes`).
   */
  themes?: Theme[];
  /** Moments montrés par le clip, dans l'ordre ; les annotations s'y rattachent. */
  parts?: ClipPart[];
}

/** Un moment montré par un clip. Son identifiant ne change pas quand les clips sont régénérés. */
export interface ClipPart {
  id: string;
  theme: Theme;
  label: string;
  gameTimeSecs: number;
}

/** Étiquettes (identifiants du catalogue) et note libre d'un moment. */
export interface Annotation {
  tags: string[];
  note: string;
}

/** Annotations d'une partie, par identifiant de moment ; un moment sans annotation est absent. */
export type Annotations = Record<string, Annotation>;

export interface Status {
  state: "idle" | "inGame";
  recording: boolean;
  ffmpegAvailable: boolean;
}

export interface ClipsProgress {
  sessionId: string;
  done: number;
  total: number;
}

/** Fin d'une génération automatique ; `error` est null quand les clips sont générés. */
export interface ClipsGenerated {
  sessionId: string;
  error: string | null;
}
