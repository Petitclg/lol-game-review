import type { Annotation, Annotations, Theme } from "./types";

/** `bad` : une erreur ; `good` : un bon réflexe ; `neutral` : un constat. */
export type TagTone = "bad" | "good" | "neutral";

export interface TagDef {
  /** Identifiant stocké sur le disque : ne jamais le changer, seul le libellé peut évoluer. */
  id: string;
  label: string;
  tone: TagTone;
}

/** Étiquettes d'une mort : les erreurs classiques, puis ce qui n'en est pas une. */
export const DEATH_TAGS: readonly TagDef[] = [
  { id: "face_check", label: "Face-check", tone: "bad" },
  { id: "mechanical_error", label: "Erreur mécanique", tone: "bad" },
  { id: "outnumbered", label: "Infériorité numérique", tone: "bad" },
  { id: "caught", label: "Catch", tone: "bad" },
  { id: "bad_jungle_tracking", label: "Mauvais jungle tracking", tone: "bad" },
  { id: "unexpected_enemy", label: "Présence d'un ennemi imprévue", tone: "bad" },
  { id: "no_vision", label: "Absence de vision", tone: "bad" },
  { id: "greed", label: "Greed", tone: "bad" },
  { id: "cooldowns_down", label: "Cooldowns clés indisponibles", tone: "bad" },
  { id: "lost_trade", label: "Trade perdant", tone: "bad" },
  { id: "solo_engage", label: "Engage sans équipe", tone: "bad" },
  { id: "objective_death", label: "Mort à l'objectif", tone: "bad" },
  { id: "ganked", label: "Gank subi", tone: "bad" },
  { id: "should_recall", label: "Aurait dû recall", tone: "bad" },
  { id: "unavoidable", label: "Mort inévitable", tone: "neutral" },
  { id: "good_death", label: "Bonne mort", tone: "good" },
];

/** Étiquettes des autres moments (kills, assists, objectifs, ultimes, Flashs…). */
export const PLAY_TAGS: readonly TagDef[] = [
  { id: "good_trade", label: "Bon trade", tone: "good" },
  { id: "good_outplay", label: "Bon outplay", tone: "good" },
  { id: "good_roam", label: "Bon roam", tone: "good" },
  { id: "easy_kill", label: "Kill facile / snowball", tone: "neutral" },
];

/** Étiquettes de tout moment : l'issue d'un combat d'équipe. */
export const TEAMFIGHT_TAGS: readonly TagDef[] = [
  { id: "good_teamfight", label: "Bon teamfight", tone: "good" },
  { id: "bad_teamfight", label: "Mauvais teamfight", tone: "bad" },
];

/** Étiquettes proposées pour un moment de ce thème. */
export function tagsFor(theme: Theme): TagDef[] {
  return [...(theme === "death" ? DEATH_TAGS : PLAY_TAGS), ...TEAMFIGHT_TAGS];
}

const ALL_TAGS: readonly TagDef[] = [...DEATH_TAGS, ...PLAY_TAGS, ...TEAMFIGHT_TAGS];

/** Préfixe de la forme enregistrée d'un tag personnalisé : `custom:` puis son texte. */
export const CUSTOM_PREFIX = "custom:";
/** Longueur maximale du texte d'un tag personnalisé (le backend refuse au-delà). */
export const MAX_LABEL_CHARS = 40;

export function isCustomTag(id: string): boolean {
  return id.startsWith(CUSTOM_PREFIX);
}

export function customTagId(label: string): string {
  return `${CUSTOM_PREFIX}${label}`;
}

/**
 * Texte d'un tag personnalisé saisi : blancs de bord retirés et blancs internes réduits à un
 * espace, comme le fait le backend. `null` si le texte est vide, trop long ou contient un
 * caractère de contrôle.
 */
export function cleanLabel(text: string): string | null {
  const label = text.split(/\s+/).filter(Boolean).join(" ");
  // eslint-disable-next-line no-control-regex
  const hasControl = /[\u0000-\u001f\u007f-\u009f]/.test(label);
  if (label === "" || hasControl || [...label].length > MAX_LABEL_CHARS) return null;
  return label;
}

/** Étiquette d'un identifiant ; un identifiant inconnu (version plus récente) reste lisible. */
export function tagDef(id: string): TagDef {
  if (isCustomTag(id)) return { id, label: id.slice(CUSTOM_PREFIX.length), tone: "neutral" };
  return ALL_TAGS.find((t) => t.id === id) ?? { id, label: id.replace(/_/g, " "), tone: "neutral" };
}

export function isEmptyAnnotation(a: Annotation | undefined): boolean {
  return !a || a.tags.length === 0;
}

/** Ajoute ou retire une étiquette sans changer l'ordre des autres. */
export function toggleTag(tags: readonly string[], id: string): string[] {
  return tags.includes(id) ? tags.filter((t) => t !== id) : [...tags, id];
}

/** Applique une annotation enregistrée : une annotation vide retire le moment du registre. */
export function withAnnotation(all: Annotations, partId: string, a: Annotation): Annotations {
  const next = { ...all };
  if (isEmptyAnnotation(a)) delete next[partId];
  else next[partId] = a;
  return next;
}
