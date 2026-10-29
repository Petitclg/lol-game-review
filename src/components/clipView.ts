import type { Backend } from "../api";
import { countByTheme, isTheme } from "../clips";
import { el, replaceChildren } from "../dom";
import { formatDuration } from "../format";
import {
  currentClip,
  selectedGame,
  visibleClips,
  type State,
} from "../state/store";
import type { Clip } from "../types";
import { NO_VIDEO_TOOLTIP } from "./gameList";
import { renderTabs } from "./tabs";
import type { ThemeFilter } from "../clips";

export const THEME_NAMES: Record<Clip["theme"], string> = {
  kill: "Kill",
  death: "Mort",
  assist: "Assist",
  ultimate: "Ultimate",
  objective: "Objectif",
  turret: "Tour",
};

export const MEDIA_ERROR_REASONS: Record<number, string> = {
  1: "lecture interrompue",
  2: "erreur réseau ou fichier inaccessible",
  3: "erreur de décodage de la vidéo",
  4: "format non pris en charge ou fichier introuvable / accès refusé",
};

export interface ClipViewHandlers {
  onGenerate(): void;
  onDeleteRecording(): void;
  onReloadClips(): void;
  onTheme(theme: ThemeFilter): void;
  onSelectClip(id: string): void;
  onStep(delta: number): void;
}

export interface ClipView {
  el: HTMLElement;
  update(state: State): void;
}

export function generateDisabledReason(state: State): string | null {
  const game = selectedGame(state);
  if (!game) return "Aucune partie sélectionnée.";
  if (!game.sessionId) return NO_VIDEO_TOOLTIP;
  if (state.generating) return "Génération en cours…";
  if (state.status && !state.status.ffmpegAvailable) return "FFmpeg est introuvable.";
  if (state.deletedSessions.includes(game.sessionId)) return "La vidéo source a été supprimée.";
  return null;
}

