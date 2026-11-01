# ADR 0009 — Génération automatique des clips à la fin de chaque partie

- Statut : accepté
- Date : 2026-11-01

## Contexte

Il fallait cliquer sur « Générer les clips » après chaque partie. On veut que ce soit fait à la fin de la partie, sans gêner la partie suivante (exigences N1 à N3 : silence, impact minimal, aucune perturbation).

## Décision

- Option `autoGenerateClips`, **activée par défaut** (réglages, avec les autres options de stockage). Les fichiers de réglages d'anciennes versions la reçoivent à `true`.
- Un fil d'arrière-plan (`runtime::spawn_auto_generation`) surveille l'état du jeu une fois par seconde (lecture d'un verrou, aucun accès disque). Quand une partie se termine (« en partie » → « inactif »), ou 15 s après le lancement, puis toutes les 30 s tant qu'aucune partie n'est en cours, il demande à `lgr-engine::AutoGenerator` la prochaine partie à générer, puis appelle la même fonction que le bouton (`generation::generate`). L'interface est prévenue par l'évènement `clips-generated` et rafraîchit les parties et les clips.
- **Jamais pendant une partie** : si une nouvelle partie démarre avant, la génération attend sa fin. Une génération déjà commencée n'est en revanche pas interrompue ; elle tourne à priorité réduite comme une génération manuelle.
- **Parties concernées (2026-11-01, v0.1.14)** : toute partie terminée dont la vidéo existe et dont les clips ne sont pas générés (`SessionStore::pending_generation` : `session.json` et `recording.mkv` présents, pas de `clips.json` ; seuls des noms de fichiers sont lus), **la plus récente d'abord, une à la fois**. La v0.1.12 ignorait les parties déjà stockées au démarrage, pour éviter un long arriéré ; c'était une erreur : une partie terminée pendant que l'application était fermée ou en cours de mise à jour n'était jamais générée. L'arriéré est de toute façon borné par le plafond de disque, qui supprime les vidéos les plus anciennes. Avec « Supprimer la vidéo source après la génération », la vidéo de chaque partie traitée est supprimée ensuite.
- Une partie n'est essayée qu'une fois par lancement : un échec n'est pas réessayé en boucle (le message est affiché et le bouton reste disponible), mais elle l'est de nouveau au lancement suivant.
- Option désactivée : rien n'est généré ; en la réactivant, les parties sans clips sont traitées à la prochaine vérification (30 s au plus).
- Une partie ne peut pas être générée deux fois en même temps (`InFlight`) : un clic manuel pendant une génération automatique est refusé avec un message, et l'interface désactive le bouton dès qu'elle reçoit la progression.
- Le réglage « supprimer la vidéo source après les clips » s'applique comme pour une génération manuelle.

## Limites

- La génération peut démarrer pendant que le client est en file d'attente ou à l'écran de chargement de la partie suivante : le jeu ne publie son API qu'une fois la partie lancée, donc l'application ne le sait pas encore. Le travail est à priorité basse, mais il n'est pas nul pendant ces secondes.
- Un échec de génération automatique est signalé quand une partie est affichée ; sans partie sélectionnée il reste silencieux.
