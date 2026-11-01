import { errorMessage, type Backend } from "./api";
import { customTagId } from "./annotations";
import { createAnnotationsView } from "./components/annotationsView";
import { createClipView } from "./components/clipView";
import { createGameList } from "./components/gameList";
import { createHeader } from "./components/header";
import { createSettingsPanel } from "./components/settingsPanel";
import { createSidePanel } from "./components/sidePanel";
import { getDdragon, getItemPrices, type Ddragon, type ItemPrices } from "./ddragon";
import { el, replaceChildren } from "./dom";
import { createStore, selectedGame, type Store } from "./state/store";
import type { ClipsGenerated } from "./types";

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

  const header = createHeader({
    onOpenSettings: () => settings.open(header.settingsButton),
    onOpenAnnotations: () => showAnnotations(),
  });
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
    onAnnotate: (partId, tags) => annotate(partId, tags),
    onAddCustomTag: (partId, label) => addCustomTag(partId, label),
    onRemoveCustomTag: (label) => removeCustomTag(label),
  });
  const side = createSidePanel();
  const annotationsView = createAnnotationsView(backend, { onClose: () => showGames() });

  const main = el("main", { class: "layout" }, [
    el("div", { class: "col-games" }, [games.el]),
    el("div", { class: "col-clips" }, [clipView.el]),
    el("div", { class: "col-side" }, [side.el]),
  ]);
  replaceChildren(root, [header.el, banner, main, annotationsView.el, settings.el]);
  if (backend.isMock) {
    header.el
      .querySelector(".header-right")
      ?.prepend(el("span", { class: "badge badge-recorded", text: "Mode simulé" }));
  }

  function showAnnotations(): void {
    main.hidden = true;
    header.annotationsButton.setAttribute("aria-pressed", "true");
    void annotationsView.open();
  }

  function showGames(): void {
    annotationsView.close();
    main.hidden = false;
    header.annotationsButton.setAttribute("aria-pressed", "false");
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
      void loadAnnotations(sessionId);
    } catch (err) {
      store.dispatch({ type: "clipsFailed", error: errorMessage(err) });
    }
  }

  async function loadAnnotations(sessionId: string): Promise<void> {
    try {
      const annotations = await backend.getAnnotations(sessionId);
      store.dispatch({ type: "annotationsLoaded", sessionId, annotations });
    } catch (err) {
      store.dispatch({ type: "notice", message: `Annotations illisibles : ${errorMessage(err)}` });
    }
  }

  // Les enregistrements partent dans l'ordre des clics : deux appels simultanés pourraient
  // être traités dans le désordre et laisser sur le disque l'ancienne version.
  let saveQueue: Promise<void> = Promise.resolve();

  function annotate(partId: string, tags: string[]): void {
    const sessionId = selectedGame(store.getState())?.sessionId;
    if (!sessionId) return;
    store.dispatch({ type: "annotationSaved", sessionId, partId, annotation: { tags } });
    enqueue(sessionId, () => backend.setAnnotation(sessionId, partId, tags).then(() => undefined));
  }

  /** Crée le tag (ou retrouve celui qui existe) puis le pose sur le moment. */
  function addCustomTag(partId: string, label: string): void {
    const sessionId = selectedGame(store.getState())?.sessionId;
    if (!sessionId) return;
    enqueue(sessionId, async () => {
      const added = await backend.addCustomTag(label);
      store.dispatch({ type: "customTagsLoaded", labels: added.labels });
      const id = customTagId(added.label);
      const current = store.getState().annotations[partId]?.tags ?? [];
      const tags = current.includes(id) ? current : [...current, id];
      store.dispatch({ type: "annotationSaved", sessionId, partId, annotation: { tags } });
      await backend.setAnnotation(sessionId, partId, tags);
    });
  }

  /** Cesse de proposer le tag ; les moments qui le portent le gardent. */
  function removeCustomTag(label: string): void {
    const sessionId = selectedGame(store.getState())?.sessionId ?? "";
    enqueue(sessionId, async () => {
      const labels = await backend.removeCustomTag(label);
      store.dispatch({ type: "customTagsLoaded", labels });
    });
  }

  function enqueue(sessionId: string, job: () => Promise<void>): void {
    saveQueue = saveQueue.then(job).catch((err: unknown) => {
      store.dispatch({
        type: "notice",
        message: `Annotation non enregistrée : ${errorMessage(err)}`,
      });
      void loadCustomTags();
      return sessionId ? loadAnnotations(sessionId) : undefined;
    });
  }

  async function loadCustomTags(): Promise<void> {
    try {
      store.dispatch({ type: "customTagsLoaded", labels: await backend.getCustomTags() });
    } catch {
      // information secondaire : la liste reste celle qu'on avait
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

  /** Une partie vient d'être générée automatiquement : on montre ses clips. */
  async function onAutoGenerated({ sessionId, error }: ClipsGenerated): Promise<void> {
    store.dispatch({ type: "generationEnded", sessionId });
    if (error) {
      store.dispatch({ type: "notice", message: `Génération automatique des clips échouée : ${error}` });
    }
    await Promise.all([refreshGames(), settings.refreshUsage()]);
    if (selectedGame(store.getState())?.sessionId === sessionId) await loadClips();
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
    annotationsView.setDdragon(d);
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
    .onClipsGenerated((done) => void onAutoGenerated(done))
    .then((u) => unlisteners.push(u))
    .catch(() => undefined);
  backend
    .getStatus()
    .then((status) => store.dispatch({ type: "statusChanged", status }))
    .catch(() => undefined);
  backend
    .getVersion()
    .then((version) => header.setVersion(version))
    .catch(() => undefined);

  void refresh();
  void loadCustomTags();

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
