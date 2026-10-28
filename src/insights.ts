import type { ItemPrices } from "./ddragon";
import type { Clip, ItemInfo, PlayerSnapshot, Role } from "./types";

export type Team = PlayerSnapshot["team"];
export type Tone = "good" | "bad" | "neutral";

export interface InvolvedPlayer {
  player: PlayerSnapshot;
  roles: Role[];
  isMe: boolean;
  /** Dans l'équipe du joueur local (ORDER par défaut si le joueur local est inconnu). */
  isAlly: boolean;
}

const ROLE_ORDER: readonly Role[] = ["killer", "victim", "assist"];

export const ROLE_LABELS: Record<Role, string> = {
  killer: "Tueur",
  victim: "Victime",
  assist: "Assist",
};

export const FLASH_NOTE =
  "Flash équipé, pas forcément disponible : Riot n'expose pas les cooldowns.";
export const GOLD_NOTE =
  "L'or total des joueurs n'est pas fourni par Riot : la valeur des items est affichée à la place.";
export const UNAVAILABLE_NOTE =
  "Analyse indisponible pour ce clip (partie enregistrée avant cette fonctionnalité)";

const FLASH_KEY = "SummonerFlash";
const FLASH_NAMES = new Set(["flash", "saut éclair", "blitz", "destello", "salto rápido"]);

export function myTeam(clip: Pick<Clip, "players">): Team | null {
  return clip.players.find((p) => p.isMe)?.team ?? null;
}

function isRole(value: unknown): value is Role {
  return value === "killer" || value === "victim" || value === "assist";
}

/**
 * Joueurs impliqués résolus dans `clip.players` : noms inconnus ignorés, rôles dédoublonnés
 * (un joueur peut en cumuler plusieurs), alliés d'abord puis ennemis.
 */
export function involvedPlayers(clip: Pick<Clip, "players" | "involved">): InvolvedPlayer[] {
  const mine = myTeam(clip);
  const byName = new Map<string, InvolvedPlayer>();
  for (const entry of clip.involved ?? []) {
    const player = clip.players.find((p) => p.name === entry.name);
    if (!player) continue;
    const existing = byName.get(player.name);
    const roles = new Set<Role>(existing?.roles);
    for (const role of entry.roles ?? []) if (isRole(role)) roles.add(role);
    byName.set(player.name, {
      player,
      roles: ROLE_ORDER.filter((r) => roles.has(r)),
      isMe: player.isMe,
      isAlly: player.team === (mine ?? "ORDER"),
    });
  }
  const all = [...byName.values()];
  return [...all.filter((p) => p.isAlly), ...all.filter((p) => !p.isAlly)];
}

/** Valeur (or) des items ; null si les prix sont indisponibles. Consommables et babioles sont absents de `prices`. */
export function itemValue(
  items: readonly ItemInfo[] | undefined,
  prices: ItemPrices | null | undefined,
): number | null {
  if (!prices) return null;
  let total = 0;
  for (const item of items ?? []) total += prices.get(item.id) ?? 0;
  return total;
}

export function isFlashEquipped(player: Pick<PlayerSnapshot, "spells" | "spellKeys">): boolean {
  if ((player.spellKeys ?? []).includes(FLASH_KEY)) return true;
  return (player.spells ?? []).some((s) => FLASH_NAMES.has(s.trim().toLowerCase()));
}

export type FlashVerdict = "advantage" | "disadvantage" | "both" | "none" | "partial";

export interface Summary {
  /** Moyenne des niveaux (alliés − ennemis), arrondie à 0,1 ; null si un camp est vide. */
  levelAdvantage: number | null;
  /** Moyenne de la valeur des items (alliés − ennemis), arrondie à l'unité ; null si prix ou camp manquant. */
  itemAdvantage: number | null;
  itemTotals: { ally: number; enemy: number } | null;
  /** null si un camp est vide (comparaison impossible). */
  flash: FlashVerdict | null;
}