export function createClipView(backend: Backend, h: ClipViewHandlers): ClipView {
  const generateBtn = el("button", {
    class: "btn btn-primary",
    attrs: { type: "button" },
    on: { click: () => h.onGenerate() },
  });
  const deleteBtn = el("button", {
    class: "btn btn-danger",
    text: "Supprimer la vidéo source",
    attrs: { type: "button" },
    on: { click: () => h.onDeleteRecording() },
  });
  const progress = el("progress", { class: "progress", attrs: { max: 1, value: 0, "aria-label": "Progression de la génération" } });
  const progressText = el("span", { class: "progress-text", attrs: { "aria-live": "polite" } });
  const progressBox = el("div", { class: "progress-box", attrs: { hidden: true } }, [progress, progressText]);
  const toolbar = el("div", { class: "toolbar" }, [generateBtn, deleteBtn, progressBox]);
  const messages = el("div", { class: "messages", attrs: { "aria-live": "polite" } });

  const tabsEl = el("div", { class: "tabs" });
  const listEl = el("ol", { class: "clip-list", attrs: { "aria-label": "Clips" } });

  const video = el("video", {
    class: "video",
    attrs: { controls: true, preload: "metadata", "aria-label": "Lecteur de clip" },
  });
  const videoMsg = el("p", { class: "video-msg muted" });
  const videoErrText = el("p", { class: "video-error-text" });
  const videoErrFile = el("p", { class: "video-error-file small muted" });
  const retryBtn = el("button", {
    class: "btn btn-small",
    text: "Réessayer",
    attrs: { type: "button" },
    on: { click: () => retryVideo() },
  });
  const videoErr = el(
    "div",
    { class: "alert alert-error video-error", attrs: { role: "alert", hidden: true } },
    [videoErrText, videoErrFile, retryBtn],
  );
  const prevBtn = el("button", {
    class: "btn",
    text: "← Précédent",
    attrs: { type: "button", "aria-keyshortcuts": "ArrowLeft" },
    on: { click: () => h.onStep(-1) },
  });
  const nextBtn = el("button", {
    class: "btn",
    text: "Suivant →",
    attrs: { type: "button", "aria-keyshortcuts": "ArrowRight" },
    on: { click: () => h.onStep(1) },
  });
  const clipTitle = el("h3", { class: "clip-title" });
  const nav = el("div", { class: "player-nav" }, [prevBtn, clipTitle, nextBtn]);
  const player = el("div", { class: "clip-player" }, [video, videoMsg, videoErr, nav]);
  const browser = el("div", { class: "clip-browser", attrs: { id: "clip-panel", role: "tabpanel" } }, [player, listEl]);

  const placeholder = el("p", { class: "placeholder muted" });
  const root = el("section", { class: "clips-panel", attrs: { "aria-label": "Clips" } }, [
    toolbar,
    messages,
    tabsEl,
    browser,
    placeholder,
  ]);

  let renderedClipId: string | null | undefined;
  let renderedClipsRef: unknown;
  let renderedTheme: ThemeFilter | undefined;
  let renderedSelected: string | null | undefined;
  let playingClip: Clip | null = null;
  let playingSrc: string | null = null;

  function hideVideoError(): void {
    videoErr.hidden = true;
    videoErrText.textContent = "";
    videoErrFile.textContent = "";
  }

  function retryVideo(): void {
    if (!playingSrc) return;
    hideVideoError();
    video.src = playingSrc;
    video.load();
  }

  video.addEventListener("error", () => {
    if (!video.getAttribute("src") || !playingClip) return;
    const err = video.error;
    const code = err?.code ?? 0;
    const reason = MEDIA_ERROR_REASONS[code] ?? "erreur de lecture inconnue";
    const detail = err?.message ? ` — ${err.message}` : "";
    console.error("Lecture du clip impossible", {
      clipId: playingClip.id,
      file: playingClip.file,
      src: playingSrc,
      code,
      message: err?.message,
    });
    videoErrText.textContent = `Impossible de lire ce clip : ${reason} (code ${code}${detail})`;
    videoErrFile.textContent = `Fichier : ${playingClip.file ?? "inconnu"}`;
    videoErr.hidden = false;
  });

  function stopVideo(): void {
    renderedClipId = undefined;
    playingClip = null;
    playingSrc = null;
    hideVideoError();
    if (!video.hasAttribute("src")) return;
    video.pause();
    video.removeAttribute("src");
    video.load();
  }

  function updatePlayer(clip: Clip | null): void {
    const id = clip?.id ?? null;
    if (id === renderedClipId) return;
    stopVideo();
    renderedClipId = id;
    if (!clip) return;
    playingClip = clip;
    const src = clip.file ? backend.videoSrc(clip.file) : null;
    if (src) {
      playingSrc = src;
      video.src = src;
      video.hidden = false;
      videoMsg.hidden = true;
    } else {
      video.hidden = true;
      videoMsg.hidden = false;
      videoMsg.textContent = clip.file
        ? "Aperçu vidéo indisponible en mode simulé (navigateur)."
        : "Clip non généré : le fichier vidéo n'existe pas encore.";
    }
  }

  return {
    el: root,
    update(state) {
      const game = selectedGame(state);
      const hasSession = !!game?.sessionId;
      const hasClips = state.clips.length > 0;

      toolbar.hidden = !game || !hasSession;
      browser.hidden = !hasClips;
      tabsEl.hidden = !hasClips;
      placeholder.hidden = hasClips;

      if (!game) {
        stopVideo();
        placeholder.textContent = "Sélectionnez une partie dans la liste pour voir ses clips.";
        replaceChildren(messages, []);
        return;
      }
      if (!game.sessionId) {
        stopVideo();
        placeholder.textContent = `Pas de vidéo pour cette partie. ${NO_VIDEO_TOOLTIP}`;
        replaceChildren(messages, []);
        return;
      }

      // Barre d'outils
      const reason = generateDisabledReason(state);
      generateBtn.textContent = state.generating
        ? "Génération en cours…"
        : game.clipsReady
          ? "Régénérer les clips"
          : "Générer les clips";
      generateBtn.disabled = reason !== null;
      if (reason) generateBtn.title = reason;
      else generateBtn.removeAttribute("title");
      deleteBtn.hidden = state.deletedSessions.includes(game.sessionId);
      deleteBtn.disabled = state.generating !== null;

      const g = state.generating;
      progressBox.hidden = !g;
      if (g) {
        progress.max = Math.max(1, g.total);
        progress.value = g.done;
        progressText.textContent =
          g.total > 0 ? `Génération des clips : ${g.done} / ${g.total}` : "Préparation…";
      }

      // Messages
      const msgs: HTMLElement[] = [];
      if (state.generateError) {
        msgs.push(el("div", { class: "alert alert-error", attrs: { role: "alert" } }, [`Échec de la génération : ${state.generateError}`]));
      }
      if (state.clipsError) {
        msgs.push(
          el("div", { class: "alert alert-error", attrs: { role: "alert" } }, [
            `Impossible de charger les clips : ${state.clipsError} `,
            el("button", { class: "btn btn-small", text: "Réessayer", attrs: { type: "button" }, on: { click: () => h.onReloadClips() } }),
          ]),
        );
      }
      if (state.notice) msgs.push(el("div", { class: "alert alert-info" }, [state.notice]));
      replaceChildren(messages, msgs);

      if (!hasClips) {
        placeholder.textContent = state.clipsLoading
          ? "Chargement des clips…"
          : state.generating
            ? "Les clips arrivent bientôt…"
            : game.clipsReady
              ? "Aucun clip pour cette partie."
              : "Aucun clip pour l'instant. Cliquez sur « Générer les clips ».";
        stopVideo();
        renderedClipsRef = undefined;
        return;
      }

      const visible = visibleClips(state);
      const current = currentClip(state);

      if (renderedClipsRef !== state.clips || renderedTheme !== state.theme) {
        renderTabs(tabsEl, countByTheme(state.clips), state.theme, h.onTheme);
      }
      if (
        renderedClipsRef !== state.clips ||
        renderedTheme !== state.theme ||
        renderedSelected !== current?.id
      ) {
        const hadFocus = listEl.contains(document.activeElement);
        replaceChildren(
          listEl,
          visible.map((clip) => {
            const selected = clip.id === current?.id;
            return el("li", {}, [
              el(
                "button",
                {
                  class: `clip-item${isTheme(clip.theme) ? ` theme-${clip.theme}` : ""}${selected ? " selected" : ""}`,
                  attrs: { type: "button", "aria-current": selected ? "true" : undefined },
                  on: { click: () => h.onSelectClip(clip.id) },
                },
                [
                  el("span", { class: "clip-time", text: formatDuration(clip.gameTimeSecs) }),
                  el("span", { class: "clip-label", text: clip.label }),
                  el("span", { class: "clip-theme", text: THEME_NAMES[clip.theme] ?? String(clip.theme) }),
                ],
              ),
            ]);
          }),
        );
        if (hadFocus) listEl.querySelector<HTMLElement>(".clip-item.selected")?.focus();
        listEl.querySelector(".clip-item.selected")?.scrollIntoView?.({ block: "nearest" });
      }
      renderedClipsRef = state.clips;
      renderedTheme = state.theme;
      renderedSelected = current?.id;

      const idx = visible.findIndex((c) => c.id === current?.id);
      prevBtn.disabled = idx <= 0;
      nextBtn.disabled = idx < 0 || idx >= visible.length - 1;
      clipTitle.textContent = current
        ? `${current.label} — ${formatDuration(current.gameTimeSecs)} (${idx + 1}/${visible.length})`
        : "Aucun clip dans ce thème";
      if (visible.length === 0) {
        stopVideo();
        video.hidden = true;
        videoMsg.hidden = false;
        videoMsg.textContent = "Aucun clip dans ce thème.";
      } else {
        updatePlayer(current);
      }
    },
  };
}
