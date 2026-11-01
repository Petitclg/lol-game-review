import { describe, expect, it, vi } from "vitest";
import {
  createStore,
  currentClip,
  initialState,
  reducer,
  selectedGame,
  visibleClips,
  type State,
} from "./store";
import type { GameEntry } from "../types";
import { SAMPLE_CLIPS } from "../testUtils";

function game(gameId: number, sessionId: string | null, clipsReady = false): GameEntry {
  return {
    gameId,
    createdMs: 0,
    durationSecs: 1800,
    queueId: 420,
    championId: 103,
    kills: 1,
    deaths: 2,
    assists: 3,
    win: true,
    sessionId,
    clipsReady,
    hasRecording: sessionId !== null,
  };
}

function withGameSelected(): State {
  let s = initialState();
  s = reducer(s, { type: "gamesLoaded", games: [game(1, "s1"), game(2, null)] });
  s = reducer(s, { type: "selectGame", gameId: 1 });
  return s;
}

function withClips(): State {
  return reducer(withGameSelected(), { type: "clipsLoaded", sessionId: "s1", clips: SAMPLE_CLIPS });
}

describe("compte et statut", () => {
  it("accountLoaded / accountFailed", () => {
    let s = reducer(initialState(), { type: "accountFailed", error: "éteint" });
    expect(s).toMatchObject({ accountLoaded: true, account: null, accountError: "éteint" });
    s = reducer(s, { type: "accountLoaded", account: { riotId: "A#B", level: 5 } });
    expect(s).toMatchObject({ account: { riotId: "A#B", level: 5 }, accountError: null });
  });
  it("statusChanged", () => {
    const status = { state: "inGame", recording: true, ffmpegAvailable: false } as const;
    expect(reducer(initialState(), { type: "statusChanged", status }).status).toEqual(status);
  });
});

describe("parties", () => {
  it("gamesLoaded réinitialise la sélection si la partie disparaît", () => {
    let s = withClips();
    s = reducer(s, { type: "gamesLoaded", games: [game(9, null)] });
    expect(s.selectedGameId).toBeNull();
    expect(s.clips).toEqual([]);
  });
  it("gamesLoaded conserve la sélection et les clips sinon", () => {
    let s = withClips();
    s = reducer(s, { type: "gamesLoaded", games: [game(1, "s1", true)] });
    expect(s.selectedGameId).toBe(1);
    expect(s.clips).toHaveLength(6);
  });
  it("selectGame remet à zéro clips, thème et sélection", () => {
    let s = withClips();
    s = reducer(s, { type: "setTheme", theme: "kill" });
    s = reducer(s, { type: "selectGame", gameId: 2 });
    expect(s).toMatchObject({ selectedGameId: 2, clips: [], theme: "all", selectedClipId: null });
    expect(selectedGame(s)?.gameId).toBe(2);
  });
  it("selectGame identique → même état", () => {
    const s = withGameSelected();
    expect(reducer(s, { type: "selectGame", gameId: 1 })).toBe(s);
  });
});

describe("clips", () => {
  it("clipsLoaded sélectionne le premier clip chronologique", () => {
    const s = withClips();
    expect(s.selectedClipId).toBe("a");
    expect(currentClip(s)?.id).toBe("a");
  });
  it("ignore un chargement tardif d'une autre session", () => {
    const s = withGameSelected();
    expect(reducer(s, { type: "clipsLoaded", sessionId: "autre", clips: SAMPLE_CLIPS })).toBe(s);
  });
  it("setTheme filtre et recale la sélection", () => {
    let s = withClips();
    s = reducer(s, { type: "selectClip", clipId: "e" });
    s = reducer(s, { type: "setTheme", theme: "kill" });
    expect(visibleClips(s).map((c) => c.id)).toEqual(["a", "b"]);
    expect(s.selectedClipId).toBe("a");
  });
  it("setTheme conserve la sélection si elle reste visible", () => {
    let s = withClips();
    s = reducer(s, { type: "selectClip", clipId: "b" });
    s = reducer(s, { type: "setTheme", theme: "kill" });
    expect(s.selectedClipId).toBe("b");
  });
  it("stepClip avance/recule et reste borné", () => {
    let s = withClips();
    s = reducer(s, { type: "stepClip", delta: -1 });
    expect(s.selectedClipId).toBe("a");
    s = reducer(s, { type: "stepClip", delta: 1 });
    expect(s.selectedClipId).toBe("f");
    for (let i = 0; i < 10; i++) s = reducer(s, { type: "stepClip", delta: 1 });
    expect(s.selectedClipId).toBe("e");
  });
  it("stepClip sans clip → inchangé", () => {
    const s = withGameSelected();
    expect(reducer(s, { type: "stepClip", delta: 1 })).toBe(s);
  });
});

