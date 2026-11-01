# Contrat IPC (backend Rust ↔ interface TypeScript)

Toutes les structures sont sérialisées en JSON, champs en `camelCase`. Les commandes sont appelées avec `invoke` de `@tauri-apps/api/core`. Les erreurs sont des chaînes lisibles.

## Types

```ts
type Theme = "kill" | "death" | "assist" | "objective" | "turret" | "ultimate" | "flash" | "key_moment";
// ultimate, flash : sorts du joueur local, lus dans la vidéo (ADR 0003)
// key_moment : ace ou écart d'or estimé, quel que soit le joueur impliqué (ADR 0007)

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
  hasRecording: boolean;    // la vidéo source existe encore (sinon : plus de génération/régénération possible)
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

type Role = "killer" | "victim" | "assist" | "caster";   // caster : lanceur d'un ultimate ou d'un Flash (le joueur local)

/** Joueur impliqué dans le moment du clip. `name` est égal à `PlayerSnapshot.name`. */
interface Involved { name: string; roles: Role[] }

interface Clip {
  id: string;               // "kill-0", ou "seq-0" pour un clip fusionné
  theme: Theme;             // thème principal
  themes: Theme[];          // tous les thèmes du clip, `theme` en premier ; [] pour un clip d'une ancienne version
  label: string;            // "Kill", "Dragon", "Kill + Ultimate + Mort"...
  gameTimeSecs: number;     // moment du premier évènement
  startSecs: number;        // position dans la vidéo source
  endSecs: number;
  file: string | null;      // chemin absolu du fichier clip, null si non généré
  players: PlayerSnapshot[]; // état de la partie au moment du clip (premier moment)
  involved: Involved[];      // joueurs impliqués (tueur, victime, assistants), [] si inconnu
  myGold: number | null;     // or non dépensé du joueur local, null si non enregistré
  parts: ClipPart[];         // moments montrés par le clip, par heure ; [] pour un clip d'une ancienne version
}

/** Un moment montré par un clip. `id` = `thème-temps de jeu à une décimale` : stable, il sert de clé aux annotations. */
interface ClipPart { id: string; theme: Theme; label: string; gameTimeSecs: number }

/** Étiquettes (identifiants a-z, 0-9, _) et note libre d'un moment. */
interface Annotation { tags: string[]; note: string }
```

## Commandes

| Commande | Arguments | Retour |
|---|---|---|
| `get_account` | — | `Account \| null` (null : client lancé, aucun compte connecté ; erreur : client éteint) |
| `list_games` | `{ count: number }` | `GameEntry[]`, plus récentes d'abord |
| `generate_clips` | `{ sessionId: string }` | `Clip[]` (extrait les fichiers vidéo, silencieusement ; fusionne les clips qui se recoupent, voir [ADR 0008](../adr/0008-clip-merging-and-annotations.md)) |
| `get_clips` | `{ sessionId: string }` | `Clip[]` (vide si non générés) |
| `get_annotations` | `{ sessionId: string }` | `Record<string, Annotation>` par identifiant de moment (vide si aucune) |
| `set_annotation` | `{ sessionId: string, partId: string, tags: string[], note: string }` | `Annotation` telle que gardée (étiquettes dédoublonnées, note sans blancs de bord) ; une annotation vide supprime celle du moment ; erreur si un identifiant, une étiquette (24 au plus) ou la note (2 000 caractères au plus) est invalide |
| `delete_recording` | `{ sessionId: string }` | `void` (supprime la vidéo source, garde les clips) |
| `get_status` | — | `Status` |
| `get_settings` | — | `Settings` |
| `set_settings` | `{ settings: Settings }` | `Settings` (valeurs normalisées) |
| `get_storage_usage` | — | `StorageUsage` |
| `get_ultimate_status` | `{ sessionId: string }` | `UltimateStatus` |

La version de l'application n'est pas une commande : l'interface la lit avec `getVersion()` de `@tauri-apps/api/app` (permission `core:default`) et l'affiche à côté du titre (`vX.Y.Z`). Le backend simulé renvoie `0.0.0-dev`.

```ts
interface Settings {
  deleteSourceAfterClips: boolean;  // supprimer la vidéo source une fois les clips générés (défaut : true)
  compressClips: boolean;           // ré-encoder les clips pour qu'ils prennent moins de place (défaut : true)
  retentionGb: number | null;       // plafond de disque en Go, les plus anciennes parties sont supprimées en premier ; null = illimité (défaut : 20)
  makeRoomForRecording: boolean;    // au début d'une partie, supprimer les données les plus anciennes si le disque ou le plafond n'a plus la place d'un enregistrement (défaut : true)
  autoGenerateClips: boolean;       // générer les clips d'une partie dès qu'elle se termine (défaut : true), voir ADR 0009
}

interface StorageUsage {
  totalBytes: number;               // vidéos sources + clips de toutes les parties
  recordingsBytes: number;
  clipsBytes: number;
  games: number;                    // nombre de parties stockées
}

/** Résultat de l'analyse des ultimates d'une partie. */
interface UltimateStatus {
  state: "ok" | "unrecognized" | "notAnalyzed";
  casts: number;                    // ultimates détectés (state "ok")
  hudScale: number | null;          // échelle d'interface retrouvée
}
```

`unrecognized` : l'interface du jeu n'a pas pu être localisée dans la vidéo (échelle, résolution ou disposition inhabituelles) ; aucun ultimate n'a été produit.
`notAnalyzed` : l'analyse n'a pas encore eu lieu pour cette partie (clips non générés, ou vidéo source absente).

```ts
interface Status {
  state: "idle" | "inGame";
  recording: boolean;
  ffmpegAvailable: boolean;
}
```

## Évènements (backend → interface)

- `status-changed` : charge utile `Status`, émis à chaque changement.
- `clips-progress` : `{ sessionId: string; done: number; total: number }` pendant `generate_clips` et pendant une génération automatique (l'interface suit alors la progression, sauf l'évènement final `done = total`).
- `clips-generated` : `{ sessionId: string; error: string | null }` à la fin d'une génération **automatique** (réussie ou non) ; l'interface rafraîchit alors les parties et les clips. `generate_clips` est refusée (« déjà en cours ») pour une partie déjà en cours de génération.

## Lecture des fichiers

Les fichiers clip sont lus dans une balise `<video>` via `convertFileSrc(clip.file)` (protocole `asset`).

## Données externes (interface seulement)

Noms et icônes de champions, icônes d'items : Data Dragon (`https://ddragon.leagueoflegends.com`). La version courante vient de `/api/versions.json`.
