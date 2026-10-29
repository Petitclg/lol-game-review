import { errorMessage, type Backend } from "../api";
import { el } from "../dom";
import {
  DEFAULT_RETENTION_GB,
  MIN_RETENTION_GB,
  RETENTION_HELP,
  formToSettings,
  formatBytes,
  normalizeSettings,
  normalizeUsage,
  settingsToForm,
} from "../settings";
import type { Settings, StorageUsage } from "../types";

export interface SettingsPanelHandlers {
  /** Appelé après un enregistrement réussi (les vidéos ont pu être supprimées). */
  onSaved(): void;
}

export interface SettingsPanel {
  el: HTMLElement;
  open(opener?: HTMLElement | null): void;
  close(): void;
  isOpen(): boolean;
  refreshUsage(): Promise<void>;
}

const FOCUSABLE = "button:not([disabled]), input:not([disabled]), [href], select:not([disabled])";

export function createSettingsPanel(backend: Backend, handlers: SettingsPanelHandlers): SettingsPanel {
  let opener: HTMLElement | null = null;
  let open = false;

  const closeIcon = el("button", {
    class: "btn btn-small modal-close",
    text: "✕",
    attrs: { type: "button", "aria-label": "Fermer" },
    on: { click: () => close() },
  });

  // Utilisation
  const usageValues = {
    total: el("dd", { text: "—" }),
    recordings: el("dd", { text: "—" }),
    clips: el("dd", { text: "—" }),
    games: el("dd", { text: "—" }),
  };
  const usageStatus = el("p", { class: "muted small", attrs: { "aria-live": "polite" } });
  const usageList = el("dl", { class: "usage-list" }, [
    el("dt", { text: "Total" }),
    usageValues.total,
    el("dt", { text: "Vidéos sources" }),
    usageValues.recordings,
    el("dt", { text: "Clips" }),
    usageValues.clips,
    el("dt", { text: "Parties stockées" }),
    usageValues.games,
  ]);

  // Formulaire
  const deleteSource = el("input", { attrs: { type: "checkbox", id: "set-delete-source" } });
  const compress = el("input", { attrs: { type: "checkbox", id: "set-compress" } });
  const limit = el("input", { attrs: { type: "checkbox", id: "set-limit" } });
  const retention = el("input", {
    class: "retention-input",
    attrs: {
      type: "number",
      id: "set-retention",
      min: MIN_RETENTION_GB,
      step: "any",
      value: DEFAULT_RETENTION_GB,
      "aria-describedby": "set-retention-help",
    },
  });
  const message = el("p", { class: "form-message", attrs: { "aria-live": "polite" } });
  const saveBtn = el("button", { class: "btn btn-primary", text: "Enregistrer", attrs: { type: "submit" } });
  const cancelBtn = el("button", {
    class: "btn",
    text: "Fermer",
    attrs: { type: "button" },
    on: { click: () => close() },
  });

  const form = el("form", { class: "settings-form", attrs: { novalidate: true } }, [
    el("h3", { text: "Réglages" }),
    el("label", { class: "check" }, [deleteSource, "Supprimer la vidéo source après la génération des clips"]),
    el("label", { class: "check" }, [compress, "Compresser les clips (gain de place, effectué hors partie)"]),
    el("fieldset", { class: "retention" }, [
      el("legend", { text: "Plafond de disque" }),
      el("label", { class: "check" }, [limit, "Limiter"]),
      el("label", { class: "retention-row", attrs: { for: "set-retention" } }, [
        "Plafond (Go) ",
        retention,
        " Go",
      ]),
      el("p", { class: "muted small", text: RETENTION_HELP, attrs: { id: "set-retention-help" } }),
    ]),
    message,
    el("div", { class: "modal-actions" }, [saveBtn, cancelBtn]),
  ]);

  const modal = el(
    "div",
    {
      class: "modal",
      attrs: { role: "dialog", "aria-modal": "true", "aria-labelledby": "settings-title", tabindex: -1 },
    },
    [
      el("div", { class: "modal-head" }, [el("h2", { text: "Stockage", attrs: { id: "settings-title" } }), closeIcon]),
      el("section", { class: "usage", attrs: { "aria-label": "Utilisation du disque" } }, [
        el("h3", { text: "Utilisation du disque" }),
        usageList,
        usageStatus,
      ]),
      form,
    ],
  );

  const overlay = el("div", { class: "modal-overlay", attrs: { hidden: true } }, [modal]);

  function syncLimit(): void {
    retention.disabled = !limit.checked;
  }
  limit.addEventListener("change", syncLimit);

  function setMessage(text: string, kind: "error" | "ok" | ""): void {
    message.textContent = text;
    message.className = `form-message${kind ? ` form-${kind}` : ""}`;
    if (kind === "error") message.setAttribute("role", "alert");
    else message.removeAttribute("role");
  }

  function applySettings(s: Settings): void {
    const f = settingsToForm(s);
    deleteSource.checked = f.deleteSourceAfterClips;
    compress.checked = f.compressClips;
    limit.checked = f.limit;
    retention.value = f.retentionText;
    syncLimit();
  }

  function showUsage(u: StorageUsage): void {
    usageValues.total.textContent = formatBytes(u.totalBytes);
    usageValues.recordings.textContent = formatBytes(u.recordingsBytes);
    usageValues.clips.textContent = formatBytes(u.clipsBytes);
    usageValues.games.textContent = String(u.games);
    usageStatus.textContent = "";
  }

  async function refreshUsage(): Promise<void> {
    try {
      showUsage(normalizeUsage(await backend.getStorageUsage()));
    } catch (err) {
      usageStatus.textContent = `Utilisation du disque indisponible : ${errorMessage(err)}`;
    }
  }

  async function loadSettings(): Promise<void> {
    try {
      applySettings(normalizeSettings(await backend.getSettings()));
    } catch (err) {
      setMessage(`Impossible de charger les réglages : ${errorMessage(err)}`, "error");
    }
  }

  form.addEventListener("submit", (e) => {
    e.preventDefault();
    void save();
  });

  async function save(): Promise<void> {
    const parsed = formToSettings({
      deleteSourceAfterClips: deleteSource.checked,
      compressClips: compress.checked,
      limit: limit.checked,
      retentionText: retention.value,
    });
    if (!parsed.ok) {
      setMessage(parsed.error, "error");
      retention.focus();
      return;
    }
    saveBtn.disabled = true;
    setMessage("Enregistrement…", "");
    try {
      const saved = normalizeSettings(await backend.setSettings(parsed.value));
      applySettings(saved);
      const adjusted =
        saved.retentionGb !== parsed.value.retentionGb && saved.retentionGb !== null
          ? ` (plafond ajusté à ${String(saved.retentionGb).replace(".", ",")} Go)`
          : "";
      setMessage(`Réglages enregistrés${adjusted}.`, "ok");
      await refreshUsage();
      handlers.onSaved();
    } catch (err) {
      setMessage(`Enregistrement impossible : ${errorMessage(err)}`, "error");
    } finally {
      saveBtn.disabled = false;
    }
  }

  function close(): void {
    if (!open) return;
    open = false;
    overlay.hidden = true;
    const target = opener;
    opener = null;
    target?.focus();
  }

  overlay.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
      return;
    }
    if (e.key !== "Tab") return;
    const items = [...modal.querySelectorAll<HTMLElement>(FOCUSABLE)];
    const first = items[0];
    const last = items[items.length - 1];
    if (!first || !last) return;
    const active = document.activeElement;
    if (e.shiftKey && (active === first || active === modal)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  });
  overlay.addEventListener("mousedown", (e) => {
    if (e.target === overlay) close();
  });

  return {
    el: overlay,
    open(from) {
      if (open) return;
      open = true;
      opener = from ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
      overlay.hidden = false;
      setMessage("", "");
      closeIcon.focus();
      void loadSettings();
      void refreshUsage();
    },
    close,
    isOpen: () => open,
    refreshUsage,
  };
}
