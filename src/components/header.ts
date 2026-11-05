import { accountKey, accountOptions } from "../accounts";
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
  annotationsButton: HTMLButtonElement;
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

export function createHeader(
  handlers: {
    onOpenSettings?: () => void;
    onOpenAnnotations?: () => void;
    onSelectAccount?: (key: string) => void;
  } = {},
): Header {
  const title = el("h1", { class: "app-title", text: "LoL Game Review" });
  const versionEl = el("span", {
    class: "app-version muted",
    attrs: { hidden: true, title: "Version de l'application" },
  });
  const brand = el("div", { class: "app-brand" }, [title, versionEl]);
  const accountName = el("span", { class: "account-name" });
  const accountSelect = el("select", {
    class: "account-select",
    attrs: { "aria-label": "Compte dont on voit les parties", title: "Compte dont on voit les parties" },
    on: { change: () => handlers.onSelectAccount?.(accountSelect.value) },
  });
  const accountLevel = el("span", { class: "account-level" });
  const account = el("div", { class: "account" }, [accountName, accountSelect, accountLevel]);
  let accountSignature = "";

  /** Le menu n'est reconstruit que si son contenu change : une liste ouverte doit rester ouverte. */
  function renderAccount(state: State): void {
    const connected = state.account?.riotId ?? null;
    const options = accountOptions(state.games, connected);
    const signature = JSON.stringify([
      options,
      state.selectedAccount,
      connected,
      state.account?.level,
      state.accountLoaded,
      state.accountError,
    ]);
    if (signature === accountSignature) return;
    accountSignature = signature;

    const several = options.length > 1;
    accountSelect.hidden = !several;
    accountName.hidden = several;
    if (several) {
      const connectedKey = accountKey(connected);
      replaceChildren(
        accountSelect,
        options.map((o) =>
          el("option", {
            text: o.key === connectedKey ? `${o.label} (connecté)` : o.label,
            attrs: { value: o.key, selected: o.key === state.selectedAccount },
          }),
        ),
      );
      accountSelect.value = state.selectedAccount ?? "";
    } else if (options[0]) {
      accountName.className = "account-name";
      accountName.textContent = options[0].label;
    } else {
      accountName.className = "account-name muted";
      accountName.textContent = !state.accountLoaded
        ? "Chargement…"
        : state.accountError
          ? "Client LoL éteint"
          : "Aucun compte connecté";
    }

    const showLevel =
      state.account !== null && state.selectedAccount === accountKey(state.account.riotId);
    accountLevel.hidden = !showLevel;
    accountLevel.textContent = showLevel && state.account ? `Niv. ${state.account.level}` : "";
  }
  const pill = el("span", { class: "status-pill", attrs: { role: "status" } });
  const warning = el("span", { class: "badge badge-warn", attrs: { hidden: true } });
  const settingsButton = el("button", {
    class: "btn btn-small settings-btn",
    text: "⚙ Stockage",
    attrs: { type: "button", "aria-haspopup": "dialog", title: "Réglages de stockage" },
    on: { click: () => handlers.onOpenSettings?.() },
  });
  const annotationsButton = el("button", {
    class: "btn btn-small annotations-btn",
    text: "✎ Annotations",
    attrs: { type: "button", title: "Revoir mes moments annotés, toutes parties confondues" },
    on: { click: () => handlers.onOpenAnnotations?.() },
  });
  const right = el("div", { class: "header-right" }, [pill, warning, account, annotationsButton, settingsButton]);
  const root = el("header", { class: "app-header" }, [brand, right]);

  return {
    el: root,
    settingsButton,
    annotationsButton,
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

      renderAccount(state);
    },
  };
}
