# ADR 0001 — Clips issus d'un enregistrement pendant la partie

- Statut : accepté
- Date : 2026-10-28

## Contexte

Le besoin est de générer des clips vidéo de moments forts, silencieusement, légèrement, sans gêner une partie lancée en parallèle.

## Options

1. **Re-rendre un replay** via le client de replay LoL : lourd, fenêtre et son, indisponible pendant une partie, limité au patch courant.
2. **Enregistrer pendant la partie** avec un tampon circulaire et un encodeur matériel, puis découper selon les événements de la Live Client Data API.

## Décision

Option 2. Le replay hors-ligne pourra être étudié plus tard comme mode expérimental, hors partie.

## Conséquences

- Les clips montrent la vue réelle du joueur, avec l'interface du jeu.
- Pas de vidéo pour les parties antérieures à l'installation.
- Le recorder est le principal risque technique : un spike mesure l'impact et la compatibilité Vanguard avant tout développement poussé.
