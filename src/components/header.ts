import { el, replaceChildren } from "../dom";
import type { State } from "../state/store";
import type { Status } from "../types";

export function statusLabel(status: Status | null): { text: string; kind: "idle" | "ingame" | "recording" | "unknown" } {
  if (!status) return { text: "Statut inconnu", kind: "unknown" };
  if (status.recording) return { text: "Enregistrement en cours", kind: "recording" };
  if (status.state === "inGame") return { text: "En partie", kind: "ingame" };
  return { text: "Inactif", kind: "idle" };
}

export interface Header {
  el: HTMLElement;
  settingsButton: HTMLButtonElement;
  update(state: State): void;
  /** Affiche la version de l'application à côté du titre ; vide ou absente : masquée. */
  setVersion(version: string | null | undefined): void;
}

/** « 0.1.9 » → « v0.1.9 » ; chaîne vide ou absente → null (rien à afficher). */
export function versionLabel(version: string | null | undefined): string | null {
  const v = version?.trim();
  if (!v) return null;
  return /^v/i.test(v) ? v : `v${v}`;
}

export function createHeader(handlers: { onOpenSettings?: () => void } = {}): Header {
  const title = el("h1", { class: "app-title", text: "LoL Game Review" });
  const versionEl = el("span", {
    class: "app-version muted",
    attrs: { hidden: true, title: "Version de l'application" },
  });
  const brand = el("div", { class: "app-brand" }, [title, versionEl]);
  const account = el("div", { class: "account" });
  const pill = el("span", { class: "status-pill", attrs: { role: "status" } });
  const warning = el("span", { class: "badge badge-warn", attrs: { hidden: true } });
  const settingsButton = el("button", {
    class: "btn btn-small settings-btn",
    text: "⚙ Stockage",
    attrs: { type: "button", "aria-haspopup": "dialog", title: "Réglages de stockage" },
    on: { click: () => handlers.onOpenSettings?.() },
  });
  const right = el("div", { class: "header-right" }, [pill, warning, account, settingsButton]);
  const root = el("header", { class: "app-header" }, [brand, right]);

  return {
    el: root,
    settingsButton,
    setVersion(version) {
      const label = versionLabel(version);
      versionEl.hidden = label === null;
      versionEl.textContent = label ?? "";
    },
    update(state) {
      const { text, kind } = statusLabel(state.status);
      pill.className = `status-pill status-${kind}`;
      replaceChildren(pill, [el("span", { class: "dot", attrs: { "aria-hidden": true } }), text]);

      const noFfmpeg = state.status !== null && !state.status.ffmpegAvailable;
      warning.hidden = !noFfmpeg;
      warning.textContent = noFfmpeg ? "⚠ FFmpeg introuvable : génération de clips impossible" : "";

      if (state.account) {
        replaceChildren(account, [
          el("span", { class: "account-name", text: state.account.riotId }),
          el("span", { class: "account-level", text: `Niv. ${state.account.level}` }),
        ]);
      } else if (state.accountLoaded) {
        replaceChildren(account, [
          el("span", {
            class: "account-name muted",
            text: state.accountError ? "Client LoL éteint" : "Aucun compte connecté",
          }),
        ]);
      } else {
        replaceChildren(account, [el("span", { class: "account-name muted", text: "Chargement…" })]);
      }
    },
  };
}
