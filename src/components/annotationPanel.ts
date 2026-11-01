import { CUSTOM_PREFIX, MAX_LABEL_CHARS, cleanLabel, customTagId, isCustomTag, tagsFor, toggleTag } from "../annotations";
import { THEME_NAMES, clipParts } from "../clips";
import { el, replaceChildren } from "../dom";
import { formatDuration } from "../format";
import { currentClip, selectedGame, type State } from "../state/store";
import type { Annotation, Annotations, ClipPart } from "../types";

export interface AnnotationHandlers {
  /** Enregistre les étiquettes d'un moment. */
  onChange(partId: string, tags: string[]): void;
  /** Crée (ou retrouve) un tag personnalisé et le pose sur le moment. */
  onAddCustomTag(partId: string, label: string): void;
  /** Cesse de proposer un tag personnalisé ; les moments qui le portent le gardent. */
  onRemoveCustomTag(label: string): void;
}

export interface AnnotationPanel {
  el: HTMLElement;
  update(state: State): void;
}

const EMPTY: Annotation = { tags: [] };

interface PartView {
  el: HTMLElement;
  /** Aligne l'affichage sur les annotations enregistrées et les tags proposés. */
  sync(annotations: Annotations, customTags: readonly string[]): void;
}

function buildPart(part: ClipPart, getLatest: () => Annotations, h: AnnotationHandlers): PartView {
  const current = (): Annotation => getLatest()[part.id] ?? EMPTY;

  const chips = tagsFor(part.theme).map((tag) =>
    el("button", {
      class: `tag-chip tone-${tag.tone}`,
      text: tag.label,
      attrs: { type: "button", "aria-pressed": "false", "data-tag": tag.id },
      on: { click: () => h.onChange(part.id, toggleTag(current().tags, tag.id)) },
    }),
  );

  // --- tags personnalisés
  const customBox = el("div", { class: "tag-chips custom-tags", attrs: { role: "group", "aria-label": "Mes tags" } });
  const input = el("input", {
    class: "custom-tag-input",
    attrs: {
      type: "text",
      maxlength: MAX_LABEL_CHARS,
      placeholder: "Nouveau tag personnalisé…",
      "aria-label": `Nouveau tag personnalisé pour ${part.label}`,
      autocomplete: "off",
    },
  });
  const message = el("span", { class: "small custom-tag-message", attrs: { "aria-live": "polite" } });
  const submit = (): void => {
    const label = cleanLabel(input.value);
    if (label === null) {
      message.textContent = `Un tag fait de 1 à ${MAX_LABEL_CHARS} caractères.`;
      return;
    }
    message.textContent = "";
    input.value = "";
    h.onAddCustomTag(part.id, label);
  };
  const form = el(
    "form",
    {
      class: "custom-tag-form",
      attrs: { novalidate: true },
      on: {
        submit: (e) => {
          e.preventDefault();
          submit();
        },
      },
    },
    [input, el("button", { class: "btn btn-small", text: "Ajouter", attrs: { type: "submit" } }), message],
  );

  let customKey = "";
  const renderCustom = (tags: readonly string[], proposed: readonly string[]): void => {
    const carried = tags.filter(isCustomTag).map((t) => t.slice(CUSTOM_PREFIX.length));
    const known = new Set(proposed.map((l) => l.toLowerCase()));
    // un tag retiré de la liste reste affiché sur les moments qui le portent, pour pouvoir le décocher
    const labels = [...proposed, ...carried.filter((l) => !known.has(l.toLowerCase()))];
    const key = JSON.stringify([labels, proposed, tags.filter(isCustomTag)]);
    if (key === customKey) return;
    customKey = key;
    replaceChildren(
      customBox,
      labels.map((label) => {
        const id = customTagId(label);
        const isProposed = proposed.includes(label);
        return el("span", { class: "custom-chip" }, [
          el("button", {
            class: "tag-chip tone-neutral",
            text: label,
            attrs: { type: "button", "aria-pressed": String(tags.includes(id)), "data-tag": id },
            on: { click: () => h.onChange(part.id, toggleTag(current().tags, id)) },
          }),
          isProposed &&
            el("button", {
              class: "tag-remove",
              text: "✕",
              attrs: {
                type: "button",
                title: "Ne plus proposer ce tag (les moments qui le portent le gardent)",
                "aria-label": `Ne plus proposer le tag ${label}`,
              },
              on: { click: () => h.onRemoveCustomTag(label) },
            }),
        ]);
      }),
    );
  };

  const heading = el("h4", { class: "annotation-part-title" }, [
    el("span", { class: `annotation-theme theme-${part.theme}`, text: THEME_NAMES[part.theme] ?? part.theme }),
    ` ${part.label} — ${formatDuration(part.gameTimeSecs)}`,
  ]);
  const node = el("div", { class: "annotation-part", attrs: { "data-part": part.id } }, [
    heading,
    el("div", { class: "tag-chips", attrs: { role: "group", "aria-label": `Étiquettes : ${part.label}` } }, chips),
    customBox,
    form,
  ]);

  return {
    el: node,
    sync(annotations, customTags) {
      const tags = (annotations[part.id] ?? EMPTY).tags;
      for (const chip of chips) {
        chip.setAttribute("aria-pressed", String(tags.includes(chip.dataset.tag ?? "")));
      }
      renderCustom(tags, customTags);
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
      for (const v of views) v.sync(latest, state.customTags);
    },
  };
}
