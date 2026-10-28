/** Cooldown effectif avec ability haste : base * 100 / (100 + haste). */
export function effectiveCooldown(base: number, abilityHaste: number): number {
  return (base * 100) / (100 + Math.max(0, abilityHaste));
}

/** Cooldown de base pour un rang (1-indexé) dans un tableau DDragon ; rang 0 → null. */
export function baseCooldownForRank(
  cooldowns: readonly number[] | undefined,
  rank: number,
): number | null {
  if (!cooldowns || cooldowns.length === 0) return null;
  if (!Number.isInteger(rank) || rank <= 0) return null;
  const value = cooldowns[Math.min(rank, cooldowns.length) - 1];
  return value === undefined ? null : value;
}

export function formatCooldown(secs: number): string {
  const rounded = Math.round(secs * 10) / 10;
  return `${Number.isInteger(rounded) ? rounded : rounded.toFixed(1)} s`;
}
