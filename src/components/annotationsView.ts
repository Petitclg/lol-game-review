import { errorMessage, type Backend } from "../api";
import { tagDef } from "../annotations";
import { THEMES, THEME_NAMES } from "../clips";
import { findChampionByKey, type Ddragon } from "../ddragon";
import { el, replaceChildren } from "../dom";
import { formatDuration, formatKda, queueLabel } from "../format";
import {
  NO_FILTERS,
  championIds,
  filterMoments,
  hasFilters,
  momentKey,
  playableMoments,
  tagCounts,
  themesOf,
  yearsOf,
  type MomentFilters,
  type Outcome,
} from "../moments";
import type { AnnotatedMoment, GameSummary } from "../types";

export interface AnnotationsView {
  el: HTMLElement;
  /** Affiche la vue et (re)charge les moments annotés. */
  open(): Promise<void>;
  /** Masque la vue et arrête la lecture. */
  close(): void;
  /** Noms et icônes des champions (Data Dragon), quand ils sont disponibles. */
  setDdragon(dd: Ddragon | null): void;
}

export interface AnnotationsViewHandlers {
  /** L'utilisateur veut revenir à la liste des parties. */
  onClose(): void;
}

const DATE = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", year: "numeric" });

function option(value: string, text: string, selected: boolean): HTMLOptionElement {
  return el("option", { text, attrs: { value, selected } });
}

