# ADR 0003 — Ultimates et Flash détectés par analyse de l'interface enregistrée

- Statut : accepté
- Date : 2026-10-29

## Contexte

Les clips d'ultimates sont demandés, mais **aucune API Riot n'expose les lancements de sorts**, ni en partie ni après :

- la Live Client API liste 14 types d'événements (kills, objectifs, tours, multikills, ace…), aucun lancement de sort ; l'état ne contient que `abilityLevel` et `cooldownReduction` ;
- la timeline Match-V5 n'a pas d'événement de cast (seulement, pour les kills, les dégâts reçus avec `spellName` / `spellSlot`, donc seulement les sorts qui ont touché une victime, et il faut une clé API) ;
- la Replay API ne pilote que la caméra.

La politique Riot interdit par ailleurs de suivre les cooldowns d'ultimates **ennemis**.

## Décision

Détecter **les ultimates du joueur local** dans la vidéo déjà enregistrée. Quand un sort part en cooldown, des chiffres blancs (secondes restantes) apparaissent au centre de son icône et y restent jusqu'à ce qu'il soit de nouveau prêt. Un lancement est le front montant de « chiffres visibles ».

- **Extraction** (`lgr-media::hud`) : ffmpeg ne décode que les images clés (une par seconde, voir ADR 0002) avec `-skip_frame nokey -threads 2`, découpe la rangée des quatre icônes (géométrie en unités de hauteur d'écran, donc indépendante de la résolution) et sort des images grises brutes. Une partie de 16 minutes s'analyse en environ 18 s à priorité réduite, sans fenêtre.
- **Détection** (`lgr-core::ultimate`) : voir « Ultimates : le voile de cooldown » ci-dessous. La première version, à hystérésis sur les chiffres (2 échantillons pour allumer, 3 pour éteindre, seuils 0,03 / 0,01), a été remplacée le 2026-11-02 : elle ne tenait pas compte des icônes dont l'illustration est presque blanche. Des chiffres déjà présents au début de l'enregistrement ne sont pas un lancement.
- **Cache** : le résultat est écrit dans `ultimates.json` (temps de jeu), la vidéo n'est analysée qu'une fois ; un échec n'est pas mis en cache et sera retenté.
- **Clips** : fenêtre de 5 s avant et 11 s après (l'effet d'un ultimate dure plus qu'un kill), thème `ultimate`, rôle `caster`.

## Validation

Sur deux enregistrements réels (Qiyana), les lancements détectés sont espacés de 109 à 329 s, cohérents avec le cooldown de l'ultimate, et plusieurs coïncident à ±0,1 s avec un kill de la chronologie. Une image extraite au lancement montre l'icône R avec un cooldown de 95 s.

## Auto-calibration (échelle de l'interface)

La géométrie (pas entre icônes 0,04076, hauteur d'icône 0,0358, rangée à 0,0632 du bas, en unités de hauteur d'écran) a été mesurée à l'échelle d'interface 1,0. Comme la correspondance entre le réglage du jeu et cette échelle n'est pas connue, l'échelle est **calibrée sur chaque enregistrement** :

- une seule passe ffmpeg (`split` / `crop` / `hstack` / `vstack`) extrait les icônes pour 11 échelles candidates (0,8 à 2,0) ;
- pour chaque candidat, le score est la **masse des chiffres** vus sur Q, W et E : la somme des ratios de pixels blancs sur toutes les secondes où des chiffres sont visibles, multipliée par l'échelle² (un candidat plus petit recadre plus serré et agrandit les chiffres de 1/échelle²) ;
- on retient le centre du plateau des candidats dont le score atteint 0,7 × le meilleur, à condition que le meilleur ait au moins 5 épisodes et une marge de 2× sur les candidats hors du voisinage ;
- sinon la partie est marquée **« interface non reconnue »** (`UltimateStatus.state = unrecognized`) : le résultat négatif est mis en cache, l'interface l'affiche, et aucun clip d'ultimate n'est inventé.

**Pourquoi la masse et non le nombre d'épisodes (correction du 2026-11-01).** Le nombre d'épisodes était le premier score. Sur vos enregistrements récents, il se trompait : un candidat voisin voit le bord de grands chiffres et compte presque autant d'épisodes (75 contre 105), ce qui rendait la calibration ambiguë et refusait l'interface. Sur deux parties il choisissait même une mauvaise échelle (1,1 ou 1,2 au lieu de 1,0 ; mesuré à l'écran : icône R de 52 px de large, soit 0,0358 × 1440). La masse sépare nettement mieux : 157 contre 81 et 53 sur la partie qui échouait. Avec la masse, la calibration choisit 1,0 sur 4 des 5 enregistrements restants ; le cinquième ne démarre pas sur un écran de chargement et n'est pas reconnu. Les analyses enregistrées par une ancienne version (sans sorts d'invocateur) sont refaites à la prochaine génération si la vidéo existe encore.

Les tests `crates/engine/tests/hud_calibration.rs` utilisent des vidéos synthétiques (traits fins de 2,5 unités) en 1080p, 1440p à l'échelle 1,3, 720p à 1,5, ultrawide 3440×1440 et sans interface. À 720p, une cellule de 32×32 est construite à partir d'environ 38 pixels dessinés sur des pixels entiers : on y accepte un candidat voisin de l'échelle vraie.

## Flash et sorts d'invocateur

Les sorts d'invocateur D et F sont lus avec la même géométrie (icônes de 0,0271 de la hauteur d'écran, centres à +0,04035 et +0,07125 du centre, à 0,0668 du bas), mesurée sur une capture réelle.

**Les chiffres ne permettent pas de les suivre** : l'icône prête du Flash contient elle-même des traînées blanches ; la fraction de pixels clairs est la même avec et sans chiffres (≈ 0,08 à 0,11). Le cooldown **assombrit** en revanche l'icône (luminosité moyenne ≈ 0,42 contre 0,54 quand elle est prête). Un lancer est donc le début d'un assombrissement qui dure au moins 90 s :

- niveau « prêt » : 90e percentile de la luminosité de l'icône après le début de la partie (l'écran de chargement n'est pas le HUD) ; assombri : sous 92 % de ce niveau ; il faut au moins 8 % d'écart entre le niveau prêt et le niveau le plus sombre ;
- 2 échantillons sombres pour confirmer, 3 clairs pour terminer (un éclair isolé ne coupe pas un cooldown) ;
- moins de 90 s est autre chose : HUD grisé d'une mort, sort désactivé (croix rouge), effet d'écran. Un cooldown déjà en cours au début de la partie n'est pas un lancer ;
- l'estimation du lancer est la première seconde sombre − 0,5 s.

Quel emplacement porte le Flash ? Les identifiants de sorts sont enregistrés dans l'ordre (`spell_keys`) : le Flash est l'emplacement où se trouve `SummonerFlash`. Sans les deux identifiants, l'emplacement est inconnu et aucun clip Flash n'est produit.

**Validation sur de vraies parties.** Sur la partie de Cho'Gath, le Flash est détecté à 226,5 s de vidéo et la vidéo affiche « 227 » (secondes restantes) à 300 s : cooldown de 300,5 s, celui du Flash. Les écarts entre lancers sur 3 parties (270 à 400 s) sont cohérents avec ce cooldown. Dans le clip généré, l'icône D passe de « prête » à « 300 » à 6 s du début, comme prévu.

## Ultimates : le voile de cooldown (2026-11-02)

**Problème constaté sur une partie de Nocturne** : des ultimates manquaient et certains clips n'en étaient pas. L'illustration de l'icône R prête de Nocturne a un fond quasi blanc (≥ 225 sur 255) : le ratio de « chiffres » restait entre les seuils de l'hystérésis (0,03 / 0,01), donc aucune fin de cooldown n'était jamais vue et les lancers suivants étaient perdus ; un changement d'illustration provoquait en plus de faux lancers (par exemple à 1134 s, en plein cooldown). La luminosité ne marche pas non plus pour R : le voile de Nocturne est bleu, aussi clair que l'illustration.

**Décision** : un ultimate lancé est un **voile plat** sur l'icône, confirmé par des chiffres.

- *Voile* : l'écart-type des pixels de l'icône hors de la bande des chiffres (lignes 3 à 10 et 25 à 29, colonnes 4 à 27 de la cellule 32×32) tombe d'environ 55 (prêt) à environ 44 (cooldown), quelle que soit l'illustration. Même détection que pour le Flash (`veil_episodes`, partagée) : niveau « prêt » au 90e percentile après le début de la partie, voile sous 92 % de ce niveau, au moins 8 % d'écart, 2 échantillons pour confirmer, 3 pour terminer. Un épisode dure au moins 15 s (le voile est visible 30 à 39 s sur un cooldown de plus de 80 s ; les éclairs d'un ou deux échantillons sont écartés).
- *Confirmation* : des chiffres **stricts** (blanc ≥ 240, `strong_digit_ratio`) doivent apparaître dans au moins 2 des 4 premiers échantillons de l'épisode, avec un seuil de 0,0136 (3 pixels sur 220). Une icône grisée faute de mana a un voile mais pas de chiffres : elle n'est pas comptée.
- *Repli* : si l'écart-type ne change jamais (illustration plate), on retombe sur l'ancienne détection par chiffres.
- *Version d'analyse* : `ANALYSIS_VERSION = 2` est stocké dans `ultimates.json` (`analysis_version`) ; les analyses plus anciennes sont refaites à la prochaine génération si la vidéo existe.

**Validation.** Sur l'enregistrement Qiyana (981 s) : exactement 7 lancers (99,5 ; 258,5 ; 406,5 ; 487,5 ; 630,5 ; 721,5 ; 916,5 s), sans le faux lancer de 283,5 s que donnait la variante « chiffres stricts seuls ». Sur les clips Nocturne, l'écart-type chute exactement aux images de lancer (475, 883, 1058, 1355, 1613, 1791 s). La vidéo de Nocturne ayant été supprimée, le détecteur n'a pas pu y être relancé de bout en bout. Les tests synthétiques dessinent maintenant une illustration sur R qui disparaît pendant le cooldown.

## Ultimates : lancers fantômes sous un cooldown en cours (2026-11-04)

**Problème constaté sur une partie de Lillia** : 17 lancers détectés en 38 min, dont des écarts de 59 à 66 s alors que l'ultimate a un cooldown de 150 s (au niveau 6). Sur les 6 clips d'ultimate générés, 4 (509, 1065, 1372, 1720 s) montrent l'icône R **déjà** en cooldown (« 46 », « 43 »…) dès le début de la fenêtre : ce sont des lancers fantômes. Les 2 autres (972, 2054 s) montrent des chiffres qui apparaissent à 5 s, comme attendu.

**Cause** : les chiffres durent tout le cooldown (150 s), mais le voile n'est franchement visible qu'au début ; il peut repasser sous le seuil en cours de route et ouvrir un nouvel épisode, que les chiffres déjà présents « confirmaient ».

**Décision** : un lancer part d'une icône **prête**. Un épisode de voile n'est compté que si au plus 1 des 4 échantillons qui précèdent son début (la seconde adjacente exclue) montre des chiffres stricts. `ANALYSIS_VERSION = 3`.

**Limites de la validation** : la vidéo source de cette partie a été supprimée ; la règle a été vérifiée sur les 6 clips (4 rejetés, 2 gardés), pas sur l'enregistrement complet, et la cause exacte du franchissement de seuil dans la vidéo brute n'est pas mesurée. Elle ne suit pas les ultimates qui ne passent pas par un cooldown visible dans le HUD (sorts à relance sans cooldown, changement de forme) : non vérifié.

## Limites

- Précision de ±0,5 s : les images clés tombent chaque seconde, et l'apparition des chiffres est située au milieu de la seconde précédant leur première observation (mesuré : chiffres réellement apparus à 400,2 s, vus à l'échantillon 401, estimation 400,5). La fenêtre du clip (5 s avant, 11 s après) l'absorbe largement.
- La géométrie suppose la disposition par défaut de l'interface (rangée de sorts centrée en bas) : seule l'échelle varie. Une autre disposition donnera « interface non reconnue ». Le 1080p, l'ultrawide et les autres échelles ne sont validés que sur des vidéos synthétiques.
- Seuls **vos** ultimates et **vos** Flashs sont détectés (c'est ce qui est visible dans votre interface). Un ultimate qui ne se lance pas (pas de mana, silence) n'affiche pas de cooldown et n'est pas compté.
- Le Flash d'une partie dont la vidéo source a été supprimée ne peut plus être détecté.
- Un Flash lancé juste après un sort désactivé (croix rouge) peut être daté 1 à 2 s trop tôt.
- Les lancers de sorts d'invocateur à cooldown court (Smite, moins de 90 s) ne sont pas détectés.
