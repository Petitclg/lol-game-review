// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../api";
import { initialState, reducer, type State } from "../state/store";
import { makeClip } from "../testUtils";
import type { GameEntry, UltimateStatus } from "../types";
import { createClipView, generateDisabledReason } from "./clipView";

const WARNING =
  "Ultimates : interface du jeu non reconnue dans la vidéo (échelle ou disposition inhabituelle). Aucun ultimate n'a été généré pour cette partie.";

function game(extra: Partial<GameEntry> = {}): GameEntry {
  return {
    gameId: 1,
    createdMs: 0,
    durationSecs: 1800,
    queueId: 420,
    championId: 103,
    kills: 1,
    deaths: 1,
    assists: 1,
    win: true,
    sessionId: "s1",
    clipsReady: true,
    hasRecording: true,
    ...extra,
  };
}

function stateFor(g: GameEntry, ultimate?: UltimateStatus): State {
  let s = initialState();
  s = reducer(s, { type: "gamesLoaded", games: [g] });
  s = reducer(s, { type: "selectGame", gameId: g.gameId });
  s = reducer(s, {
    type: "clipsLoaded",
    sessionId: "s1",
    clips: [makeClip("a", "kill", 100), makeClip("b", "ultimate", 200)],
  });
  if (ultimate) s = reducer(s, { type: "ultimateLoaded", sessionId: "s1", status: ultimate });
  return s;
}

function mount(state: State) {
  const backend = { isMock: false, videoSrc: (p: string) => `asset://${p}` } as unknown as Backend;
  const view = createClipView(backend, {
    onGenerate: vi.fn(),
    onDeleteRecording: vi.fn(),
    onReloadClips: vi.fn(),
    onTheme: vi.fn(),
    onSelectClip: vi.fn(),
    onStep: vi.fn(),
  });
  document.body.appendChild(view.el);
  view.update(state);
  const q = <T extends HTMLElement>(sel: string) => view.el.querySelector<T>(sel);
  return {
    view,
    generate: () => q<HTMLButtonElement>(".toolbar .btn-primary") as HTMLButtonElement,
    del: () => q<HTMLButtonElement>(".toolbar .btn-danger") as HTMLButtonElement,
    warning: () => q<HTMLElement>(".ultimate-warning") as HTMLElement,
  };
}

describe("hasRecording : barre d'outils", () => {
  beforeEach(() => {
    vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  });
  afterEach(() => {
    document.body.replaceChildren();
    vi.restoreAllMocks();
  });

  it("vidéo source présente : génération active, suppression visible", () => {
    const ui = mount(stateFor(game()));
    expect(ui.generate().disabled).toBe(false);
    expect(ui.generate().textContent).toBe("Régénérer les clips");
    expect(ui.del().hidden).toBe(false);
  });

  it("vidéo source supprimée : génération désactivée avec infobulle, suppression masquée, clips visibles", () => {
    const state = stateFor(game({ hasRecording: false }));
    const ui = mount(state);
    expect(ui.generate().disabled).toBe(true);
    expect(ui.generate().title).toBe("La vidéo source a été supprimée");
    expect(ui.del().hidden).toBe(true);
    expect(generateDisabledReason(state)).toBe("La vidéo source a été supprimée");
    expect(ui.view.el.querySelectorAll(".clip-item")).toHaveLength(2);
    expect(ui.view.el.querySelector("video")?.getAttribute("src")).toBe("asset://C:\\clips\\a.mp4");
  });

  it("sans clips générés et sans vidéo : « Générer » reste désactivé", () => {
    const g = game({ hasRecording: false, clipsReady: false });
    let s = reducer(initialState(), { type: "gamesLoaded", games: [g] });
    s = reducer(s, { type: "selectGame", gameId: 1 });
    const ui = mount(s);
    expect(ui.generate().textContent).toBe("Générer les clips");
    expect(ui.generate().disabled).toBe(true);
  });

  it("redevient actif quand le backend renvoie hasRecording: true", () => {
    const ui = mount(stateFor(game({ hasRecording: false })));
    expect(ui.generate().disabled).toBe(true);
    ui.view.update(stateFor(game({ hasRecording: true })));
    expect(ui.generate().disabled).toBe(false);
    expect(ui.del().hidden).toBe(false);
  });
});

describe("bandeau « Ultimates : interface non reconnue »", () => {
  beforeEach(() => {
    vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  });
  afterEach(() => {
    document.body.replaceChildren();
    vi.restoreAllMocks();
  });

  it("visible pour « unrecognized », avec le texte exact, avant les onglets", () => {
    const ui = mount(stateFor(game(), { state: "unrecognized", casts: 0, hudScale: null }));
    expect(ui.warning().hidden).toBe(false);
    expect(ui.warning().textContent).toBe(WARNING);
    const children = [...ui.view.el.children];
    expect(children.indexOf(ui.warning())).toBeLessThan(children.indexOf(ui.view.el.querySelector(".tabs") as Element));
  });

  it.each<UltimateStatus | undefined>([
    { state: "ok", casts: 0, hudScale: 1 },
    { state: "ok", casts: 3, hudScale: 1 },
    { state: "notAnalyzed", casts: 0, hudScale: null },
    undefined,
  ])("masqué pour %j", (status) => {
    const ui = mount(stateFor(game(), status));
    expect(ui.warning().hidden).toBe(true);
  });

  it("masqué quand on change de partie (statut réinitialisé)", () => {
    const state = stateFor(game(), { state: "unrecognized", casts: 0, hudScale: null });
    const ui = mount(state);
    expect(ui.warning().hidden).toBe(false);
    const other = reducer(
      reducer(state, { type: "gamesLoaded", games: [game(), game({ gameId: 2, sessionId: "s2" })] }),
      { type: "selectGame", gameId: 2 },
    );
    ui.view.update(other);
    expect(ui.warning().hidden).toBe(true);
  });
});
