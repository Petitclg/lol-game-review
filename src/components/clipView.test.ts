// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Backend } from "../api";
import { initialState, reducer, type State } from "../state/store";
import { makeClip } from "../testUtils";
import type { Clip, GameEntry } from "../types";
import { createClipView } from "./clipView";

const GAME: GameEntry = {
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
};

function stateWith(clips: Clip[]): State {
  let s = initialState();
  s = reducer(s, { type: "gamesLoaded", games: [GAME] });
  s = reducer(s, { type: "selectGame", gameId: 1 });
  return reducer(s, { type: "clipsLoaded", sessionId: "s1", clips });
}

function setup(clips: Clip[], videoSrc: (p: string) => string | null = (p) => `asset://${p}`) {
  const backend = { isMock: false, videoSrc } as unknown as Backend;
  const view = createClipView(backend, {
    onGenerate: vi.fn(),
    onDeleteRecording: vi.fn(),
    onReloadClips: vi.fn(),
    onTheme: vi.fn(),
    onSelectClip: vi.fn(),
    onStep: vi.fn(),
  });
  document.body.appendChild(view.el);
  view.update(stateWith(clips));
  const video = view.el.querySelector("video") as HTMLVideoElement;
  const errBox = view.el.querySelector<HTMLElement>(".video-error") as HTMLElement;
  return { view, video, errBox };
}

function failVideo(video: HTMLVideoElement, code: number, message: string): void {
  Object.defineProperty(video, "error", { configurable: true, value: { code, message } });
  video.dispatchEvent(new Event("error"));
}

describe("lecteur vidéo : erreurs", () => {
  let load: ReturnType<typeof vi.fn>;
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    load = vi.fn();
    vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(load as () => void);
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
    consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
  });
  afterEach(() => {
    document.body.replaceChildren();
    vi.restoreAllMocks();
  });

  it("charge la source du clip sans erreur affichée", () => {
    const { video, errBox } = setup([makeClip("a", "kill", 100, { file: "C:\\clips\\a.mp4" })]);
    expect(video.getAttribute("src")).toBe("asset://C:\\clips\\a.mp4");
    expect(errBox.hidden).toBe(true);
  });

  it("événement error : message français, code, chemin du fichier et console.error", () => {
    const { video, errBox } = setup([makeClip("a", "kill", 100, { file: "C:\\clips\\a.mp4" })]);
    failVideo(video, 4, "DEMUXER_ERROR_COULD_NOT_OPEN");
    expect(errBox.hidden).toBe(false);
    expect(errBox.textContent).toContain("Impossible de lire ce clip :");
    expect(errBox.textContent).toContain("code 4");
    expect(errBox.textContent).toContain("DEMUXER_ERROR_COULD_NOT_OPEN");
    expect(errBox.textContent).toContain("C:\\clips\\a.mp4");
    expect(consoleError).toHaveBeenCalled();
  });

  it("MediaError absente : code 0 et raison générique", () => {
    const { video, errBox } = setup([makeClip("a", "kill", 100, { file: "C:\\clips\\a.mp4" })]);
    Object.defineProperty(video, "error", { configurable: true, value: null });
    video.dispatchEvent(new Event("error"));
    expect(errBox.textContent).toContain("code 0");
    expect(errBox.textContent).toContain("erreur de lecture inconnue");
  });

  it("« Réessayer » recharge la source et masque l'erreur", () => {
    const { video, errBox } = setup([makeClip("a", "kill", 100, { file: "C:\\clips\\a.mp4" })]);
    failVideo(video, 2, "");
    load.mockClear();
    errBox.querySelector("button")?.click();
    expect(errBox.hidden).toBe(true);
    expect(load).toHaveBeenCalledTimes(1);
    expect(video.getAttribute("src")).toBe("asset://C:\\clips\\a.mp4");
  });

  it("l'erreur disparaît quand on change de clip", () => {
    const clips = [
      makeClip("a", "kill", 100, { file: "C:\\clips\\a.mp4" }),
      makeClip("b", "kill", 200, { file: "C:\\clips\\b.mp4" }),
    ];
    const { view, video, errBox } = setup(clips);
    failVideo(video, 3, "");
    expect(errBox.hidden).toBe(false);
    view.update(reducer(stateWith(clips), { type: "selectClip", clipId: "b" }));
    expect(errBox.hidden).toBe(true);
    expect(video.getAttribute("src")).toBe("asset://C:\\clips\\b.mp4");
  });

  it("file null : « Clip non généré », pas de source ni d'erreur", () => {
    const { view, video, errBox } = setup([makeClip("a", "kill", 100, { file: null })]);
    const msg = view.el.querySelector<HTMLElement>(".video-msg");
    expect(msg?.hidden).toBe(false);
    expect(msg?.textContent).toContain("Clip non généré");
    expect(video.hidden).toBe(true);
    expect(video.hasAttribute("src")).toBe(false);
    // un évènement error parasite sans source est ignoré
    video.dispatchEvent(new Event("error"));
    expect(errBox.hidden).toBe(true);
  });

  it("source illisible (mode simulé) : message d'aperçu indisponible", () => {
    const { view } = setup([makeClip("a", "kill", 100, { file: "C:\\x.mp4" })], () => null);
    expect(view.el.querySelector(".video-msg")?.textContent).toContain("mode simulé");
  });

  it("rend le chemin via textContent, sans interpréter le HTML", () => {
    const evil = '<img src=x onerror="alert(1)">.mp4';
    const { view, video } = setup([makeClip("a", "kill", 100, { file: evil })]);
    failVideo(video, 4, "<b>x</b>");
    expect(view.el.querySelector(".video-error img")).toBeNull();
    expect(view.el.querySelector(".video-error b")).toBeNull();
    expect(view.el.querySelector(".video-error")?.textContent).toContain(evil);
  });
});
