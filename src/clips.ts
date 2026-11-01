import type { Clip, ClipPart, Theme } from "./types";

export type ThemeFilter = Theme | "all";

export const THEMES: readonly Theme[] = [
  "kill",
  "death",
  "assist",
  "ultimate",
  "flash",
  "key_moment",
  "objective",
  "turret",
];

export const THEME_TABS: ReadonlyArray<{ id: ThemeFilter; label: string }> = [
  { id: "all", label: "Tous" },
  { id: "kill", label: "Kills" },
  { id: "death", label: "Morts" },
  { id: "assist", label: "Assists" },
  { id: "ultimate", label: "Ultimates" },
  { id: "flash", label: "Flashs" },
  { id: "key_moment", label: "Moments clés" },
  { id: "objective", label: "Objectifs" },
  { id: "turret", label: "Tours" },
];

export const THEME_NAMES: Record<Theme, string> = {
  kill: "Kill",
  death: "Mort",
  assist: "Assist",
  ultimate: "Ultimate",
  flash: "Flash",
  key_moment: "Moment clé",
  objective: "Objectif",
  turret: "Tour",
};

/** Garde de type : un thème inconnu (backend plus récent) est ignoré plutôt que de planter. */
export function isTheme(value: unknown): value is Theme {
  return typeof value === "string" && (THEMES as readonly string[]).includes(value);
}

export function sortClips(clips: readonly Clip[]): Clip[] {
  return [...clips].sort((a, b) => a.gameTimeSecs - b.gameTimeSecs || a.id.localeCompare(b.id));
}

/** Thèmes connus d'un clip : ceux d'un clip fusionné, sinon son thème seul. */
export function clipThemes(clip: Clip): Theme[] {
  const themes = (clip.themes ?? []).filter(isTheme);
  if (themes.length > 0) return themes;
  return isTheme(clip.theme) ? [clip.theme] : [];
}

/**
 * Moments d'un clip. Un clip sans `parts` (ancienne version, ou mock) est un moment unique ;
 * son identifiant est reconstruit comme le fait le backend (`theme-temps` à une décimale).
 */
export function clipParts(clip: Clip): ClipPart[] {
  if (clip.parts && clip.parts.length > 0) return clip.parts;
  return [
    {
      id: `${clip.theme}-${clip.gameTimeSecs.toFixed(1)}`,
      theme: clip.theme,
      label: clip.label,
      gameTimeSecs: clip.gameTimeSecs,
    },
  ];
}

export function filterClips(clips: readonly Clip[], theme: ThemeFilter): Clip[] {
  const sorted = sortClips(clips);
  return theme === "all" ? sorted : sorted.filter((c) => clipThemes(c).includes(theme));
}

export function countByTheme(clips: readonly Clip[]): Record<ThemeFilter, number> {
  const counts: Record<ThemeFilter, number> = {
    all: clips.length,
    kill: 0,
    death: 0,
    assist: 0,
    ultimate: 0,
    flash: 0,
    key_moment: 0,
    objective: 0,
    turret: 0,
  };
  for (const clip of clips) for (const theme of clipThemes(clip)) counts[theme] += 1;
  return counts;
}

export function groupByTheme(clips: readonly Clip[]): Record<Theme, Clip[]> {
  const groups: Record<Theme, Clip[]> = {
    kill: [],
    death: [],
    assist: [],
    ultimate: [],
    flash: [],
    key_moment: [],
    objective: [],
    turret: [],
  };
  for (const clip of sortClips(clips)) for (const theme of clipThemes(clip)) groups[theme].push(clip);
  return groups;
}
