import type { GameEntry, Settings, StorageUsage, UltimateStatus } from "./types";

export const DEFAULT_RETENTION_GB = 20;
export const MIN_RETENTION_GB = 1;
export const MAX_RETENTION_GB = 100_000;

export const RETENTION_HELP =
  "Quand le plafond est dépassé, les vidéos des parties les plus anciennes sont supprimées en premier ; les chronologies sont conservées.";

export const ULTIMATE_WARNING =
  "Ultimates : interface du jeu non reconnue dans la vidéo (échelle ou disposition inhabituelle). Aucun ultimate n'a été généré pour cette partie.";

export const SOURCE_DELETED_REASON = "La vidéo source a été supprimée";

export const DEFAULT_SETTINGS: Settings = {
  deleteSourceAfterClips: true,
  compressClips: true,
  retentionGb: DEFAULT_RETENTION_GB,
};

const KIB = 1024;

/** Taille lisible en français : "1,2 Go", "512 Mo", "12 Ko", "0 o" (unités binaires). */
export function formatBytes(bytes: number): string {
  const b = Number.isFinite(bytes) ? Math.max(0, bytes) : 0;
  const fmt = (v: number, unit: string) => {
    const rounded = Math.round(v * 10) / 10;
    return `${String(rounded).replace(".", ",")} ${unit}`;
  };
  if (b >= KIB ** 3) return fmt(b / KIB ** 3, "Go");
  if (b >= KIB ** 2) return fmt(b / KIB ** 2, "Mo");
  if (b >= KIB) return fmt(b / KIB, "Ko");
  return `${Math.round(b)} o`;
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}

function clampRetention(n: number): number {
  const rounded = Math.round(n * 10) / 10;
  return Math.min(MAX_RETENTION_GB, Math.max(MIN_RETENTION_GB, rounded));
}

/** Normalise des réglages reçus du backend (champs manquants → défauts ; plafond borné ; null = illimité). */
export function normalizeSettings(raw: unknown): Settings {
  const r = isRecord(raw) ? raw : {};
  const bool = (v: unknown, fallback: boolean) => (typeof v === "boolean" ? v : fallback);
  let retentionGb: number | null;
  if (r["retentionGb"] === null) retentionGb = null;
  else if (typeof r["retentionGb"] === "number" && Number.isFinite(r["retentionGb"])) {
    retentionGb = clampRetention(r["retentionGb"]);
  } else retentionGb = DEFAULT_SETTINGS.retentionGb;
  return {
    deleteSourceAfterClips: bool(r["deleteSourceAfterClips"], DEFAULT_SETTINGS.deleteSourceAfterClips),
    compressClips: bool(r["compressClips"], DEFAULT_SETTINGS.compressClips),
    retentionGb,
  };
}

export type Parsed<T> = { ok: true; value: T } | { ok: false; error: string };

/** Valide la saisie du plafond (virgule ou point décimal, ≥ 1 Go). */
export function parseRetentionInput(text: string): Parsed<number> {
  const cleaned = text.trim().replace(",", ".");
  if (cleaned === "") return { ok: false, error: "Indiquez un plafond en Go." };
  if (!/^\d+(\.\d+)?$/.test(cleaned)) return { ok: false, error: "Le plafond doit être un nombre." };
  const n = Number(cleaned);
  if (n < MIN_RETENTION_GB) {
    return { ok: false, error: `Le plafond doit être d'au moins ${MIN_RETENTION_GB} Go.` };
  }
  if (n > MAX_RETENTION_GB) return { ok: false, error: "Le plafond est trop élevé." };
  return { ok: true, value: Math.round(n * 10) / 10 };
}

export interface SettingsForm {
  deleteSourceAfterClips: boolean;
  compressClips: boolean;
  limit: boolean;
  retentionText: string;
}

export function settingsToForm(s: Settings): SettingsForm {
  return {
    deleteSourceAfterClips: s.deleteSourceAfterClips,
    compressClips: s.compressClips,
    limit: s.retentionGb !== null,
    retentionText: String(s.retentionGb ?? DEFAULT_RETENTION_GB),
  };
}

/** Formulaire → Settings ; « Limiter » décoché ⇒ retentionGb null. */
export function formToSettings(form: SettingsForm): Parsed<Settings> {
  let retentionGb: number | null = null;
  if (form.limit) {
    const parsed = parseRetentionInput(form.retentionText);
    if (!parsed.ok) return parsed;
    retentionGb = parsed.value;
  }
  return {
    ok: true,
    value: {
      deleteSourceAfterClips: form.deleteSourceAfterClips,
      compressClips: form.compressClips,
      retentionGb,
    },
  };
}

export function normalizeUsage(raw: unknown): StorageUsage {
  const r = isRecord(raw) ? raw : {};
  const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) && v > 0 ? v : 0);
  return {
    totalBytes: num(r["totalBytes"]),
    recordingsBytes: num(r["recordingsBytes"]),
    clipsBytes: num(r["clipsBytes"]),
    games: Math.trunc(num(r["games"])),
  };
}

/** La vidéo source existe-t-elle ? Champ absent (ancien backend) ⇒ oui. */
export function hasSource(game: Pick<GameEntry, "hasRecording">): boolean {
  return game.hasRecording !== false;
}

/** Régénération possible : session enregistrée ET vidéo source encore présente. */
export function canGenerate(game: Pick<GameEntry, "sessionId" | "hasRecording">): boolean {
  return game.sessionId !== null && hasSource(game);
}

export function canDeleteRecording(game: Pick<GameEntry, "sessionId" | "hasRecording">): boolean {
  return canGenerate(game);
}

/** Avertissement affiché seulement quand l'interface du jeu n'a pas été reconnue. */
export function shouldShowUltimateWarning(status: UltimateStatus | null | undefined): boolean {
  return status?.state === "unrecognized";
}