describe("génération", () => {
  it("progression puis fin : clips chargés et partie marquée prête", () => {
    let s = withGameSelected();
    s = reducer(s, { type: "generateStarted", sessionId: "s1" });
    expect(s.generating).toEqual({ sessionId: "s1", done: 0, total: 0 });
    s = reducer(s, { type: "generateProgress", progress: { sessionId: "s1", done: 2, total: 6 } });
    expect(s.generating).toEqual({ sessionId: "s1", done: 2, total: 6 });
    s = reducer(s, { type: "generateDone", sessionId: "s1", clips: SAMPLE_CLIPS });
    expect(s.generating).toBeNull();
    expect(s.games.find((g) => g.gameId === 1)?.clipsReady).toBe(true);
    expect(s.clips).toHaveLength(6);
  });
  it("ignore la progression d'une autre session", () => {
    let s = withGameSelected();
    s = reducer(s, { type: "generateStarted", sessionId: "s1" });
    const next = reducer(s, { type: "generateProgress", progress: { sessionId: "x", done: 1, total: 2 } });
    expect(next).toBe(s);
  });
  it("échec : erreur mémorisée, génération stoppée", () => {
    let s = reducer(withGameSelected(), { type: "generateStarted", sessionId: "s1" });
    s = reducer(s, { type: "generateFailed", error: "ffmpeg" });
    expect(s).toMatchObject({ generating: null, generateError: "ffmpeg" });
  });
  it("fin de génération d'une autre partie ne touche pas aux clips affichés", () => {
    let s = withClips();
    s = reducer(s, { type: "generateDone", sessionId: "s-autre", clips: [] });
    expect(s.clips).toHaveLength(6);
  });
  it("génération automatique : la progression d'une génération non lancée par la fenêtre est suivie", () => {
    let s = withGameSelected();
    s = reducer(s, { type: "generateProgress", progress: { sessionId: "s9", done: 1, total: 5 } });
    expect(s.generating).toEqual({ sessionId: "s9", done: 1, total: 5 });
    s = reducer(s, { type: "generateProgress", progress: { sessionId: "s9", done: 3, total: 5 } });
    expect(s.generating?.done).toBe(3);
    s = reducer(s, { type: "generationEnded", sessionId: "s9" });
    expect(s.generating).toBeNull();
  });
  it("le dernier évènement d'une génération terminée ne la rouvre pas", () => {
    const s = withGameSelected();
    const late = reducer(s, { type: "generateProgress", progress: { sessionId: "s1", done: 6, total: 6 } });
    expect(late).toBe(s);
  });
  it("generationEnded ignore une autre session", () => {
    const s = reducer(withGameSelected(), { type: "generateStarted", sessionId: "s1" });
    expect(reducer(s, { type: "generationEnded", sessionId: "autre" })).toBe(s);
  });
});

describe("statut des ultimates", () => {
  const status = { state: "unrecognized", casts: 0, hudScale: null } as const;
  it("stocké pour la partie sélectionnée, ignoré pour une autre session", () => {
    const s = withGameSelected();
    const loaded = reducer(s, { type: "ultimateLoaded", sessionId: "s1", status });
    expect(loaded.ultimate).toEqual({ sessionId: "s1", status });
    expect(reducer(s, { type: "ultimateLoaded", sessionId: "autre", status })).toBe(s);
  });
  it("status null efface ; réinitialisé quand on change de partie", () => {
    let s = reducer(withGameSelected(), { type: "ultimateLoaded", sessionId: "s1", status });
    expect(reducer(s, { type: "ultimateLoaded", sessionId: "s1", status: null }).ultimate).toBeNull();
    s = reducer(s, { type: "selectGame", gameId: 2 });
    expect(s.ultimate).toBeNull();
  });
  it("réinitialisé si la partie disparaît de la liste", () => {
    let s = reducer(withGameSelected(), { type: "ultimateLoaded", sessionId: "s1", status });
    s = reducer(s, { type: "gamesLoaded", games: [game(9, null)] });
    expect(s.ultimate).toBeNull();
  });
});

