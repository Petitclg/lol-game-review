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

function stateWith(
  clips: Clip[],
  annotations: Annotations = {},
  selected?: string,
  customTags: string[] = [],
): State {
  let s = reducer(initialState(), { type: "gamesLoaded", games: [GAME] });
  s = reducer(s, { type: "selectGame", gameId: 1 });
  s = reducer(s, { type: "clipsLoaded", sessionId: "s1", clips });
  s = reducer(s, { type: "annotationsLoaded", sessionId: "s1", annotations });
  s = reducer(s, { type: "customTagsLoaded", labels: customTags });
  return selected ? reducer(s, { type: "selectClip", clipId: selected }) : s;
}

function mount() {
  const handlers = { onChange: vi.fn(), onAddCustomTag: vi.fn(), onRemoveCustomTag: vi.fn() };
  const panel = createAnnotationPanel(handlers);
  document.body.appendChild(panel.el);
  return { panel, ...handlers };
}

const chip = (root: HTMLElement, part: string, tag: string) =>
  root.querySelector<HTMLButtonElement>(`[data-part="${part}"] [data-tag="${tag}"]`) as HTMLButtonElement;

const input = (root: HTMLElement, part: string) =>
  root.querySelector<HTMLInputElement>(`[data-part="${part}"] .custom-tag-input`) as HTMLInputElement;

const submit = (root: HTMLElement, part: string) =>
  root.querySelector(`[data-part="${part}"] form`)?.dispatchEvent(new Event("submit", { cancelable: true }));

afterEach(() => {
  document.body.replaceChildren();
});

describe("panneau d'annotations : étiquettes du catalogue", () => {
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

  it("n'a plus de champ de note", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED]));
    expect(panel.el.querySelector("textarea")).toBeNull();
  });

  it("un ancien clip sans moments est annoté comme un moment unique", () => {
    const { panel, onChange } = mount();
    panel.update(stateWith([makeClip("death-0", "death", 160.04)]));
    chip(panel.el, "death-160.0", "greed").click();
    expect(onChange).toHaveBeenCalledWith("death-160.0", ["greed"]);
  });

  it("reflète les annotations enregistrées", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["greed", "no_vision"] } }));
    expect(chip(panel.el, "death-160.0", "greed").getAttribute("aria-pressed")).toBe("true");
    expect(chip(panel.el, "death-160.0", "no_vision").getAttribute("aria-pressed")).toBe("true");
    expect(chip(panel.el, "death-160.0", "face_check").getAttribute("aria-pressed")).toBe("false");
    expect(chip(panel.el, "kill-150.0", "good_trade").getAttribute("aria-pressed")).toBe("false");
  });

  it("une étiquette se pose et se retire, en gardant les autres", () => {
    const { panel, onChange } = mount();
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["greed", "custom:à revoir"] } }));
    chip(panel.el, "death-160.0", "face_check").click();
    expect(onChange).toHaveBeenLastCalledWith("death-160.0", ["greed", "custom:à revoir", "face_check"]);
    chip(panel.el, "death-160.0", "greed").click();
    expect(onChange).toHaveBeenLastCalledWith("death-160.0", ["custom:à revoir"]);
  });

  it("changer de clip reconstruit le panneau", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED, OTHER], {}, "seq-0"));
    expect(panel.el.querySelectorAll(".annotation-part")).toHaveLength(2);
    panel.update(stateWith([MERGED, OTHER], {}, "kill-1"));
    const parts = panel.el.querySelectorAll<HTMLElement>(".annotation-part");
    expect([...parts].map((p) => p.dataset.part)).toEqual(["kill-400.0"]);
  });
});

