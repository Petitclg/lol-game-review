import type { GameEntry } from "./types";

/** Clé des parties dont l'enregistrement ne dit pas quel compte les a jouées. */
export const UNKNOWN_ACCOUNT_KEY = "";

export const UNKNOWN_ACCOUNT_LABEL = "Compte inconnu";

export interface AccountOption {
  key: string;
  label: string;
  games: number;
}

/** Les Riot ID ne tiennent pas compte de la casse : « Nom#EUW » et « nom#euw » sont le même compte. */
export function accountKey(account: string | null | undefined): string {
  return account?.trim().toLowerCase() ?? UNKNOWN_ACCOUNT_KEY;
}

export function gameAccountKey(game: GameEntry): string {
  return accountKey(game.account);
}

/**
 * Les comptes qui ont des parties, le plus récemment joué d'abord, puis le compte connecté
 * s'il n'en a aucune, puis les parties de compte inconnu.
 */
export function accountOptions(games: readonly GameEntry[], connected: string | null): AccountOption[] {
  const found = new Map<string, AccountOption & { latest: number }>();
  for (const game of games) {
    const key = gameAccountKey(game);
    const known = found.get(key);
    if (known) {
      known.games += 1;
      if (game.createdMs > known.latest) {
        known.latest = game.createdMs;
        known.label = game.account?.trim() || known.label;
      }
    } else {
      found.set(key, {
        key,
        label: game.account?.trim() || UNKNOWN_ACCOUNT_LABEL,
        games: 1,
        latest: game.createdMs,
      });
    }
  }
  const connectedKey = accountKey(connected);
  if (connected && !found.has(connectedKey)) {
    found.set(connectedKey, { key: connectedKey, label: connected, games: 0, latest: -1 });
  }
  const rank = (o: { key: string; latest: number }) => (o.key === UNKNOWN_ACCOUNT_KEY ? -2 : o.latest);
  return [...found.values()]
    .sort((a, b) => rank(b) - rank(a))
    .map(({ key, label, games: count }) => ({ key, label, games: count }));
}

/** Le compte connecté s'il est proposé, sinon celui de la partie la plus récente. */
export function defaultAccountKey(options: readonly AccountOption[], connected: string | null): string | null {
  const connectedKey = accountKey(connected);
  if (connected && options.some((o) => o.key === connectedKey)) return connectedKey;
  return options[0]?.key ?? null;
}
