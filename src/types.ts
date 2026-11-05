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
  /** 0 : file inconnue (partie connue par son seul enregistrement). */
  queueId: number;
  /** 0 : champion inconnu du client, voir `champion`. */
  championId: number;
  /** Identifiant DDragon du champion (ex. "MonkeyKing") ; null ou absent si inconnu. */
  champion?: string | null;
  kills: number;
  deaths: number;
  assists: number;
  /** null : issue inconnue (partie connue par son seul enregistrement, absente de l'historique du client). */
  win: boolean | null;
  /** Riot ID du compte qui a joué ; null ou absent si l'enregistrement ne le dit pas. */
  account?: string | null;
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

/** Étiquettes (identifiants du catalogue ou `custom:texte`) d'un moment. */
export interface Annotation {
  tags: string[];
}

/** Annotations d'une partie, par identifiant de moment ; un moment sans annotation est absent. */
export type Annotations = Record<string, Annotation>;

/** Résultat de l'ajout d'un tag personnalisé : la liste proposée, et le texte à poser sur le moment. */
export interface AddedCustomTag {
  labels: string[];
  label: string;
}

/** Ce que l'historique du client dit d'une partie, gardé avec elle (champion, issue, date). */
export interface GameSummary {
  gameId: number;
  createdMs: number;
  durationSecs: number;
  queueId: number;
  championId: number;
  kills: number;
  deaths: number;
  assists: number;
  win: boolean;
}

/** Un moment annoté, toutes parties confondues. */
export interface AnnotatedMoment {
  sessionId: string;
  partId: string;
  theme: Theme;
  /** Libellé du moment quand son clip est connu, sinon vide. */
  label: string;
  gameTimeSecs: number;
  tags: string[];
  /** Clip montrant le moment (un clip fusionné peut en montrer plusieurs). */
  clipId: string | null;
  /** Fichier du clip s'il existe encore. */
  file: string | null;
  /** null pour une partie qui n'a jamais été listée : champion, année et issue inconnus. */
  game: GameSummary | null;
}

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
