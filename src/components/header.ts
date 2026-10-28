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
  update(state: State): void;
}

export function createHeader(): Header {
  const title = el("h1", { class: "app-title", text: "LoL Game Review" });
  const account = el("div", { class: "account" });
  const pill = el("span", { class: "status-pill", attrs: { role: "status" } });
  const warning = el("span", { class: "badge badge-warn", attrs: { hidden: true } });
  const right = el("div", { class: "header-right" }, [pill, warning, account]);
  const root = el("header", { class: "app-header" }, [title, right]);

  return {
    el: root,
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
