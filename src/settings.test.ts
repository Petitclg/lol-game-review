import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS,
  canDeleteRecording,
  canGenerate,
  formToSettings,
  formatBytes,
  hasSource,
  normalizeSettings,
  normalizeUsage,
  parseRetentionInput,
  settingsToForm,
  shouldShowUltimateWarning,
} from "./settings";

describe("formatBytes", () => {
  it("Go avec virgule décimale française", () => {
    expect(formatBytes(1.2 * 1024 ** 3)).toBe("1,2 Go");
    expect(formatBytes(20 * 1024 ** 3)).toBe("20 Go");
    expect(formatBytes(1536 * 1024 ** 2)).toBe("1,5 Go");
  });
  it("Mo, Ko et octets", () => {
    expect(formatBytes(512 * 1024 ** 2)).toBe("512 Mo");
    expect(formatBytes(1.25 * 1024 ** 2)).toBe("1,3 Mo");
    expect(formatBytes(12 * 1024)).toBe("12 Ko");
    expect(formatBytes(999)).toBe("999 o");
    expect(formatBytes(0)).toBe("0 o");
  });
  it("seuils exacts et valeurs invalides", () => {
    expect(formatBytes(1024 ** 3)).toBe("1 Go");
    expect(formatBytes(1024 ** 2)).toBe("1 Mo");
    expect(formatBytes(-5)).toBe("0 o");
    expect(formatBytes(NaN)).toBe("0 o");
  });
});

describe("normalizeSettings", () => {
  it("conserve des réglages valides, null = illimité", () => {
    expect(
      normalizeSettings({
        deleteSourceAfterClips: false,
        compressClips: false,
        retentionGb: null,
        makeRoomForRecording: false,
      }),
    ).toEqual({
      deleteSourceAfterClips: false,
      compressClips: false,
      retentionGb: null,
      makeRoomForRecording: false,
    });
  });
  it("champs manquants ou invalides → défauts", () => {
    expect(normalizeSettings(undefined)).toEqual(DEFAULT_SETTINGS);
    expect(normalizeSettings({ compressClips: "oui", retentionGb: "20" })).toEqual(DEFAULT_SETTINGS);
    expect(normalizeSettings({ retentionGb: NaN }).retentionGb).toBe(20);
  });
  it("« faire de la place pour la partie en cours » est activé par défaut", () => {
    expect(DEFAULT_SETTINGS.makeRoomForRecording).toBe(true);
    // un backend plus ancien ne renvoie pas le champ
    expect(normalizeSettings({ deleteSourceAfterClips: true }).makeRoomForRecording).toBe(true);
    expect(normalizeSettings({ makeRoomForRecording: "non" }).makeRoomForRecording).toBe(true);
    expect(normalizeSettings({ makeRoomForRecording: false }).makeRoomForRecording).toBe(false);
  });
  it("plafond borné à 1 Go minimum et arrondi à 0,1", () => {
    expect(normalizeSettings({ retentionGb: 0.2 }).retentionGb).toBe(1);
    expect(normalizeSettings({ retentionGb: -4 }).retentionGb).toBe(1);
    expect(normalizeSettings({ retentionGb: 12.34 }).retentionGb).toBe(12.3);
  });
});

describe("parseRetentionInput", () => {
  it("accepte point et virgule", () => {
    expect(parseRetentionInput("20")).toEqual({ ok: true, value: 20 });
    expect(parseRetentionInput(" 7,5 ")).toEqual({ ok: true, value: 7.5 });
    expect(parseRetentionInput("2.25")).toEqual({ ok: true, value: 2.3 });
  });
  it("refuse vide, non numérique, < 1, trop grand", () => {
    expect(parseRetentionInput("")).toMatchObject({ ok: false });
    expect(parseRetentionInput("abc")).toMatchObject({ ok: false });
    expect(parseRetentionInput("-3")).toMatchObject({ ok: false });
    expect(parseRetentionInput("1e3")).toMatchObject({ ok: false });
    expect(parseRetentionInput("0")).toMatchObject({ ok: false, error: "Le plafond doit être d'au moins 1 Go." });
    expect(parseRetentionInput("0,5")).toMatchObject({ ok: false });
    expect(parseRetentionInput("1000000")).toMatchObject({ ok: false });
  });
});

