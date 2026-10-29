// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../api";
import type { Settings, StorageUsage } from "../types";
import { createSettingsPanel } from "./settingsPanel";

const GIB = 1024 ** 3;
const USAGE: StorageUsage = {
  totalBytes: 1.2 * GIB,
  recordingsBytes: 1 * GIB,
  clipsBytes: 0.2 * GIB,
  games: 4,
};

const flush = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
};

function setup(overrides: Partial<Backend> = {}, initial: Settings = { deleteSourceAfterClips: true, compressClips: false, retentionGb: 20 }) {
  const getStorageUsage = vi.fn(() => Promise.resolve(USAGE));
  const setSettings = vi.fn((s: Settings) => Promise.resolve(s));
  const backend = {
    getSettings: vi.fn(() => Promise.resolve(initial)),
    setSettings,
    getStorageUsage,
    ...overrides,
  } as unknown as Backend;
  const onSaved = vi.fn();
  const panel = createSettingsPanel(backend, { onSaved });
  const opener = document.createElement("button");
  document.body.append(opener, panel.el);
  const q = <T extends HTMLElement>(sel: string) => panel.el.querySelector<T>(sel) as T;
  return {
    panel,
    opener,
    onSaved,
    setSettings,
    getStorageUsage,
    q,
    deleteSource: () => q<HTMLInputElement>("#set-delete-source"),
    compress: () => q<HTMLInputElement>("#set-compress"),
    limit: () => q<HTMLInputElement>("#set-limit"),
    retention: () => q<HTMLInputElement>("#set-retention"),
    message: () => q<HTMLElement>(".form-message"),
    submit: () => q<HTMLFormElement>("form").requestSubmit(),
  };
}

