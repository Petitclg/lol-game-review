import { el, remoteImage, replaceChildren } from "../dom";
import { championIconUrl, findChampion, findChampionByKey, type Ddragon } from "../ddragon";
import { formatDuration, formatKda, formatRelativeDate, queueLabel } from "../format";
import { hasSource } from "../settings";
import { visibleGames, type State } from "../state/store";
import type { GameEntry } from "../types";

export const NO_VIDEO_TOOLTIP =
  "Seules les parties jouées pendant que l'application est ouverte sont enregistrées.";

export interface GameListHandlers {
  onSelect(gameId: number): void;
  onRefresh(): void;
}

export interface GameList {
  el: HTMLElement;
  update(state: State, dd: Ddragon | null): void;
}

export function gameBadge(game: GameEntry): { text: string; kind: string } {
  if (!game.sessionId) return { text: "Pas de vidéo", kind: "none" };
  if (game.clipsReady) return { text: "Clips prêts", kind: "ready" };
  if (!hasSource(game)) return { text: "Vidéo supprimée", kind: "none" };
  return { text: "Enregistrée", kind: "recorded" };
}

export function gameOutcome(game: GameEntry): { text: string; kind: "win" | "loss" | "unknown" } {
  if (game.win === true) return { text: "Victoire", kind: "win" };
  if (game.win === false) return { text: "Défaite", kind: "loss" };
  return { text: "Résultat inconnu", kind: "unknown" };
}

function gameItem(game: GameEntry, selected: boolean, dd: Ddragon | null, h: GameListHandlers) {
  const champ =
    findChampionByKey(dd?.champions ?? null, game.championId) ??
    (game.champion ? findChampion(dd?.champions ?? null, game.champion) : null);
  const name = champ?.name ?? game.champion ?? `Champion ${game.championId}`;
  const icon = remoteImage(
    dd && champ ? championIconUrl(dd.version, champ.id) : null,
    "",
    "champ-icon",
  );
  const badge = gameBadge(game);
  const outcome = gameOutcome(game);
  const meta = [
    game.queueId > 0 ? queueLabel(game.queueId) : null,
    formatRelativeDate(game.createdMs),
    formatDuration(game.durationSecs),
  ]
    .filter((part) => part !== null)
    .join(" · ");
  const button = el(
    "button",
    {
      class: `game-item ${outcome.kind}${selected ? " selected" : ""}`,
      attrs: {
        type: "button",
        "aria-pressed": selected,
        "aria-label": `${name}, ${outcome.text.toLowerCase()}, ${formatKda(game.kills, game.deaths, game.assists)}`,
      },
      on: { click: () => h.onSelect(game.gameId) },
    },
    [
      icon,
      el("span", { class: "game-main" }, [
        el("span", { class: "game-title" }, [
          el("span", { class: "champ-name", text: name }),
          el("span", { class: "result", text: outcome.text }),
        ]),
        el("span", { class: "game-kda", text: formatKda(game.kills, game.deaths, game.assists) }),
        el("span", { class: "game-meta", text: meta }),
      ]),
      el("span", {
        class: `badge badge-${badge.kind}`,
        text: badge.text,
        attrs: { title: game.sessionId ? undefined : NO_VIDEO_TOOLTIP },
      }),
    ],
  );
  return el("li", { class: "game-li" }, [button]);
}

export function createGameList(h: GameListHandlers): GameList {
  const refresh = el("button", {
    class: "btn btn-small",
    text: "Actualiser",
    attrs: { type: "button" },
    on: { click: () => h.onRefresh() },
  });
  const heading = el("div", { class: "panel-head" }, [
    el("h2", { text: "Parties récentes", attrs: { id: "games-heading" } }),
    refresh,
  ]);
  const body = el("div", { class: "game-body" });
  const root = el("section", { class: "games-panel", attrs: { "aria-labelledby": "games-heading" } }, [
    heading,
    body,
  ]);

  let last: {
    games: unknown;
    account: string | null;
    selected: number | null;
    loading: boolean;
    error: string | null;
    dd: Ddragon | null;
  } | null = null;

  return {
    el: root,
    update(state, dd) {
      if (
        last &&
        last.games === state.games &&
        last.account === state.selectedAccount &&
        last.selected === state.selectedGameId &&
        last.loading === state.gamesLoading &&
        last.error === state.gamesError &&
        last.dd === dd
      ) {
        return;
      }
      last = {
        games: state.games,
        account: state.selectedAccount,
        selected: state.selectedGameId,
        loading: state.gamesLoading,
        error: state.gamesError,
        dd,
      };
      refresh.disabled = state.gamesLoading;
      const shown = visibleGames(state);
      const hadFocus = body.contains(document.activeElement);
      if (state.gamesLoading && state.games.length === 0) {
        replaceChildren(body, [el("p", { class: "muted", text: "Chargement des parties…" })]);
      } else if (state.gamesError && state.games.length === 0) {
        replaceChildren(body, [el("p", { class: "muted", text: "Impossible de charger les parties." })]);
      } else if (shown.length === 0) {
        replaceChildren(body, [el("p", { class: "muted", text: "Aucune partie récente." })]);
      } else {
        const list = el(
          "ul",
          { class: "game-list" },
          shown.map((g) => gameItem(g, g.gameId === state.selectedGameId, dd, h)),
        );
        replaceChildren(body, [list]);
        if (hadFocus) list.querySelector<HTMLElement>(".game-item.selected")?.focus();
      }
    },
  };
}
