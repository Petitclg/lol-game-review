import type { Clip, Theme } from "./types";

export type ThemeFilter = Theme | "all";

export const THEME_TABS: ReadonlyArray<{ id: ThemeFilter; label: string }> = [
  { id: "all", label: "Tous" },
  { id: "kill", label: "Kills" },
  { id: "death", label: "Morts" },
  { id: "assist", label: "Assists" },
  { id: "objective", label: "Objectifs" },
  { id: "turret", label: "Tours" },
];

export function sortClips(clips: readonly Clip[]): Clip[] {
  return [...clips].sort((a, b) => a.gameTimeSecs - b.gameTimeSecs || a.id.localeCompare(b.id));
}

export function filterClips(clips: readonly Clip[], theme: ThemeFilter): Clip[] {
  const sorted = sortClips(clips);
  return theme === "all" ? sorted : sorted.filter((c) => c.theme === theme);
}

export function countByTheme(clips: readonly Clip[]): Record<ThemeFilter, number> {
  const counts: Record<ThemeFilter, number> = {
    all: clips.length,
    kill: 0,
    death: 0,
    assist: 0,
    objective: 0,
    turret: 0,
  };
  for (const clip of clips) counts[clip.theme] += 1;
  return counts;
}

export function groupByTheme(clips: readonly Clip[]): Record<Theme, Clip[]> {
  const groups: Record<Theme, Clip[]> = {
    kill: [],
    death: [],
    assist: [],
    objective: [],
    turret: [],
  };
  for (const clip of sortClips(clips)) groups[clip.theme].push(clip);
  return groups;
}
