// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { initialState, reducer, type State } from "../state/store";
import { makeClip } from "../testUtils";
import type { Annotations, Clip, GameEntry } from "../types";
import { createAnnotationPanel } from "./annotationPanel";

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
  hasRecording: true,
};

const MERGED = makeClip("seq-0", "death", 150, {
  themes: ["death", "kill"],
  label: "Kill + Mort",
  parts: [
    { id: "kill-150.0", theme: "kill", label: "Kill", gameTimeSecs: 150 },
    { id: "death-160.0", theme: "death", label: "Mort", gameTimeSecs: 160 },
  ],
});
const OTHER = makeClip("kill-1", "kill", 400);

function stateWith(clips: Clip[], annotations: Annotations = {}, selected?: string): State {
  let s = reducer(initialState(), { type: "gamesLoaded", games: [GAME] });
  s = reducer(s, { type: "selectGame", gameId: 1 });
  s = reducer(s, { type: "clipsLoaded", sessionId: "s1", clips });
  s = reducer(s, { type: "annotationsLoaded", sessionId: "s1", annotations });
  return selected ? reducer(s, { type: "selectClip", clipId: selected }) : s;
}

function mount() {
  const onChange = vi.fn();
  const panel = createAnnotationPanel({ onChange });
  document.body.appendChild(panel.el);
  return { panel, onChange };
}

const chip = (root: HTMLElement, part: string, tag: string) =>
  root.querySelector<HTMLButtonElement>(`[data-part="${part}"] [data-tag="${tag}"]`) as HTMLButtonElement;

afterEach(() => {
  document.body.replaceChildren();
});

describe("panneau d'annotations", () => {
  it("est masqué sans clip, sans partie ou sans vidéo analysée", () => {
    const { panel } = mount();
    panel.update(initialState());
    expect(panel.el.hidden).toBe(true);
    panel.update(stateWith([MERGED]));
    expect(panel.el.hidden).toBe(false);
  });

  it("propose un bloc par moment, avec les étiquettes de son thème", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED]));
    const parts = panel.el.querySelectorAll<HTMLElement>(".annotation-part");
    expect([...parts].map((p) => p.dataset.part)).toEqual(["kill-150.0", "death-160.0"]);
    expect(chip(panel.el, "death-160.0", "face_check").textContent).toBe("Face-check");
    expect(chip(panel.el, "kill-150.0", "face_check")).toBeNull();
    expect(chip(panel.el, "kill-150.0", "good_outplay")).not.toBeNull();
    expect(chip(panel.el, "kill-150.0", "bad_teamfight")).not.toBeNull();
  });

  it("un ancien clip sans moments est annoté comme un moment unique", () => {
    const { panel, onChange } = mount();
    panel.update(stateWith([makeClip("death-0", "death", 160.04)]));
    chip(panel.el, "death-160.0", "greed").click();
    expect(onChange).toHaveBeenCalledWith("death-160.0", ["greed"], "");
  });

  it("reflète les annotations enregistrées", () => {
    const { panel } = mount();
    panel.update(
      stateWith([MERGED], { "death-160.0": { tags: ["greed", "no_vision"], note: "mal placé" } }),
    );
    expect(chip(panel.el, "death-160.0", "greed").getAttribute("aria-pressed")).toBe("true");
    expect(chip(panel.el, "death-160.0", "no_vision").getAttribute("aria-pressed")).toBe("true");
    expect(chip(panel.el, "death-160.0", "face_check").getAttribute("aria-pressed")).toBe("false");
    expect(panel.el.querySelector<HTMLTextAreaElement>('[data-part="death-160.0"] textarea')?.value).toBe(
      "mal placé",
    );
    expect(panel.el.querySelector<HTMLTextAreaElement>('[data-part="kill-150.0"] textarea')?.value).toBe("");
  });

  it("une étiquette se pose et se retire, en gardant les autres et la note en cours de saisie", () => {
    const { panel, onChange } = mount();
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["greed"], note: "" } }));
    const note = panel.el.querySelector<HTMLTextAreaElement>('[data-part="death-160.0"] textarea') as HTMLTextAreaElement;
    note.value = "pas encore enregistrée";
    chip(panel.el, "death-160.0", "face_check").click();
    expect(onChange).toHaveBeenLastCalledWith("death-160.0", ["greed", "face_check"], "pas encore enregistrée");
    chip(panel.el, "death-160.0", "greed").click();
    expect(onChange).toHaveBeenLastCalledWith("death-160.0", [], "pas encore enregistrée");
  });

  it("la note est enregistrée avec les étiquettes du moment quand le champ change", () => {
    const { panel, onChange } = mount();
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["greed"], note: "" } }));
    const note = panel.el.querySelector<HTMLTextAreaElement>('[data-part="death-160.0"] textarea') as HTMLTextAreaElement;
    note.value = "à revoir";
    note.dispatchEvent(new Event("change"));
    expect(onChange).toHaveBeenCalledWith("death-160.0", ["greed"], "à revoir");
  });

  it("une mise à jour du même clip ne reconstruit pas le panneau et ne touche pas à une saisie en cours", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED]));
    const first = panel.el.querySelector(".annotation-part");
    const note = panel.el.querySelector<HTMLTextAreaElement>('[data-part="kill-150.0"] textarea') as HTMLTextAreaElement;
    note.focus();
    note.value = "en train d'écrire";
    panel.update(stateWith([MERGED], { "kill-150.0": { tags: ["good_trade"], note: "" } }));
    expect(panel.el.querySelector(".annotation-part")).toBe(first);
    expect(note.value).toBe("en train d'écrire");
    expect(chip(panel.el, "kill-150.0", "good_trade").getAttribute("aria-pressed")).toBe("true");
  });

  it("changer de clip reconstruit le panneau", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED, OTHER], {}, "seq-0"));
    expect(panel.el.querySelectorAll(".annotation-part")).toHaveLength(2);
    panel.update(stateWith([MERGED, OTHER], {}, "kill-1"));
    const parts = panel.el.querySelectorAll<HTMLElement>(".annotation-part");
    expect([...parts].map((p) => p.dataset.part)).toEqual(["kill-400.0"]);
  });

  it("le texte d'un libellé est inséré comme texte, jamais interprété comme HTML", () => {
    const { panel } = mount();
    const hostile = makeClip("k", "kill", 5, { label: "<img src=x onerror=alert(1)>" });
    panel.update(stateWith([hostile]));
    expect(panel.el.querySelector("img")).toBeNull();
    expect(panel.el.textContent).toContain("<img src=x onerror=alert(1)>");
  });
});
