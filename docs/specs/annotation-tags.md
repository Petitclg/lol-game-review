# Catalogue des étiquettes d'annotation

Les étiquettes se posent sur un **moment** d'un clip (voir [ADR 0008](../adr/0008-clip-merging-and-annotations.md)). Le catalogue vit dans l'interface (`src/annotations.ts`) ; le backend ne stocke que les **identifiants**. Un identifiant ne change jamais (il est écrit sur le disque) ; seul le libellé peut évoluer.

Format d'un identifiant : `a-z`, `0-9` et `_`, 48 caractères au plus. Au plus 24 étiquettes par moment, et une note libre de 2 000 caractères au plus.

Nuance : **erreur** (rouge), **bon réflexe** (vert), **constat** (bleu, ni l'un ni l'autre).

## Moments « Mort »

Les erreurs classiques, puis ce qui n'en est pas une.

| Identifiant | Libellé | Nuance |
|---|---|---|
| `face_check` | Face-check | erreur |
| `mechanical_error` | Erreur mécanique | erreur |
| `outnumbered` | Infériorité numérique | erreur |
| `caught` | Catch | erreur |
| `bad_jungle_tracking` | Mauvais jungle tracking | erreur |
| `unexpected_enemy` | Présence d'un ennemi imprévue | erreur |
| `no_vision` | Absence de vision | erreur |
| `greed` | Greed | erreur |
| `cooldowns_down` | Cooldowns clés indisponibles | erreur |
| `lost_trade` | Trade perdant | erreur |
| `solo_engage` | Engage sans équipe | erreur |
| `objective_death` | Mort à l'objectif | erreur |
| `ganked` | Gank subi | erreur |
| `should_recall` | Aurait dû recall | erreur |
| `unavoidable` | Mort inévitable | constat |
| `good_death` | Bonne mort | bon réflexe |

## Autres moments

Kills, assists, objectifs, tours, ultimates, Flashs et moments clés.

| Identifiant | Libellé | Nuance |
|---|---|---|
| `good_trade` | Bon trade | bon réflexe |
| `good_outplay` | Bon outplay | bon réflexe |
| `good_roam` | Bon roam | bon réflexe |
| `easy_kill` | Kill facile / snowball | constat |

## Tous les moments

| Identifiant | Libellé | Nuance |
|---|---|---|
| `good_teamfight` | Bon teamfight | bon réflexe |
| `bad_teamfight` | Mauvais teamfight | erreur |

## Évolution

- **Ajouter** une étiquette : l'ajouter dans `src/annotations.ts` (la liste du moment concerné) et dans ce tableau. Le test `annotations.test.ts` vérifie que les identifiants sont uniques et acceptés par le backend.
- **Renommer** un libellé : sans risque. Ne jamais modifier un identifiant, ou les annotations existantes perdraient leur étiquette (elle resterait affichée sous la forme de l'identifiant, lisible mais sans nuance).
- Une étiquette inconnue (écrite par une version plus récente) reste affichée telle quelle.
