import { tagDef } from "./annotations";
import { THEME_NAMES } from "./clips";
import type { AnnotatedMoment, Theme } from "./types";

export type Outcome = "all" | "win" | "loss";

export interface MomentFilters {
  championId: number | null;
  year: number | null;
  outcome: Outcome;
  tag: string | null;
  theme: Theme | null;
  text: string;
}

export const NO_FILTERS: MomentFilters = {
  championId: null,
  year: null,
  outcome: "all",
  tag: null,
  theme: null,
  text: "",
};

export function hasFilters(f: MomentFilters): boolean {
  return (
    f.championId !== null ||
    f.year !== null ||
    f.outcome !== "all" ||
    f.tag !== null ||
    f.theme !== null ||
    f.text.trim() !== ""
  );
}

/** Identifiant d'un moment dans la liste : une partie, un moment. */
export function momentKey(m: Pick<AnnotatedMoment, "sessionId" | "partId">): string {
  return `${m.sessionId}|${m.partId}`;
}

/** Année (heure locale) de la partie ; null si la partie n'a jamais été listée. */
export function momentYear(m: AnnotatedMoment): number | null {
  return m.game ? new Date(m.game.createdMs).getFullYear() : null;
}

/** Minuscules sans accents, pour chercher « presence » dans « Présence ». */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

function matchesText(m: AnnotatedMoment, text: string): boolean {
  const needle = fold(text.trim());
  if (needle === "") return true;
  const haystack = [m.note, m.label, THEME_NAMES[m.theme], ...m.tags.map((t) => tagDef(t).label)]
    .map(fold)
    .join("\n");
  return haystack.includes(needle);
}

/**
 * Moments qui respectent tous les filtres. Une partie dont le résumé est inconnu n'a ni
 * champion, ni année, ni issue : elle est écartée dès qu'un de ces trois filtres est actif.
 */
export function filterMoments(moments: readonly AnnotatedMoment[], f: MomentFilters): AnnotatedMoment[] {
  return moments.filter((m) => {
    if (f.championId !== null && m.game?.championId !== f.championId) return false;
    if (f.year !== null && momentYear(m) !== f.year) return false;
    if (f.outcome !== "all" && (m.game === null || m.game.win !== (f.outcome === "win"))) return false;
    if (f.tag !== null && !m.tags.includes(f.tag)) return false;
    if (f.theme !== null && m.theme !== f.theme) return false;
    return matchesText(m, f.text);
  });
}

/** Nombre de moments par étiquette, les plus fréquentes d'abord (puis par libellé). */
export function tagCounts(moments: readonly AnnotatedMoment[]): Array<{ tag: string; count: number }> {
  const counts = new Map<string, number>();
  for (const m of moments) for (const tag of m.tags) counts.set(tag, (counts.get(tag) ?? 0) + 1);
  return [...counts]
    .map(([tag, count]) => ({ tag, count }))
    .sort((a, b) => b.count - a.count || tagDef(a.tag).label.localeCompare(tagDef(b.tag).label, "fr"));
}

/** Champions (identifiants) présents, sans doublon, dans l'ordre d'apparition. */
export function championIds(moments: readonly AnnotatedMoment[]): number[] {
  return [...new Set(moments.flatMap((m) => (m.game ? [m.game.championId] : [])))];
}

/** Années présentes, la plus récente d'abord. */
export function yearsOf(moments: readonly AnnotatedMoment[]): number[] {
  const years = new Set<number>();
  for (const m of moments) {
    const y = momentYear(m);
    if (y !== null) years.add(y);
  }
  return [...years].sort((a, b) => b - a);
}

/** Types de moments présents, dans l'ordre des onglets. */
export function themesOf(moments: readonly AnnotatedMoment[], order: readonly Theme[]): Theme[] {
  return order.filter((t) => moments.some((m) => m.theme === t));
}

/** Moments dont le clip existe encore : ceux qu'on peut revoir. */
export function playableMoments(moments: readonly AnnotatedMoment[]): AnnotatedMoment[] {
  return moments.filter((m) => m.file !== null);
}
