# ADR 0009 — Génération automatique des clips à la fin de chaque partie

- Statut : accepté
- Date : 2026-11-01

## Contexte

Il fallait cliquer sur « Générer les clips » après chaque partie. On veut que ce soit fait à la fin de la partie, sans gêner la partie suivante (exigences N1 à N3 : silence, impact minimal, aucune perturbation).

## Décision

- Option `autoGenerateClips`, **activée par défaut** (réglages, avec les autres options de stockage). Les fichiers de réglages d'anciennes versions la reçoivent à `true`.
- Un fil d'arrière-plan (`runtime::spawn_auto_generation`) surveille l'état du jeu une fois par seconde (lecture d'un verrou, aucun accès disque). Quand une partie se termine (« en partie » → « inactif »), il demande à `lgr-engine::AutoGenerator` la prochaine partie à générer, puis appelle la même fonction que le bouton (`generation::generate`). L'interface est prévenue par l'évènement `clips-generated` et rafraîchit les parties et les clips.
- **Jamais pendant une partie** : si une nouvelle partie démarre avant, la génération attend sa fin. Une génération déjà commencée n'est en revanche pas interrompue ; elle tourne à priorité réduite comme une génération manuelle.
- **Seules les parties qui se terminent pendant que l'application tourne** sont générées : `AutoGenerator` retient les parties déjà stockées au démarrage, pour qu'ouvrir l'application ne lance jamais un long arriéré. Les parties enregistrées avant restent à générer à la main.
- Une partie n'est examinée qu'une fois : un échec n'est pas réessayé en boucle (le message est affiché et le bouton reste disponible).
- Option désactivée : les parties qui se terminent à ce moment-là sont ignorées, elles ne sont pas générées en bloc au moment de la réactiver.
- Une partie ne peut pas être générée deux fois en même temps (`InFlight`) : un clic manuel pendant une génération automatique est refusé avec un message, et l'interface désactive le bouton dès qu'elle reçoit la progression.
- Le réglage « supprimer la vidéo source après les clips » s'applique comme pour une génération manuelle.

## Limites

- La génération peut démarrer pendant que le client est en file d'attente ou à l'écran de chargement de la partie suivante : le jeu ne publie son API qu'une fois la partie lancée, donc l'application ne le sait pas encore. Le travail est à priorité basse, mais il n'est pas nul pendant ces secondes.
- Un échec de génération automatique est signalé quand une partie est affichée ; sans partie sélectionnée il reste silencieux.
