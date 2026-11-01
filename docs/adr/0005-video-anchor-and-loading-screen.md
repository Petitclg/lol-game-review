# ADR 0005 — Ancre vidéo et écran de chargement

- Statut : accepté
- Date : 2026-11-01

## Contexte

Le moteur convertit un temps de jeu en position dans la vidéo avec une **ancre** : le temps de jeu qui correspond à l'instant 0 de la vidéo. Elle est estimée en comparant le temps de jeu rapporté par la Live Client API à l'horloge murale depuis le début de l'enregistrement, et en gardant le **maximum** des estimations (la latence HTTP ne peut que retarder le temps de jeu observé).

Défaut constaté sur les parties réelles : les clips étaient décalés d'une valeur **constante par partie** (1 à 92 s) et on n'y voyait plus l'action. L'API Live répond déjà pendant l'écran de chargement, avec une horloge de jeu **figée à 0**, et l'enregistrement démarre à ce moment. Les premiers échantillons (temps de jeu figé, peu de temps écoulé) donnaient donc l'estimation la plus haute (≈ 0), alors que la bonne valeur est −(durée du chargement).

Vérifié sur 17 enregistrements : à l'écran, l'horloge de jeu à 60 s de vidéo valait 00:59, 00:50, 00:41 et 00:00 (chargement de 70 s) selon la partie.

## Décisions

1. **Nouvelles parties** : un échantillon ne compte pour l'ancre que si le temps de jeu a avancé depuis le précédent. Les échantillons figés (chargement, pause) sont ignorés. Tant qu'aucun échantillon valide n'existe, le premier sert de repli et `Session.anchor_verified` reste faux.
2. **Parties déjà enregistrées** : avant de générer les clips, si l'ancre n'est pas vérifiée et proche de zéro (|ancre| ≤ 5 s ; au-delà, l'enregistrement a démarré en cours de partie et l'ancre est juste), on cherche la fin de l'écran de chargement **dans la vidéo** (`lgr-core::loading`, `lgr-media::start`).
   - Une passe ffmpeg ne décode que les images clés (une par seconde) des 10 premières minutes, réduites en 32×18 gris.
   - L'écran de chargement est quasi statique (différence moyenne entre images clés < 3), puis la coupe vers le jeu fait un saut (≥ 10). La fin du chargement est la première image après le saut ; l'ancre vaut 0,5 − cette valeur.
   - Pas de coupe nette, ou chargement de moins de 3 s : on ne touche à rien.
3. Quand l'ancre change, ce qui en dépend est **supprimé** : analyse des ultimates (temps de jeu) et clips déjà générés (mal placés, et réutilisés sinon d'après leur seul identifiant). La session est marquée vérifiée. Si la vidéo est absente ou illisible, rien n'est modifié et la vérification sera retentée.

## Validation

- 17 enregistrements réels : 15 ont un écran de chargement (4 à 92 s) et 2 n'en ont pas (ceux dont l'ancre était déjà juste). La fin trouvée recoupe deux mesures indépendantes : l'horloge lue à l'écran (70, 10 et 19 s) et la durée de la vidéo moins celle de la partie (70, 16, 91, 5, 81, 10, 82, 19 s, la vidéo s'arrêtant à la fin de la partie).
- Test de bout en bout en bac à sable sur la copie d'une partie : ancre corrigée de 0 à −9,5 s, clips régénérés, horloge de jeu lue au début de 4 clips identique à l'attendu (04:13, 07:15, 10:28, 08:32), et le kill survient à 8 s dans le clip.

## Limites

- Les clips d'une partie dont la vidéo a été supprimée (option « supprimer la vidéo après les clips ») ne peuvent plus être corrigés.
- Précision ≈ ±1 s (une image clé par seconde), absorbée par les marges avant et après les clips.
- Une pause de la partie décale ensuite la vidéo de sa durée : non traité (rare).
