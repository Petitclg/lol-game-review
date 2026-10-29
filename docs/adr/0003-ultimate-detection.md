# ADR 0003 — Ultimates détectés par analyse de l'interface enregistrée

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
- **Détection** (`lgr-core::ultimate`) : fonction pure à hystérésis (2 échantillons pour allumer, 3 pour éteindre, seuils 0,03 / 0,01), car le nombre de chiffres change pendant le cooldown et fait osciller le ratio. Des chiffres déjà présents au début de l'enregistrement ne sont pas un lancement.
- **Cache** : le résultat est écrit dans `ultimates.json` (temps de jeu), la vidéo n'est analysée qu'une fois ; un échec n'est pas mis en cache et sera retenté.
- **Clips** : fenêtre de 5 s avant et 11 s après (l'effet d'un ultimate dure plus qu'un kill), thème `ultimate`, rôle `caster`.

## Validation

Sur deux enregistrements réels (Qiyana), les lancements détectés sont espacés de 109 à 329 s, cohérents avec le cooldown de l'ultimate, et plusieurs coïncident à ±0,1 s avec un kill de la chronologie. Une image extraite au lancement montre l'icône R avec un cooldown de 95 s.

## Auto-calibration (échelle de l'interface)

La géométrie (pas entre icônes 0,04076, hauteur d'icône 0,0358, rangée à 0,0632 du bas, en unités de hauteur d'écran) a été mesurée à l'échelle d'interface 1,0. Comme la correspondance entre le réglage du jeu et cette échelle n'est pas connue, l'échelle est **calibrée sur chaque enregistrement** :

- une seule passe ffmpeg (`split` / `crop` / `vstack`) extrait la rangée d'icônes pour 11 échelles candidates (0,8 à 2,0) ;
- pour chaque candidat, un score compte les épisodes de cooldown vus sur Q, W et E (ils se lancent bien plus souvent que l'ultimate) ;
- on retient le centre du plateau des candidats dont le score atteint 0,7 × le meilleur, à condition d'avoir au moins 5 épisodes et une marge de 2× sur les candidats hors du voisinage ;
- sinon la partie est marquée **« interface non reconnue »** (`UltimateStatus.state = unrecognized`) : le résultat négatif est mis en cache, l'interface l'affiche, et aucun clip d'ultimate n'est inventé.

Les tests `crates/engine/tests/hud_calibration.rs` utilisent des vidéos synthétiques (traits fins de 2,5 unités) en 1080p, 1440p à l'échelle 1,3, 720p à 1,5, ultrawide 3440×1440 et sans interface. Sur 6 enregistrements réels (2560×1440, sans bordure), la calibration choisit toujours l'échelle 1,0 avec une grande marge, et les lancements trouvés sont identiques à ceux de la version à échelle fixe.

## Limites

- Précision de ±0,5 s : les images clés tombent chaque seconde, et l'apparition des chiffres est située au milieu de la seconde précédant leur première observation (mesuré : chiffres réellement apparus à 400,2 s, vus à l'échantillon 401, estimation 400,5). La fenêtre du clip (5 s avant, 11 s après) l'absorbe largement.
- La géométrie suppose la disposition par défaut de l'interface (rangée de sorts centrée en bas) : seule l'échelle varie. Une autre disposition donnera « interface non reconnue ». Le 1080p, l'ultrawide et les autres échelles ne sont validés que sur des vidéos synthétiques.
- Seuls **vos** ultimates sont détectés (c'est ce qui est visible dans votre interface). Un ultimate qui ne se lance pas (pas de mana, silence) n'affiche pas de cooldown et n'est pas compté.
- Les mêmes principes permettraient de détecter les lancements de Flash du joueur (icône du sort d'invocateur), non fait à ce stade.