describe("panneau « Stockage »", () => {
  beforeEach(() => {
    document.body.replaceChildren();
  });
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("fermé au départ, boîte de dialogue accessible une fois ouvert", async () => {
    const ui = setup();
    expect(ui.panel.el.hidden).toBe(true);
    ui.opener.focus();
    ui.panel.open(ui.opener);
    const dialog = ui.q("[role=dialog]");
    expect(ui.panel.el.hidden).toBe(false);
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.getAttribute("aria-labelledby")).toBe("settings-title");
    expect(ui.q("#settings-title").textContent).toBe("Stockage");
    expect(document.activeElement).toBe(ui.q(".modal-close"));
    await flush();
  });

  it("affiche l'utilisation (tailles lisibles, virgule française) et les réglages courants", async () => {
    const ui = setup();
    ui.panel.open(ui.opener);
    await flush();
    const values = [...ui.panel.el.querySelectorAll(".usage-list dd")].map((d) => d.textContent);
    expect(values).toEqual(["1,2 Go", "1 Go", "204,8 Mo", "4"]);
    expect(ui.deleteSource().checked).toBe(true);
    expect(ui.compress().checked).toBe(false);
    expect(ui.limit().checked).toBe(true);
    expect(ui.retention().value).toBe("20");
    expect(ui.retention().disabled).toBe(false);
    expect(ui.panel.el.textContent).toContain(
      "Quand le plafond est dépassé, les vidéos des parties les plus anciennes sont supprimées en premier ; les chronologies sont conservées.",
    );
  });

  it("plafond illimité : « Limiter » décoché et saisie désactivée ; cocher réactive", async () => {
    const ui = setup({}, { deleteSourceAfterClips: false, compressClips: true, retentionGb: null });
    ui.panel.open(ui.opener);
    await flush();
    expect(ui.limit().checked).toBe(false);
    expect(ui.retention().disabled).toBe(true);
    ui.limit().checked = true;
    ui.limit().dispatchEvent(new Event("change"));
    expect(ui.retention().disabled).toBe(false);
  });

  it("Échap ferme et rend le focus au bouton d'ouverture", async () => {
    const ui = setup();
    ui.opener.focus();
    ui.panel.open(ui.opener);
    await flush();
    ui.q(".retention-input").dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(ui.panel.isOpen()).toBe(false);
    expect(ui.panel.el.hidden).toBe(true);
    expect(document.activeElement).toBe(ui.opener);
  });

  it("le bouton Fermer ferme aussi", async () => {
    const ui = setup();
    ui.panel.open(ui.opener);
    await flush();
    ui.q<HTMLButtonElement>(".modal-actions button[type=button]").click();
    expect(ui.panel.isOpen()).toBe(false);
  });

  it("piège le focus : Tab depuis le dernier élément revient au premier", async () => {
    const ui = setup();
    ui.panel.open(ui.opener);
    await flush();
    const buttons = ui.panel.el.querySelectorAll<HTMLElement>("button");
    const last = ui.q<HTMLButtonElement>(".modal-actions button[type=button]");
    last.focus();
    const ev = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    last.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(buttons[0]);
  });

  it("enregistre, affiche la valeur normalisée renvoyée, rafraîchit l'utilisation et notifie", async () => {
    const ui = setup({
      setSettings: vi.fn((s: Settings) => Promise.resolve({ ...s, retentionGb: 12 })),
    } as Partial<Backend>);
    ui.panel.open(ui.opener);
    await flush();
    ui.retention().value = "12,34".replace(",", ".");
    ui.deleteSource().checked = false;
    ui.compress().checked = true;
    const callsBefore = ui.getStorageUsage.mock.calls.length;
    ui.submit();
    await flush();
    expect(ui.retention().value).toBe("12");
    expect(ui.message().textContent).toBe("Réglages enregistrés (plafond ajusté à 12 Go).");
    expect(ui.getStorageUsage.mock.calls.length).toBe(callsBefore + 1);
    expect(ui.onSaved).toHaveBeenCalledTimes(1);
    expect(ui.deleteSource().checked).toBe(false);
  });

  it("transmet les valeurs saisies ; « Limiter » décoché ⇒ retentionGb null", async () => {
    const ui = setup();
    ui.panel.open(ui.opener);
    await flush();
    ui.limit().checked = false;
    ui.limit().dispatchEvent(new Event("change"));
    ui.submit();
    await flush();
    expect(ui.setSettings).toHaveBeenCalledWith({
      deleteSourceAfterClips: true,
      compressClips: false,
      retentionGb: null,
    });
    expect(ui.message().textContent).toBe("Réglages enregistrés.");
  });

  it("saisie invalide : message d'erreur, aucun appel au backend", async () => {
    const ui = setup();
    ui.panel.open(ui.opener);
    await flush();
    ui.retention().value = "0";
    ui.submit();
    await flush();
    expect(ui.setSettings).not.toHaveBeenCalled();
    expect(ui.message().textContent).toBe("Le plafond doit être d'au moins 1 Go.");
    expect(ui.message().getAttribute("role")).toBe("alert");
    expect(ui.onSaved).not.toHaveBeenCalled();
  });

  it("erreur du backend affichée en texte brut", async () => {
    const evil = "<img src=x onerror=alert(1)>";
    const ui = setup({ setSettings: vi.fn(() => Promise.reject(evil)) } as Partial<Backend>);
    ui.panel.open(ui.opener);
    await flush();
    ui.submit();
    await flush();
    expect(ui.message().textContent).toBe(`Enregistrement impossible : ${evil}`);
    expect(ui.panel.el.querySelector("img")).toBeNull();
    expect(ui.onSaved).not.toHaveBeenCalled();
  });

  it("échec du chargement de l'utilisation et des réglages : messages, pas de plantage", async () => {
    const ui = setup({
      getStorageUsage: vi.fn(() => Promise.reject("disque illisible")),
      getSettings: vi.fn(() => Promise.reject("réglages illisibles")),
    } as Partial<Backend>);
    ui.panel.open(ui.opener);
    await flush();
    expect(ui.panel.el.textContent).toContain("Utilisation du disque indisponible : disque illisible");
    expect(ui.message().textContent).toBe("Impossible de charger les réglages : réglages illisibles");
  });

  it("refreshUsage met à jour les valeurs affichées", async () => {
    const usage = vi.fn(() => Promise.resolve(USAGE));
    const ui = setup({ getStorageUsage: usage } as Partial<Backend>);
    ui.panel.open(ui.opener);
    await flush();
    usage.mockResolvedValueOnce({ totalBytes: 3 * GIB, recordingsBytes: 3 * GIB, clipsBytes: 0, games: 1 });
    await ui.panel.refreshUsage();
    expect(ui.panel.el.querySelector(".usage-list dd")?.textContent).toBe("3 Go");
  });
});
