# Instructions pour les agents Copilot — LoL Game Review

Application de bureau Windows (Tauri : backend Rust en workspace de petits crates, interface TypeScript sans framework) qui enregistre les parties de League of Legends et en génère des clips. Dépôt **public** : [Petitclg/lol-game-review](https://github.com/Petitclg/lol-game-review). Vue d'ensemble : [README](../README.md), [spécification produit](../docs/specs/product.md), [contrat IPC](../docs/specs/ipc.md), décisions dans [docs/adr](../docs/adr).

L'utilisateur parle français : répondez en français, concisément. Identifiants et commentaires de code en anglais, textes visibles par l'utilisateur et documentation en français.

## Façon de travailler (règles fixées par l'utilisateur)

1. **Une session par sujet.** Ne pas enchaîner plusieurs chantiers sans lien dans la même session : le contexte renvoyé à chaque appel d'outil fait croître la consommation de tokens. Proposer d'ouvrir une nouvelle session quand le sujet change.
2. **Rester dans le périmètre demandé.** Si vous découvrez un autre problème en chemin, le **signaler** à l'utilisateur et lui laisser décider, sans le corriger d'office. Exception : un défaut qui invalide directement la demande en cours.
3. **Mesurer avant de construire.** Pour tout ce qui dépend de données réelles (enregistrements, HUD, calibration, détection), faire une mesure rapide sur de vraies données avant d'écrire du code. Les cas synthétiques ne remplacent pas cette mesure : plusieurs fonctions ont d'abord été bâties sur une hypothèse fausse (par exemple lire le Flash par chiffres de cooldown, alors que l'icône prête est déjà claire). Quand une première approche échoue, chercher la cause sur **plusieurs** exemples avant de la reprendre.
4. **Tests ciblés pendant le travail, suite complète une seule fois à la fin.** Lancer le test ou le crate concerné pendant l'itération ; la suite complète (Rust + frontend + lint) seulement avant de commiter. Ne pas relancer des builds complets à chaque petite modification.
5. **Économiser le contexte.** Lire un fichier par plages (`view_range`) ou par recherche ciblée plutôt qu'en entier ; ne pas relire ni recréer un gros fichier pour une correction ponctuelle (utiliser des éditions ciblées) ; grouper les lectures indépendantes ; limiter les sorties longues (`Select-Object -Last`, `Select-String`).
6. **Ne jamais surveiller l'activité de l'utilisateur.** Ne pas interroger l'état d'une partie en cours ni attendre qu'elle se termine avant de compiler, tester ou analyser : l'utilisateur interrompra lui-même si c'est trop lourd.
7. **Dire la vérité sur l'état du travail.** Distinguer ce qui est vérifié sur de vraies données de ce qui ne l'est que sur des données synthétiques, signaler les limites et les valeurs approximatives (par exemple l'or estimé), et corriger ouvertement une affirmation fausse (exemple passé : « Régénérer applique la nouvelle durée », qui était inexact).

## Git, publications, secrets

- **Identité git** : `Gaspard Petitclair <gaspard.petitclair@gmail.com>`, configurée **localement dans le dépôt** (la configuration globale est celle d'un autre compte). Vérifier avec `git config user.email` avant de commiter. Messages au format [Conventional Commits](https://www.conventionalcommits.org/) et trailer `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`.
- **Dépôt public : jamais de secret, de nom de joueur réel ni de données de partie réelles** dans le code, les tests, les fixtures ou les documents. Utiliser des données synthétiques ou anonymisées. Le webhook Discord, les clés de signature et leur mot de passe sont des secrets GitHub, jamais des fichiers.
- **Ne jamais publier une release sans l'accord explicite de l'utilisateur** (utiliser `ask_user`). Procédure : monter la version dans `src-tauri/tauri.conf.json` **et** dans `Cargo.toml` du workspace, pousser sur `main`, attendre la CI verte, puis `git tag -a vX.Y.Z` et pousser le tag. Le workflow `Release` construit l'installeur signé, puis appelle `announce.yml` qui poste dans Discord.
- **Notes de release** : écrire `docs/release-notes/vX.Y.Z.md` (français, Markdown Discord, avant de poser le tag) ; sans fichier, le message Discord est généré depuis les commits et reste technique. Voir [ADR 0006](../docs/adr/0006-discord-release-announcement.md).
- Après une release, vérifier la mise à jour de l'application installée (`%LOCALAPPDATA%\com.petitclg.lolgamereview\logs\updater.log`).

## Construire et tester

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # LGR_REQUIRE_FFMPEG=1 fait échouer (au lieu d'ignorer) les tests qui ont besoin de ffmpeg
npm run typecheck; npm run lint; npm test
npx tauri build --no-bundle       # exe de test, après un `npm run build` ; ajouter `npm ci` seulement si les dépendances changent
```

Le test `crates/media/tests/window_capture.rs` dépend du GPU et peut échouer quand il tourne en parallèle du reste ; le relancer seul avant de conclure à une régression. ffmpeg vient de `winget install Gyan.FFmpeg` (non présent dans le PATH : `locate_ffmpeg` le trouve).

## Tester sans toucher à l'installation réelle

Utiliser le **bac à sable** : `LGR_SANDBOX=1` (ou un dossier) lance une instance isolée (données et WebView séparés, aucun enregistrement, aucune mise à jour) qui peut tourner à côté de l'application installée. Pour une vérification de bout en bout, y copier la session d'un vrai enregistrement (`session.json` + `recording.mkv`), jamais la manipuler en place. Piloter l'interface avec `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>` et un script CDP (le créer **avant** de l'exécuter). Ne pas tuer l'application installée pendant qu'elle enregistre (ffmpeg orphelin) ; arrêter un processus avec son PID littéral.

Données utilisateur : `%LOCALAPPDATA%\com.petitclg.lolgamereview` (`sessions/<id>/…`, `settings.json`, `logs/`). L'application installée est dans `%LOCALAPPDATA%\LoL Game Review`.

## Conventions du projet

- **Tests et spécifications** : toute logique métier est dans des crates testés ; une décision d'architecture donne un ADR dans `docs/adr` ; le contrat Rust ↔ TypeScript est dans [ipc.md](../docs/specs/ipc.md) et doit rester synchronisé avec `src/types.ts`. Mettre à jour la documentation concernée avec le code.
- **Lints** : `unsafe_code` interdit et clippy pédant dans le workspace (`-D warnings` en CI). `src-tauri` n'utilise pas les lints du workspace.
- **Interface** : pas de `innerHTML` (règle ESLint) ; textes en français ; `src/mock.ts` fournit un backend simulé pour `npm run dev`.
- **Stockage** : fichiers JSON par session, pas de base de données ([ADR 0002](../docs/adr/0002-storage-and-ffmpeg.md)). Les unités de disque sont binaires (1 Go = 1024³).
- **Aucune détection ne repose sur une API Riot qui n'existe pas.** Les lancers de sorts sont lus dans la vidéo enregistrée ([ADR 0003](../docs/adr/0003-ultimate-detection.md)) ; les aces et l'or estimé viennent des événements ([ADR 0007](../docs/adr/0007-key-moments.md)). Ne pas suivre les cooldowns des adversaires (politique Riot). Ne pas contourner l'anti-cheat ni lire la mémoire ou les fichiers de replay du jeu.

## Environnement d'exécution (Windows, PowerShell)

- PowerShell sans `&&`, `||` ni here-doc ; enchaîner avec `;` et tester `$?` ou `$LASTEXITCODE`.
- Créer et modifier les fichiers avec les outils d'édition, pas avec `Set-Content` ni des redirections.
- **Ne pas lancer une commande dans le même bloc parallèle qu'une édition dont elle dépend** : elle peut s'exécuter avant que l'édition soit écrite et donner un résultat périmé. Relancer la commande séparément.
- Le dépôt est en CRLF localement ; les avertissements « CRLF will be replaced by LF » de git sont normaux.
