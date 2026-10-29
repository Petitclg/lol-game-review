import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createMockBackend } from "./mock";
import type {
  Account,
  Clip,
  ClipsProgress,
  GameEntry,
  Settings,
  Status,
  StorageUsage,
  UltimateStatus,
} from "./types";

export type Unlisten = () => void;

export interface Backend {
  readonly isMock: boolean;
  getAccount(): Promise<Account | null>;
  listGames(count: number): Promise<GameEntry[]>;
  generateClips(sessionId: string): Promise<Clip[]>;
  getClips(sessionId: string): Promise<Clip[]>;
  deleteRecording(sessionId: string): Promise<void>;
  getStatus(): Promise<Status>;
  getSettings(): Promise<Settings>;
  setSettings(settings: Settings): Promise<Settings>;
  getStorageUsage(): Promise<StorageUsage>;
  getUltimateStatus(sessionId: string): Promise<UltimateStatus>;
  onStatusChanged(cb: (status: Status) => void): Promise<Unlisten>;
  onClipsProgress(cb: (progress: ClipsProgress) => void): Promise<Unlisten>;
  /** URL lisible par <video>, ou null si le fichier n'est pas lisible (mode simulé). */
  videoSrc(path: string): string | null;
}

/** Les erreurs du backend sont des chaînes ; on normalise tout en message lisible. */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

export function hasTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function createTauriBackend(): Backend {
  return {
    isMock: false,
    getAccount: () => invoke<Account | null>("get_account"),
    listGames: (count) => invoke<GameEntry[]>("list_games", { count }),
    generateClips: (sessionId) => invoke<Clip[]>("generate_clips", { sessionId }),
    getClips: (sessionId) => invoke<Clip[]>("get_clips", { sessionId }),
    deleteRecording: (sessionId) => invoke<void>("delete_recording", { sessionId }),
    getStatus: () => invoke<Status>("get_status"),
    getSettings: () => invoke<Settings>("get_settings"),
    setSettings: (settings) => invoke<Settings>("set_settings", { settings }),
    getStorageUsage: () => invoke<StorageUsage>("get_storage_usage"),
    getUltimateStatus: (sessionId) => invoke<UltimateStatus>("get_ultimate_status", { sessionId }),
    onStatusChanged: (cb) => listen<Status>("status-changed", (e) => cb(e.payload)),
    onClipsProgress: (cb) => listen<ClipsProgress>("clips-progress", (e) => cb(e.payload)),
    videoSrc: (path) => convertFileSrc(path),
  };
}

let backend: Backend | null = null;

/** Backend Tauri réel, ou simulé dans un navigateur simple (`npm run dev`). */
export function getBackend(): Backend {
  backend ??= hasTauri() ? createTauriBackend() : createMockBackend();
  return backend;
}
