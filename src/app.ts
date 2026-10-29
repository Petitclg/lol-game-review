import { errorMessage, type Backend } from "./api";
import { createClipView } from "./components/clipView";
import { createGameList } from "./components/gameList";
import { createHeader } from "./components/header";
import { createSettingsPanel } from "./components/settingsPanel";
import { createSidePanel } from "./components/sidePanel";
import { getDdragon, getItemPrices, type Ddragon, type ItemPrices } from "./ddragon";
import { el, replaceChildren } from "./dom";
import { createStore, selectedGame, type Store } from "./state/store";

const GAME_COUNT = 20;

export interface App {
  store: Store;
  refresh(): Promise<void>;
  destroy(): void;
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable
  );
}

export function createApp(root: HTMLElement, backend: Backend): App {
  const store = createStore();
  let dd: Ddragon | null = null;
  let prices: ItemPrices | null = null;
  const unlisteners: Array<() => void> = [];

  const header = createHeader({ onOpenSettings: () => settings.open(header.settingsButton) });
  const settings = createSettingsPanel(backend, {
    onSaved: () => {
      // le plafond a pu supprimer des vidéos sources
      void refreshGames();
    },
  });
  const banner = el("div", { class: "banner", attrs: { "aria-live": "polite" } });
  const games = createGameList({
    onSelect: (gameId) => selectGame(gameId),
    onRefresh: () => void refreshGames(),
  });
  const clipView = createClipView(backend, {
    onGenerate: () => void generate(),
    onDeleteRecording: () => void deleteRecording(),
    onReloadClips: () => void loadClips(),
    onTheme: (theme) => store.dispatch({ type: "setTheme", theme }),
    onSelectClip: (clipId) => store.dispatch({ type: "selectClip", clipId }),
    onStep: (delta) => store.dispatch({ type: "stepClip", delta }),
  });
  const side = createSidePanel();

  const main = el("main", { class: "layout" }, [
    el("div", { class: "col-games" }, [games.el]),
    el("div", { class: "col-clips" }, [clipView.el]),
    el("div", { class: "col-side" }, [side.el]),
  ]);
  replaceChildren(root, [header.el, banner, main, settings.el]);
  if (backend.isMock) {
    header.el
      .querySelector(".header-right")
      ?.prepend(el("span", { class: "badge badge-recorded", text: "Mode simulé" }));
  }

  function render(): void {
    const state = store.getState();
    header.update(state);
    renderBanner();
    games.update(state, dd);
    clipView.update(state);
    side.update(state, dd, prices);
  }

  function renderBanner(): void {
    const state = store.getState();
    if (state.accountError) {
      replaceChildren(banner, [
        el("div", { class: "alert alert-error", attrs: { role: "alert" } }, [
          "Le client League of Legends semble éteint ou injoignable. Lancez-le puis réessayez. ",
          el("span", { class: "muted small", text: `(${state.accountError})` }),
          " ",
          el("button", {
            class: "btn btn-small",
            text: "Réessayer",
            attrs: { type: "button" },
            on: { click: () => void refresh() },
          }),
        ]),
      ]);
    } else if (state.accountLoaded && !state.account) {
      replaceChildren(banner, [
        el("div", { class: "alert alert-info" }, [
          "Aucun compte n'est connecté dans le client League of Legends. Connectez-vous puis ",
          el("button", {
            class: "btn btn-small",
            text: "réessayez",
            attrs: { type: "button" },
            on: { click: () => void refresh() },
          }),
          ".",
        ]),
      ]);
    } else if (state.gamesError) {
      replaceChildren(banner, [
        el("div", { class: "alert alert-error", attrs: { role: "alert" } }, [
          `Impossible de charger les parties : ${state.gamesError} `,
          el("button", {
            class: "btn btn-small",
            text: "Réessayer",
            attrs: { type: "button" },
            on: { click: () => void refreshGames() },
          }),
        ]),
      ]);
    } else {
      replaceChildren(banner, []);
    }
  }

  async function refreshGames(): Promise<void> {
    store.dispatch({ type: "gamesLoading" });
    try {
      const list = await backend.listGames(GAME_COUNT);
      store.dispatch({ type: "gamesLoaded", games: list });
    } catch (err) {
      store.dispatch({ type: "gamesFailed", error: errorMessage(err) });
    }
  }

  async function refresh(): Promise<void> {
    try {
      const account = await backend.getAccount();
      store.dispatch({ type: "accountLoaded", account });
      if (account) await refreshGames();
    } catch (err) {
      store.dispatch({ type: "accountFailed", error: errorMessage(err) });
    }
  }

  async function loadClips(): Promise<void> {
    const game = selectedGame(store.getState());
    if (!game?.sessionId) return;
    const sessionId = game.sessionId;
    store.dispatch({ type: "clipsLoading" });
    try {
      const clips = await backend.getClips(sessionId);
      store.dispatch({ type: "clipsLoaded", sessionId, clips });
      void loadUltimate(sessionId);
    } catch (err) {
      store.dispatch({ type: "clipsFailed", error: errorMessage(err) });
    }
  }

  async function loadUltimate(sessionId: string): Promise<void> {
    try {
      const status = await backend.getUltimateStatus(sessionId);
      store.dispatch({ type: "ultimateLoaded", sessionId, status });
    } catch {
      // information secondaire : pas d'avertissement si le statut est indisponible
      store.dispatch({ type: "ultimateLoaded", sessionId, status: null });
    }
  }

  function selectGame(gameId: number): void {
    store.dispatch({ type: "selectGame", gameId });
    void loadClips();
  }

  async function generate(): Promise<void> {
    const game = selectedGame(store.getState());
    if (!game?.sessionId || store.getState().generating) return;
    const sessionId = game.sessionId;
    store.dispatch({ type: "generateStarted", sessionId });
    try {
      const clips = await backend.generateClips(sessionId);
      store.dispatch({ type: "generateDone", sessionId, clips });
      // l'état de la vidéo source vient du backend (suppression automatique, plafond…)
      await Promise.all([refreshGames(), loadUltimate(sessionId), settings.refreshUsage()]);
    } catch (err) {
      store.dispatch({ type: "generateFailed", error: errorMessage(err) });
    }
  }

  async function deleteRecording(): Promise<void> {
    const game = selectedGame(store.getState());
    if (!game?.sessionId) return;
    const sessionId = game.sessionId;
    const ok = window.confirm(
      "Supprimer la vidéo source de cette partie ?\n\nLes clips déjà générés sont conservés, mais vous ne pourrez plus en générer de nouveaux.",
    );
    if (!ok) return;
    try {
      await backend.deleteRecording(sessionId);
      store.dispatch({ type: "notice", message: "Vidéo source supprimée. Les clips sont conservés." });
      await Promise.all([refreshGames(), loadUltimate(sessionId), settings.refreshUsage()]);
    } catch (err) {
      store.dispatch({ type: "notice", message: `Suppression impossible : ${errorMessage(err)}` });
    }
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    if (isTypingTarget(e.target) || settings.isOpen()) return;
    e.preventDefault();
    store.dispatch({ type: "stepClip", delta: e.key === "ArrowLeft" ? -1 : 1 });
  }

  store.subscribe(render);
  document.addEventListener("keydown", onKeyDown);
  render();

  void getDdragon().then((d) => {
    dd = d;
    render();
    if (!d) return;
    void getItemPrices(d.version).then((p) => {
      prices = p;
      render();
    });
  });

  backend
    .onStatusChanged((status) => store.dispatch({ type: "statusChanged", status }))
    .then((u) => unlisteners.push(u))
    .catch(() => undefined);
  backend
    .onClipsProgress((progress) => store.dispatch({ type: "generateProgress", progress }))
    .then((u) => unlisteners.push(u))
    .catch(() => undefined);
  backend
    .getStatus()
    .then((status) => store.dispatch({ type: "statusChanged", status }))
    .catch(() => undefined);

  void refresh();

  return {
    store,
    refresh,
    destroy() {
      document.removeEventListener("keydown", onKeyDown);
      settings.close();
      for (const u of unlisteners) u();
    },
  };
}
