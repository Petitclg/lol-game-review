# ADR 0004 — Capture de la fenêtre du jeu et maîtrise du stockage

- Statut : accepté
- Date : 2026-10-30

## Contexte

Un enregistrement mesuré pèse environ 1 Go par partie (5 Mbit/s), et les clips générés s'y ajoutent. Par ailleurs la capture du bureau entier enregistre un mauvais écran dès que le jeu n'est pas sur l'écran principal, et l'analyse de l'interface (ADR 0003) dépend de l'image.

## Décisions

### Capture

- La fenêtre `League of Legends` est capturée par `gfxcapture` (Windows Graphics Capture) : zone client seulement, curseur masqué, 30 i/s, quel que soit l'écran.
- Repli automatique sur `ddagrab` (bureau principal) si la fenêtre est introuvable : ffmpeg échoue en environ 0,3 s, ce que détecte la vérification des 400 ms de `Recording::start_from`.
- `LGR_CAPTURE_EXE` remplace le nom du processus (valeur filtrée).
- Une image clé forcée **chaque seconde** (`-force_key_frames expr:gte(t,n_forced*1)`, plus `-forced-idr 1` pour NVENC) : `-g 30` seul dérive (1,03 à 1,07 s) et fausserait la datation des clips et de l'analyse. `-fps_mode cfr` est proscrit, il bloque sur une fenêtre statique.

### Stockage (réglages « Stockage », tous activés par défaut)

1. **Supprimer la vidéo source après les clips** : seulement si chaque clip a son fichier et que l'analyse des ultimates est enregistrée (sinon on garderait des clips sans pouvoir les régénérer correctement). Mesuré : 917 Mo → 275 Mo.
2. **Compresser les clips** en tâche de fond, uniquement quand aucune partie n'est en cours : NVENC `p6`, `vbr`, `cq 36` (AMF et Quick Sync ont des équivalents, non testés), jamais d'encodeur CPU. Un clip n'est remplacé que s'il gagne au moins 8 % ; `compressed.json` évite de recommencer.
3. **Plafond de rétention** (20 Go par défaut, 1 à 100 000 Go) : on supprime d'abord la partie la plus ancienne, vidéo avant clips, en épargnant toujours la plus récente ; les orphelins périmés sont nettoyés. Les métadonnées (chronologie) restent.

Gain de compression mesuré sur un clip réel (NVENC) : `cq 30` ne gagne rien (9,2 → 9,2 Mo), `cq 33` 71 % (SSIM 0,972), `cq 36` 50 % (SSIM 0,967), `cq 38` 41 %. `cq 36` garde l'interface lisible. Sur une partie complète : 275 → 158 Mo (−43 %), soit 917 → 158 Mo avec la suppression de la source (−83 %).

## Validation

Test de bout en bout en bac à sable sur la copie d'une vraie partie : 27 clips en 36 s, source supprimée, compression en arrière-plan puis lecture de tous les clips vérifiée (`ffprobe`), plafond de 1 Go appliqué (clips de la partie la plus ancienne supprimés, la plus récente épargnée). `crates/media/tests/window_capture.rs` (GPU requis, ignoré sinon) vérifie une image clé exactement par seconde et le repli sur le bureau. Le build ffmpeg LGPL de la release contient `gfxcapture`, `-forced-idr` et `-cq`.

## Limites

- La capture de fenêtre n'a pas été validée sous Riot Vanguard avec une vraie partie ; le repli sur le bureau reste disponible.
- Compression AMF et Quick Sync non testées ; sans encodeur matériel, les clips ne sont pas compressés.