export function createAnnotationsView(backend: Backend, h: AnnotationsViewHandlers): AnnotationsView {
  let moments: AnnotatedMoment[] = [];
  let filters: MomentFilters = { ...NO_FILTERS };
  let selectedKey: string | null = null;
  let autoplay = true;
  let loading = false;
  let error: string | null = null;
  let dd: Ddragon | null = null;
  let playingKey: string | null | undefined;
  /** La lecture ne démarre seule qu'après un choix de l'utilisateur, pas à l'ouverture. */
  let userPicked = false;

  const championName = (id: number) => findChampionByKey(dd?.champions ?? null, id)?.name ?? `Champion ${id}`;

  // --- lecteur
  const video = el("video", {
    class: "video",
    attrs: { controls: true, preload: "metadata", "aria-label": "Clip du moment annoté" },
  });
  const videoMsg = el("p", { class: "video-msg muted", attrs: { hidden: true } });
  const prevBtn = el("button", {
    class: "btn btn-small",
    text: "◀ Précédent",
    attrs: { type: "button" },
    on: { click: () => step(-1) },
  });
  const nextBtn = el("button", {
    class: "btn btn-small",
    text: "Suivant ▶",
    attrs: { type: "button" },
    on: { click: () => step(1) },
  });
  const autoplayBox = el("input", {
    attrs: { type: "checkbox", id: "am-autoplay", checked: true },
    on: {
      change: () => {
        autoplay = autoplayBox.checked;
      },
    },
  });
  const nowPlaying = el("div", { class: "am-now" });
  const nav = el("div", { class: "player-nav" }, [
    prevBtn,
    el("label", { class: "check", attrs: { for: "am-autoplay" } }, [autoplayBox, "Lecture continue"]),
    nextBtn,
  ]);
  const player = el("div", { class: "clip-player am-player" }, [video, videoMsg, nav, nowPlaying]);

  video.addEventListener("ended", () => {
    if (autoplay) step(1);
  });

  // --- filtres
  const championSel = el("select", { attrs: { id: "am-champion", "aria-label": "Champion" } });
  const yearSel = el("select", { attrs: { id: "am-year", "aria-label": "Année" } });
  const outcomeSel = el("select", { attrs: { id: "am-outcome", "aria-label": "Issue" } });
  const themeSel = el("select", { attrs: { id: "am-theme", "aria-label": "Type de moment" } });
  const tagSel = el("select", { attrs: { id: "am-tag", "aria-label": "Étiquette" } });
  const textInput = el("input", {
    attrs: { type: "search", id: "am-text", placeholder: "Chercher un tag…", "aria-label": "Texte à chercher dans les tags" },
  });
  const resetBtn = el("button", {
    class: "btn btn-small",
    text: "Réinitialiser",
    attrs: { type: "button" },
    on: {
      click: () => {
        filters = { ...NO_FILTERS };
        render();
      },
    },
  });
  const field = (label: string, control: HTMLElement) =>
    el("label", { class: "am-field" }, [el("span", { class: "muted small", text: label }), control]);
  const filtersBar = el("div", { class: "am-filters" }, [
    field("Champion", championSel),
    field("Année", yearSel),
    field("Issue", outcomeSel),
    field("Type", themeSel),
    field("Étiquette", tagSel),
    field("Texte", textInput),
    resetBtn,
  ]);

  const onFilter = () => {
    filters = {
      championId: championSel.value === "" ? null : Number(championSel.value),
      year: yearSel.value === "" ? null : Number(yearSel.value),
      outcome: outcomeSel.value as Outcome,
      theme: themeSel.value === "" ? null : (themeSel.value as MomentFilters["theme"]),
      tag: tagSel.value === "" ? null : tagSel.value,
      text: textInput.value,
    };
    render();
  };
  for (const sel of [championSel, yearSel, outcomeSel, themeSel, tagSel]) sel.addEventListener("change", onFilter);
  textInput.addEventListener("input", onFilter);

  const summaryEl = el("div", { class: "am-summary", attrs: { "aria-label": "Décompte des étiquettes" } });
  const countEl = el("p", { class: "muted small am-count", attrs: { "aria-live": "polite" } });
  const listEl = el("ol", { class: "am-list", attrs: { "aria-label": "Moments annotés" } });
  const messageEl = el("p", { class: "placeholder muted", attrs: { "aria-live": "polite" } });

  const closeBtn = el("button", {
    class: "btn btn-small",
    text: "← Parties",
    attrs: { type: "button" },
    on: { click: () => h.onClose() },
  });
  const root = el("section", { class: "annotations-view", attrs: { "aria-label": "Annotations de toutes les parties" } }, [
    el("div", { class: "am-head" }, [closeBtn, el("h2", { text: "Mes annotations" })]),
    filtersBar,
    summaryEl,
    countEl,
    messageEl,
    el("div", { class: "am-body" }, [player, listEl]),
  ]);
  root.hidden = true;
  const body = root.querySelector<HTMLElement>(".am-body") as HTMLElement;

  // --- logique
  function visible(): AnnotatedMoment[] {
    return filterMoments(moments, filters);
  }

  function current(list: readonly AnnotatedMoment[]): AnnotatedMoment | null {
    return list.find((m) => momentKey(m) === selectedKey) ?? list[0] ?? null;
  }

  function step(delta: number): void {
    const playable = playableMoments(visible());
    if (playable.length === 0) return;
    const idx = playable.findIndex((m) => momentKey(m) === selectedKey);
    const next = playable[Math.min(playable.length - 1, Math.max(0, (idx < 0 ? 0 : idx) + delta))];
    if (next) {
      userPicked = true;
      selectedKey = momentKey(next);
      render();
    }
  }

  function gameLine(game: GameSummary | null): string {
    if (!game) return "Partie inconnue";
    return `${championName(game.championId)} · ${game.win ? "Victoire" : "Défaite"} · ${DATE.format(game.createdMs)} · ${formatKda(game.kills, game.deaths, game.assists)} · ${queueLabel(game.queueId)}`;
  }

  function momentTitle(m: AnnotatedMoment): string {
    return `${m.label || THEME_NAMES[m.theme]} — ${formatDuration(m.gameTimeSecs)}`;
  }

  function tagChips(tags: readonly string[]): HTMLElement {
    return el(
      "span",
      { class: "am-tags" },
      tags.map((t) => el("span", { class: `tag-chip static tone-${tagDef(t).tone}`, text: tagDef(t).label })),
    );
  }

  function renderFilters(): void {
    const sorted = [...championIds(moments)].sort((a, b) => championName(a).localeCompare(championName(b), "fr"));
    replaceChildren(championSel, [
      option("", "Tous", filters.championId === null),
      ...sorted.map((id) => option(String(id), championName(id), filters.championId === id)),
    ]);
    replaceChildren(yearSel, [
      option("", "Toutes", filters.year === null),
      ...yearsOf(moments).map((y) => option(String(y), String(y), filters.year === y)),
    ]);
    replaceChildren(outcomeSel, [
      option("all", "Toutes", filters.outcome === "all"),
      option("win", "Victoires", filters.outcome === "win"),
      option("loss", "Défaites", filters.outcome === "loss"),
    ]);
    replaceChildren(themeSel, [
      option("", "Tous", filters.theme === null),
      ...themesOf(moments, THEMES).map((t) => option(t, THEME_NAMES[t], filters.theme === t)),
    ]);
    const tags = tagCounts(moments).map((c) => c.tag);
    replaceChildren(tagSel, [
      option("", "Toutes", filters.tag === null),
      ...tags.map((t) => option(t, tagDef(t).label, filters.tag === t)),
    ]);
    if (document.activeElement !== textInput) textInput.value = filters.text;
    resetBtn.disabled = !hasFilters(filters);
  }

  function renderSummary(list: readonly AnnotatedMoment[]): void {
    // le décompte porte sur les moments filtrés, sauf par étiquette : on garde de quoi en changer
    const counts = tagCounts(filterMoments(moments, { ...filters, tag: null }));
    replaceChildren(
      summaryEl,
      counts.map(({ tag, count }) => {
        const def = tagDef(tag);
        return el("button", {
          class: `tag-chip tone-${def.tone}`,
          text: `${def.label} ${count}`,
          attrs: { type: "button", "aria-pressed": String(filters.tag === tag), "data-tag": tag },
          on: {
            click: () => {
              filters = { ...filters, tag: filters.tag === tag ? null : tag };
              render();
            },
          },
        });
      }),
    );
    const playable = playableMoments(list).length;
    countEl.textContent =
      list.length === 0
        ? ""
        : `${list.length} moment${list.length > 1 ? "s" : ""} annoté${list.length > 1 ? "s" : ""}, ${playable} avec clip à revoir`;
  }

  function renderList(list: readonly AnnotatedMoment[], sel: AnnotatedMoment | null): void {
    replaceChildren(
      listEl,
      list.map((m) => {
        const selected = sel !== null && momentKey(m) === momentKey(sel);
        return el("li", {}, [
          el(
            "button",
            {
              class: `am-item${selected ? " selected" : ""}${m.file ? "" : " unavailable"}`,
              attrs: { type: "button", "aria-current": selected ? "true" : undefined, "data-key": momentKey(m) },
              on: {
                click: () => {
                  userPicked = true;
                  selectedKey = momentKey(m);
                  render();
                },
              },
            },
            [
              el("span", { class: "am-item-game", text: gameLine(m.game) }),
              el("span", { class: "am-item-moment" }, [
                el("span", { class: `annotation-theme theme-${m.theme}`, text: THEME_NAMES[m.theme] }),
                ` ${momentTitle(m)}`,
                !m.file && el("span", { class: "muted", text: " · clip indisponible" }),
              ]),
              tagChips(m.tags),
            ],
          ),
        ]);
      }),
    );
  }

  function renderPlayer(list: readonly AnnotatedMoment[], sel: AnnotatedMoment | null): void {
    const playable = playableMoments(list);
    const idx = playable.findIndex((m) => sel && momentKey(m) === momentKey(sel));
    prevBtn.disabled = idx <= 0;
    nextBtn.disabled = idx < 0 || idx >= playable.length - 1;
    player.hidden = list.length === 0;
    if (!sel) {
      stop();
      return;
    }
    replaceChildren(nowPlaying, [
      el("h3", { class: "clip-title", text: gameLine(sel.game) }),
      el("p", { class: "small" }, [`${momentTitle(sel)} `, tagChips(sel.tags)]),
    ]);
    const key = momentKey(sel);
    if (key === playingKey) return;
    playingKey = key;
    const src = sel.file ? backend.videoSrc(sel.file) : null;
    if (src) {
      video.src = src;
      video.hidden = false;
      videoMsg.hidden = true;
      if (autoplay && userPicked) void video.play()?.catch?.(() => undefined);
    } else {
      stop();
      playingKey = key;
      video.hidden = true;
      videoMsg.hidden = false;
      videoMsg.textContent = sel.file
        ? "Aperçu vidéo indisponible en mode simulé (navigateur)."
        : "Le clip de ce moment n'existe plus (supprimé pour libérer de la place).";
    }
  }

  function stop(): void {
    playingKey = undefined;
    if (!video.hasAttribute("src")) return;
    video.pause();
    video.removeAttribute("src");
    video.load();
  }

  function render(): void {
    messageEl.hidden = !loading && error === null && moments.length > 0;
    if (loading) messageEl.textContent = "Chargement des annotations…";
    else if (error) messageEl.textContent = `Impossible de charger les annotations : ${error}`;
    else if (moments.length === 0) {
      messageEl.textContent =
        "Aucune annotation pour l'instant. Annotez un moment depuis le lecteur de clips (étiquettes et tags personnalisés).";
    }
    body.hidden = moments.length === 0;
    filtersBar.hidden = moments.length === 0;
    summaryEl.hidden = moments.length === 0;
    countEl.hidden = moments.length === 0;
    if (moments.length === 0) {
      stop();
      return;
    }
    const list = visible();
    const sel = current(list);
    selectedKey = sel ? momentKey(sel) : null;
    renderFilters();
    renderSummary(list);
    if (list.length === 0) {
      messageEl.hidden = false;
      messageEl.textContent = "Aucun moment ne correspond à ces filtres.";
    }
    renderList(list, sel);
    renderPlayer(list, sel);
    listEl.querySelector(".am-item.selected")?.scrollIntoView?.({ block: "nearest" });
  }

  return {
    el: root,
    async open() {
      root.hidden = false;
      loading = true;
      error = null;
      render();
      try {
        moments = await backend.listAnnotatedMoments();
      } catch (err) {
        error = errorMessage(err);
        moments = [];
      }
      loading = false;
      // un filtre sur un champion ou une étiquette qui n'existe plus n'aurait plus de sens
      if (filters.championId !== null && !championIds(moments).includes(filters.championId)) filters = { ...filters, championId: null };
      if (filters.tag !== null && !moments.some((m) => m.tags.includes(filters.tag as string))) filters = { ...filters, tag: null };
      playingKey = undefined;
      render();
    },
    setDdragon(next) {
      dd = next;
      if (!root.hidden && moments.length > 0) render();
    },
    close() {
      root.hidden = true;
      stop();
    },
  };
}