describe("panneau d'annotations : tags personnalisés", () => {
  const custom = (root: HTMLElement, part: string) =>
    [...root.querySelectorAll<HTMLButtonElement>(`[data-part="${part}"] .custom-chip .tag-chip`)].map((c) => c.textContent);

  it("propose les tags personnalisés sur chaque moment, après les étiquettes du catalogue", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED], {}, undefined, ["mal placé", "side lane"]));
    expect(custom(panel.el, "death-160.0")).toEqual(["mal placé", "side lane"]);
    expect(custom(panel.el, "kill-150.0")).toEqual(["mal placé", "side lane"]);
  });

  it("un tag personnalisé se pose et se retire comme les autres, sous sa forme enregistrée", () => {
    const { panel, onChange } = mount();
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["face_check"] } }, undefined, ["mal placé"]));
    const c = chip(panel.el, "death-160.0", "custom:mal placé");
    expect(c.getAttribute("aria-pressed")).toBe("false");
    c.click();
    expect(onChange).toHaveBeenLastCalledWith("death-160.0", ["face_check", "custom:mal placé"]);
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["face_check", "custom:mal placé"] } }, undefined, ["mal placé"]));
    expect(chip(panel.el, "death-160.0", "custom:mal placé").getAttribute("aria-pressed")).toBe("true");
    chip(panel.el, "death-160.0", "custom:mal placé").click();
    expect(onChange).toHaveBeenLastCalledWith("death-160.0", ["face_check"]);
  });

  it("la croix retire le tag de la liste proposée, sans toucher aux moments", () => {
    const { panel, onChange, onRemoveCustomTag } = mount();
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["custom:mal placé"] } }, undefined, ["mal placé"]));
    const cross = panel.el.querySelector<HTMLButtonElement>('[data-part="death-160.0"] .tag-remove') as HTMLButtonElement;
    expect(cross.textContent).toBe("✕");
    expect(cross.getAttribute("aria-label")).toBe("Ne plus proposer le tag mal placé");
    cross.click();
    expect(onRemoveCustomTag).toHaveBeenCalledWith("mal placé");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("un tag retiré de la liste reste visible, sans croix, sur le moment qui le porte", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["custom:ancien"] } }, undefined, []));
    expect(custom(panel.el, "death-160.0")).toEqual(["ancien"]);
    expect(chip(panel.el, "death-160.0", "custom:ancien").getAttribute("aria-pressed")).toBe("true");
    expect(panel.el.querySelector('[data-part="death-160.0"] .tag-remove')).toBeNull();
    expect(custom(panel.el, "kill-150.0")).toEqual([]);
  });

  it("la liste se met à jour quand les tags proposés changent, sans reconstruire le panneau", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED], {}, undefined, ["a"]));
    const first = panel.el.querySelector(".annotation-part");
    panel.update(stateWith([MERGED], {}, undefined, ["a", "b"]));
    expect(panel.el.querySelector(".annotation-part")).toBe(first);
    expect(custom(panel.el, "death-160.0")).toEqual(["a", "b"]);
    panel.update(stateWith([MERGED], {}, undefined, ["b"]));
    expect(custom(panel.el, "death-160.0")).toEqual(["b"]);
  });

  it("ajouter : le texte est nettoyé, le champ vidé et le tag demandé pour ce moment", () => {
    const { panel, onAddCustomTag } = mount();
    panel.update(stateWith([MERGED]));
    input(panel.el, "death-160.0").value = "  mal   placé  ";
    submit(panel.el, "death-160.0");
    expect(onAddCustomTag).toHaveBeenCalledWith("death-160.0", "mal placé");
    expect(input(panel.el, "death-160.0").value).toBe("");
    expect(panel.el.querySelector(".custom-tag-message")?.textContent).toBe("");
  });

  it("ajouter un texte vide ou trop long est refusé avec un message, sans appel", () => {
    const { panel, onAddCustomTag } = mount();
    panel.update(stateWith([MERGED]));
    submit(panel.el, "death-160.0");
    expect(onAddCustomTag).not.toHaveBeenCalled();
    expect(panel.el.querySelector('[data-part="death-160.0"] .custom-tag-message')?.textContent).toBe(
      "Un tag fait de 1 à 40 caractères.",
    );
    input(panel.el, "death-160.0").value = "x".repeat(41);
    submit(panel.el, "death-160.0");
    expect(onAddCustomTag).not.toHaveBeenCalled();
    expect(input(panel.el, "death-160.0").value).toHaveLength(41);
  });

  it("le champ est limité à 40 caractères et la touche Entrée (envoi du formulaire) ajoute le tag", () => {
    const { panel, onAddCustomTag } = mount();
    panel.update(stateWith([MERGED]));
    expect(input(panel.el, "kill-150.0").maxLength).toBe(40);
    input(panel.el, "kill-150.0").value = "outplay";
    const event = new Event("submit", { cancelable: true });
    panel.el.querySelector('[data-part="kill-150.0"] form')?.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(onAddCustomTag).toHaveBeenCalledWith("kill-150.0", "outplay");
  });

  it("une saisie en cours survit à la mise à jour du même clip", () => {
    const { panel } = mount();
    panel.update(stateWith([MERGED], {}, undefined, ["a"]));
    input(panel.el, "death-160.0").value = "en train d'écrire";
    panel.update(stateWith([MERGED], { "death-160.0": { tags: ["greed"] } }, undefined, ["a", "b"]));
    expect(input(panel.el, "death-160.0").value).toBe("en train d'écrire");
  });

  it("le texte d'un tag est inséré comme texte, jamais interprété comme HTML", () => {
    const { panel } = mount();
    panel.update(stateWith([makeClip("k", "kill", 5, { label: "<img src=x onerror=alert(1)>" })], {}, undefined, ["<b>x</b>"]));
    expect(panel.el.querySelector("img")).toBeNull();
    expect(panel.el.querySelector("b")).toBeNull();
    expect(panel.el.textContent).toContain("<img src=x onerror=alert(1)>");
    expect(panel.el.textContent).toContain("<b>x</b>");
  });
});
