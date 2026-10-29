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
- **Clips** : fenêtre de 5 s avant et 8 s après (l'effet d'un ultimate dure plus qu'un kill), thème `ultimate`, rôle `caster`.

## Validation

Sur deux enregistrements réels (Qiyana), les lancements détectés sont espacés de 109 à 329 s, cohérents avec le cooldown de l'ultimate, et plusieurs coïncident à ±0,1 s avec un kill de la chronologie. Une image extraite au lancement montre l'icône R avec un cooldown de 95 s.

## Limites

- Précision de ±0,5 s : les images clés tombent chaque seconde, et l'apparition des chiffres est située au milieu de la seconde précédant leur première observation (mesuré : chiffres réellement apparus à 400,2 s, vus à l'échantillon 401, estimation 400,5). La fenêtre du clip (5 s avant, 8 s après) l'absorbe largement.
- La géométrie a été mesurée avec l'interface par défaut en 16:9 sur l'écran principal : une échelle d'interface personnalisée ou une autre disposition ne trouvera aucun chiffre, et il n'y aura simplement pas d'ultimates.
- Seuls **vos** ultimates sont détectés (c'est ce qui est visible dans votre interface). Un ultimate qui ne se lance pas (pas de mana, silence) n'affiche pas de cooldown et n'est pas compté.
- Les mêmes principes permettraient de détecter les lancements de Flash du joueur (icône du sort d'invocateur), non fait à ce stade.
