import { THEME_NAMES, clipParts } from "../clips";
import { el } from "../dom";
import { MAX_NOTE_CHARS, tagsFor, toggleTag } from "../annotations";
import { formatDuration } from "../format";
import { currentClip, selectedGame, type State } from "../state/store";
import type { Annotation, Annotations, ClipPart } from "../types";

export interface AnnotationHandlers {
  /** Enregistre l'annotation complète d'un moment. */
  onChange(partId: string, tags: string[], note: string): void;
}

export interface AnnotationPanel {
  el: HTMLElement;
  update(state: State): void;
}

const EMPTY: Annotation = { tags: [], note: "" };

interface PartView {
  el: HTMLElement;
  /** Aligne l'affichage sur les annotations enregistrées, sans casser une saisie en cours. */
  sync(annotations: Annotations): void;
}

function buildPart(part: ClipPart, getLatest: () => Annotations, h: AnnotationHandlers): PartView {
  const current = (): Annotation => getLatest()[part.id] ?? EMPTY;
  const note = el("textarea", {
    class: "annotation-note",
    attrs: {
      rows: 2,
      maxlength: MAX_NOTE_CHARS,
      placeholder: "Note libre : ce que vous referiez autrement…",
      "aria-label": `Note sur ${part.label}`,
    },
    on: { change: () => h.onChange(part.id, current().tags, note.value) },
  });

  const chips = tagsFor(part.theme).map((tag) => {
    const chip = el("button", {
      class: `tag-chip tone-${tag.tone}`,
      text: tag.label,
      attrs: { type: "button", "aria-pressed": "false", "data-tag": tag.id },
      on: { click: () => h.onChange(part.id, toggleTag(current().tags, tag.id), note.value) },
    });
    return chip;
  });

  const heading = el("h4", { class: "annotation-part-title" }, [
    el("span", { class: `annotation-theme theme-${part.theme}`, text: THEME_NAMES[part.theme] ?? part.theme }),
    ` ${part.label} — ${formatDuration(part.gameTimeSecs)}`,
  ]);
  const node = el("div", { class: "annotation-part", attrs: { "data-part": part.id } }, [
    heading,
    el("div", { class: "tag-chips", attrs: { role: "group", "aria-label": `Étiquettes : ${part.label}` } }, chips),
    note,
  ]);

  return {
    el: node,
    sync(annotations) {
      const a = annotations[part.id] ?? EMPTY;
      for (const chip of chips) {
        chip.setAttribute("aria-pressed", String(a.tags.includes(chip.dataset.tag ?? "")));
      }
      // les étiquettes d'une version plus récente restent affichées telles quelles
      if (document.activeElement !== note && note.value !== a.note) note.value = a.note;
    },
  };
}

export function createAnnotationPanel(h: AnnotationHandlers): AnnotationPanel {
  const body = el("div", { class: "annotation-parts" });
  const root = el("section", { class: "annotations", attrs: { "aria-label": "Annotations du clip" } }, [
    el("h3", { class: "annotation-title", text: "Annotations" }),
    body,
  ]);
  root.hidden = true;

  let renderedKey = "";
  let views: PartView[] = [];
  let latest: Annotations = {};

  return {
    el: root,
    update(state) {
      latest = state.annotations;
      const clip = currentClip(state);
      const sessionId = selectedGame(state)?.sessionId;
      root.hidden = !clip || !sessionId;
      if (!clip || !sessionId) {
        renderedKey = "";
        views = [];
        body.replaceChildren();
        return;
      }
      const parts = clipParts(clip);
      const key = `${sessionId}|${clip.id}|${parts.map((p) => p.id).join(",")}`;
      if (key !== renderedKey) {
        renderedKey = key;
        views = parts.map((p) => buildPart(p, () => latest, h));
        body.replaceChildren(...views.map((v) => v.el));
      }
      for (const v of views) v.sync(latest);
    },
  };
}