describe("formulaire ↔ réglages", () => {
  const base = { deleteSourceAfterClips: true, compressClips: false, makeRoomForRecording: true };
  it("Limiter décoché ⇒ retentionGb null, même avec une saisie invalide", () => {
    expect(formToSettings({ ...base, limit: false, retentionText: "zzz" })).toEqual({
      ok: true,
      value: { deleteSourceAfterClips: true, compressClips: false, retentionGb: null, makeRoomForRecording: true },
    });
  });
  it("Limiter coché valide la saisie", () => {
    expect(
      formToSettings({
        deleteSourceAfterClips: false,
        compressClips: true,
        makeRoomForRecording: false,
        limit: true,
        retentionText: "30",
      }),
    ).toEqual({
      ok: true,
      value: { deleteSourceAfterClips: false, compressClips: true, retentionGb: 30, makeRoomForRecording: false },
    });
    expect(formToSettings({ ...base, limit: true, retentionText: "0" })).toMatchObject({ ok: false });
  });
  it("settingsToForm : illimité → case décochée, valeur par défaut 20 conservée", () => {
    expect(settingsToForm({ ...DEFAULT_SETTINGS, retentionGb: null })).toMatchObject({ limit: false, retentionText: "20" });
    expect(settingsToForm({ ...DEFAULT_SETTINGS, retentionGb: 7.5 })).toMatchObject({ limit: true, retentionText: "7.5" });
  });
  it("le réglage « faire de la place » traverse le formulaire dans les deux sens", () => {
    expect(settingsToForm({ ...DEFAULT_SETTINGS, makeRoomForRecording: false }).makeRoomForRecording).toBe(false);
    const parsed = formToSettings({ ...base, makeRoomForRecording: false, limit: false, retentionText: "20" });
    expect(parsed.ok && parsed.value.makeRoomForRecording).toBe(false);
  });
});

describe("normalizeUsage", () => {
  it("valeurs invalides ramenées à 0", () => {
    expect(normalizeUsage({ totalBytes: 10, recordingsBytes: -1, clipsBytes: "x", games: 2.9 })).toEqual({
      totalBytes: 10,
      recordingsBytes: 0,
      clipsBytes: 0,
      games: 2,
    });
    expect(normalizeUsage(null).totalBytes).toBe(0);
  });
});

describe("hasRecording : gating", () => {
  it("génération et suppression exigent une session ET la vidéo source", () => {
    expect(canGenerate({ sessionId: "s", hasRecording: true })).toBe(true);
    expect(canGenerate({ sessionId: "s", hasRecording: false })).toBe(false);
    expect(canGenerate({ sessionId: null, hasRecording: true })).toBe(false);
    expect(canDeleteRecording({ sessionId: "s", hasRecording: false })).toBe(false);
    expect(canDeleteRecording({ sessionId: "s", hasRecording: true })).toBe(true);
  });
  it("champ absent (ancien backend) ⇒ vidéo présente", () => {
    expect(hasSource({} as { hasRecording: boolean })).toBe(true);
  });
});

describe("shouldShowUltimateWarning", () => {
  it("visible seulement pour « unrecognized »", () => {
    expect(shouldShowUltimateWarning({ state: "unrecognized", casts: 0, hudScale: null })).toBe(true);
    expect(shouldShowUltimateWarning({ state: "ok", casts: 0, hudScale: 1 })).toBe(false);
    expect(shouldShowUltimateWarning({ state: "ok", casts: 4, hudScale: 1 })).toBe(false);
    expect(shouldShowUltimateWarning({ state: "notAnalyzed", casts: 0, hudScale: null })).toBe(false);
    expect(shouldShowUltimateWarning(null)).toBe(false);
    expect(shouldShowUltimateWarning(undefined)).toBe(false);
  });
});
