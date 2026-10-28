# ADR 0002 — Stockage en fichiers JSON et ffmpeg externe

- Statut : accepté
- Date : 2026-10-28

## Contexte

Il faut persister, par partie, la chronologie, les instantanés de joueurs, l'enregistrement et les clips, et enregistrer / découper de la vidéo avec un impact minimal.

## Décisions

1. **Un dossier par session** (`session.json`, `recording.mkv`, `clips.json`, `clips/*.mp4`) plutôt que SQLite. Les données sont écrites une fois en fin de partie et relues en bloc ; il n'y a ni requête relationnelle ni écriture concurrente. Les écritures sont atomiques (fichier temporaire puis renommage). Les identifiants de session sont validés strictement (`[A-Za-z0-9_-]`, 64 caractères max) pour empêcher toute traversée de chemin depuis l'interface.
2. **ffmpeg en processus externe** (build LGPL embarqué) plutôt qu'une liaison native : isolation de la licence, pas de code `unsafe`, arrêt propre par `q` sur l'entrée standard. Capture `ddagrab` (Desktop Duplication), encodeur matériel détecté au démarrage par un essai réel, repli CPU 720p sinon. Fichier source en Matroska (robuste à un arrêt brutal), clips en MP4 (lisibles par WebView2).
3. **Découpe sans ré-encodage** (`-c copy`) grâce à une image clé par seconde : rapide, sans charge CPU/GPU, précision d'environ une seconde absorbée par le pré-roll.
4. **Synchronisation vidéo / jeu** : le temps de jeu à l'instant 0 de la vidéo est estimé comme le maximum, sur tous les échantillons, de `gameTime - temps écoulé depuis le début de l'enregistrement` (le maximum correspond à la latence la plus faible).

## Conséquences

- Pas de migration de schéma complexe ; un champ ajouté doit rester compatible avec les sessions existantes (valeurs par défaut serde).
- La précision de synchronisation dépend du délai de démarrage de ffmpeg (estimé à 300 ms) ; à calibrer en conditions réelles.
- Un ffmpeg ne doit jamais survivre à l'application : chaque enregistrement écrit `recording.mkv.pid`, les orphelins sont arrêtés au démarrage suivant (`reap_orphans`), et ffmpeg s'arrête de lui-même après 4 h (`-t 14400`). Un job object Windows serait plus strict mais exigerait du code `unsafe`.