describe("hasRecording venant du backend", () => {
  it("gamesLoaded remplace l'état de la vidéo source, sans mémoire locale", () => {
    let s = withGameSelected();
    expect(s.games[0]?.hasRecording).toBe(true);
    s = reducer(s, { type: "gamesLoaded", games: [{ ...game(1, "s1", true), hasRecording: false }] });
    expect(s.games[0]?.hasRecording).toBe(false);
    expect(s.selectedGameId).toBe(1);
  });
});

describe("notice", () => {
  it("notice", () => {
    const s = reducer(initialState(), { type: "notice", message: "ok" });
    expect(s.notice).toBe("ok");
  });
});

describe("annotations", () => {
  const greed = { tags: ["greed"] };

  it("annotationsLoaded remplace les annotations de la partie sélectionnée uniquement", () => {
    const s = withGameSelected();
    expect(reducer(s, { type: "annotationsLoaded", sessionId: "s1", annotations: { "a-1.0": greed } }).annotations).toEqual({
      "a-1.0": greed,
    });
    expect(reducer(s, { type: "annotationsLoaded", sessionId: "autre", annotations: { "a-1.0": greed } })).toBe(s);
  });

  it("annotationSaved ajoute, remplace, puis retire une annotation vide", () => {
    let s = withGameSelected();
    s = reducer(s, { type: "annotationSaved", sessionId: "s1", partId: "death-5.0", annotation: greed });
    s = reducer(s, { type: "annotationSaved", sessionId: "s1", partId: "death-5.0", annotation: { tags: ["custom:x"] } });
    expect(s.annotations).toEqual({ "death-5.0": { tags: ["custom:x"] } });
    s = reducer(s, { type: "annotationSaved", sessionId: "s1", partId: "death-5.0", annotation: { tags: [] } });
    expect(s.annotations).toEqual({});
  });

  it("customTagsLoaded remplace la liste des tags personnalisés, quelle que soit la partie", () => {
    expect(initialState().customTags).toEqual([]);
    let s = reducer(initialState(), { type: "customTagsLoaded", labels: ["mal placé", "side lane"] });
    expect(s.customTags).toEqual(["mal placé", "side lane"]);
    s = reducer(reducer(s, { type: "selectGame", gameId: 1 }), { type: "customTagsLoaded", labels: ["side lane"] });
    expect(s.customTags).toEqual(["side lane"]);
    expect(reducer(withGameSelected(), { type: "selectGame", gameId: 2 }).customTags).toEqual([]);
    const kept = reducer(initialState(), { type: "customTagsLoaded", labels: ["a"] });
    expect(reducer({ ...kept, games: [game(1, "s1")] }, { type: "selectGame", gameId: 1 }).customTags).toEqual(["a"]);
  });

  it("annotationSaved d'une autre partie est ignoré", () => {
    const s = withGameSelected();
    expect(reducer(s, { type: "annotationSaved", sessionId: "autre", partId: "k-1.0", annotation: greed })).toBe(s);
  });

  it("changer de partie ou voir la partie disparaître efface les annotations", () => {
    let s = reducer(withGameSelected(), { type: "annotationSaved", sessionId: "s1", partId: "k-1.0", annotation: greed });
    expect(reducer(s, { type: "selectGame", gameId: 2 }).annotations).toEqual({});
    s = reducer(s, { type: "gamesLoaded", games: [game(9, "s9")] });
    expect(s.annotations).toEqual({});
  });
});

describe("createStore", () => {
  it("notifie uniquement lors d'un changement d'état", () => {
    const store = createStore();
    const listener = vi.fn();
    const off = store.subscribe(listener);
    store.dispatch({ type: "gamesLoading" });
    expect(listener).toHaveBeenCalledTimes(1);
    store.dispatch({ type: "notice", message: "x" });
    store.dispatch({ type: "notice", message: "x" });
    expect(listener).toHaveBeenCalledTimes(2);
    off();
    store.dispatch({ type: "notice", message: "m" });
    expect(listener).toHaveBeenCalledTimes(2);
    expect(store.getState().notice).toBe("m");
  });
});