const average = (values: number[]): number =>
  values.reduce((sum, v) => sum + v, 0) / values.length;

const round1 = (v: number): number => Math.round(v * 10) / 10 + 0;

export function summarize(
  involved: readonly InvolvedPlayer[],
  prices: ItemPrices | null | undefined,
): Summary | null {
  if (involved.length === 0) return null;
  const allies = involved.filter((p) => p.isAlly);
  const enemies = involved.filter((p) => !p.isAlly);
  const bothSides = allies.length > 0 && enemies.length > 0;

  const levelAdvantage = bothSides
    ? round1(
        average(allies.map((p) => p.player.level)) - average(enemies.map((p) => p.player.level)),
      )
    : null;

  let itemAdvantage: number | null = null;
  let itemTotals: Summary["itemTotals"] = null;
  if (bothSides && prices) {
    const allyValues = allies.map((p) => itemValue(p.player.items, prices) ?? 0);
    const enemyValues = enemies.map((p) => itemValue(p.player.items, prices) ?? 0);
    itemAdvantage = Math.round(average(allyValues) - average(enemyValues)) + 0;
    itemTotals = {
      ally: allyValues.reduce((s, v) => s + v, 0),
      enemy: enemyValues.reduce((s, v) => s + v, 0),
    };
  }

  let flash: FlashVerdict | null = null;
  if (bothSides) {
    const allyFlash = allies.map((p) => isFlashEquipped(p.player));
    const enemyFlash = enemies.map((p) => isFlashEquipped(p.player));
    const allAlly = allyFlash.every(Boolean);
    const allEnemy = enemyFlash.every(Boolean);
    const someEnemyLacks = !allEnemy;
    const someAllyLacks = !allAlly;
    if (allAlly && someEnemyLacks) flash = "advantage";
    else if (allEnemy && someAllyLacks) flash = "disadvantage";
    else if (allAlly && allEnemy) flash = "both";
    else if (![...allyFlash, ...enemyFlash].some(Boolean)) flash = "none";
    else flash = "partial";
  }

  return { levelAdvantage, itemAdvantage, itemTotals, flash };
}

export function toneOf(value: number | null): Tone {
  if (value === null || value === 0) return "neutral";
  return value > 0 ? "good" : "bad";
}

export function flashTone(verdict: FlashVerdict | null): Tone {
  if (verdict === "advantage") return "good";
  if (verdict === "disadvantage") return "bad";
  return "neutral";
}

export const FLASH_LABELS: Record<FlashVerdict, string> = {
  advantage: "Avantage Flash",
  disadvantage: "Désavantage Flash",
  both: "Flash équipé des deux côtés",
  none: "Aucun Flash équipé",
  partial: "Flash équipé de façon inégale",
};

const MINUS = "\u2212";
const NARROW_NBSP = "\u202f";

/** 3250 → "3 250" (séparateur fin insécable). */
export function groupThousands(n: number): string {
  const sign = n < 0 ? MINUS : "";
  return sign + String(Math.abs(Math.round(n))).replace(/\B(?=(\d{3})+(?!\d))/g, NARROW_NBSP);
}

export function formatGold(n: number): string {
  return `${groupThousands(n)} or`;
}

/** "+850 or d'items" / "−1 200 or d'items" / "0 or d'items". */
export function formatItemAdvantage(n: number): string {
  const sign = n > 0 ? "+" : "";
  return `${sign}${groupThousands(n)} or d'items`;
}

/** "+2 niveaux", "−1,5 niveau", "0 niveau". */
export function formatLevelAdvantage(n: number): string {
  const abs = Math.abs(n);
  const text = String(abs).replace(".", ",");
  const sign = n > 0 ? "+" : n < 0 ? MINUS : "";
  return `${sign}${text} ${abs >= 2 ? "niveaux" : "niveau"}`;
}
