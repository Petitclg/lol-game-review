export type Theme = "kill" | "death" | "assist" | "objective" | "turret";

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
}

export interface AbilityRanks {
  q: number;
  w: number;
  e: number;
  r: number;
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
  abilityRanks: AbilityRanks | null;
  abilityHaste: number | null;
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
