# Contrat IPC (backend Rust ↔ interface TypeScript)

Toutes les structures sont sérialisées en JSON, champs en `camelCase`. Les commandes sont appelées avec `invoke` de `@tauri-apps/api/core`. Les erreurs sont des chaînes lisibles.

## Types

```ts
type Theme = "kill" | "death" | "assist" | "objective" | "turret" | "ultimate";

interface Account { riotId: string; level: number }

interface GameEntry {
  gameId: number;
  createdMs: number;        // epoch ms
  durationSecs: number;
  queueId: number;
  championId: number;
  kills: number; deaths: number; assists: number;
  win: boolean;
  sessionId: string | null; // non nul si la partie a été enregistrée par l'application
  clipsReady: boolean;      // les clips ont déjà été générés
}

interface ItemInfo { id: number; name: string }

interface PlayerSnapshot {
  name: string;             // Riot ID affiché
  champion: string;         // nom du champion (ex. "Ahri")
  team: "ORDER" | "CHAOS";
  isMe: boolean;
  level: number;
  kills: number; deaths: number; assists: number;
  items: ItemInfo[];
  spells: string[];         // sorts d'invocateur (noms affichés, selon la langue du jeu)
  spellKeys: string[];      // identifiants stables, ex. "SummonerFlash" ; [] pour les anciennes parties
}

type Role = "killer" | "victim" | "assist" | "caster";   // caster : lanceur d'un ultimate (le joueur local)

/** Joueur impliqué dans le moment du clip. `name` est égal à `PlayerSnapshot.name`. */
interface Involved { name: string; roles: Role[] }

interface Clip {
  id: string;
  theme: Theme;
  label: string;            // "Kill", "Dragon", ...
  gameTimeSecs: number;     // moment de l'événement
  startSecs: number;        // position dans la vidéo source
  endSecs: number;
  file: string | null;      // chemin absolu du fichier clip, null si non généré
  players: PlayerSnapshot[]; // état de la partie au moment du clip
  involved: Involved[];      // joueurs impliqués (tueur, victime, assistants), [] si inconnu
  myGold: number | null;     // or non dépensé du joueur local, null si non enregistré
}
```

## Commandes

| Commande | Arguments | Retour |
|---|---|---|
| `get_account` | — | `Account \| null` (null : client lancé, aucun compte connecté ; erreur : client éteint) |
| `list_games` | `{ count: number }` | `GameEntry[]`, plus récentes d'abord |
| `generate_clips` | `{ sessionId: string }` | `Clip[]` (extrait les fichiers vidéo, silencieusement) |
| `get_clips` | `{ sessionId: string }` | `Clip[]` (vide si non générés) |
| `delete_recording` | `{ sessionId: string }` | `void` (supprime la vidéo source, garde les clips) |
| `get_status` | — | `Status` |

```ts
interface Status {
  state: "idle" | "inGame";
  recording: boolean;
  ffmpegAvailable: boolean;
}
```

## Évènements (backend → interface)

- `status-changed` : charge utile `Status`, émis à chaque changement.
- `clips-progress` : `{ sessionId: string; done: number; total: number }` pendant `generate_clips`.

## Lecture des fichiers

Les fichiers clip sont lus dans une balise `<video>` via `convertFileSrc(clip.file)` (protocole `asset`).

## Données externes (interface seulement)

Noms et icônes de champions, icônes d'items : Data Dragon (`https://ddragon.leagueoflegends.com`). La version courante vient de `/api/versions.json`.
