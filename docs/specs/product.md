# Spécification produit

## Objectif

Permettre au joueur de revoir, par thématique, les clips des moments où il intervient, avec le contexte de jeu (items, sorts, cooldowns).

## Exigences fonctionnelles

| ID | Exigence |
|----|----------|
| F1 | Détecter le compte actif du client LoL (LCU, `lockfile`). |
| F2 | Lister les parties récentes avec un résumé (champion, résultat, KDA, durée). |
| F3 | Choisir une partie et générer l'analyse de ses clips. |
| F4 | Clips par thème : kills, morts, assists, objectifs, tours, ultimates. |
| F5 | Chaque clip montre l'action du joueur. |
| F6 | Chaque clip expose les items et les cooldowns des sorts du joueur, des alliés et des adversaires. |
| F7 | Mise à jour automatique vers la dernière release au démarrage. |

## Exigences non fonctionnelles

| ID | Exigence |
|----|----------|
| N1 | Génération silencieuse : pas de son, pas de clignotement, aucune fenêtre au premier plan. |
| N2 | Impact minimal : priorité basse, encodeur matériel, pas de dégradation de FPS en partie. |
| N3 | Ne jamais perturber le lancement ou le déroulement d'une partie. |
| N4 | Conformité à la politique développeur de Riot Games. |
| N5 | Couverture de tests élevée sur la logique métier ; CI obligatoire avant fusion. |

## Contraintes techniques connues

1. **Source vidéo.** Les parties passées ne sont pas filmées. Les clips viennent d'un enregistrement léger fait pendant la partie ; les parties jouées avant l'installation n'ont que le résumé (voir [ADR 0001](../adr/0001-recording-during-game.md)).
2. **Ultimates (F4).** La Live Client Data API n'expose pas les lancements de sorts. Détecter les ultimates exige une autre source, à investiguer (spike). Le thème est reporté tant qu'aucune source fiable n'est validée.
3. **Cooldowns adverses (F6).** Estimés (niveaux de sorts, ability haste, Data Dragon), donc approximatifs.
4. **Anti-cheat Vanguard.** La compatibilité de la capture doit être vérifiée tôt (spike recorder).
5. **Clé API Riot.** Non requise au MVP. Match-V5 est un fournisseur optionnel ultérieur.

## Découpage

1. `lgr-core` : logique métier (fait : événements, fenêtres de clips, cooldowns).
2. Client LCU : compte actif, parties récentes.
3. Moniteur de partie en cours et stockage SQLite.
4. Recorder (spike puis implémentation).
5. Extraction des clips.
6. Interface Tauri.
7. CI/CD, release, mise à jour automatique.
