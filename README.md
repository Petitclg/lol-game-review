# LoL Game Review

Application de bureau Windows (Tauri) pour revoir les moments forts de ses parties de League of Legends : kills, morts, assists, objectifs, tours, ultimates, Flashs et moments clés (aces, gros écarts d'or), avec les items et les rangs de sorts au moment de l'action.

## Fonctionnement

1. L'application reste active dans la zone de notification (fermer la fenêtre ne la quitte pas).
2. Quand une partie démarre (Live Client Data API), elle enregistre la fenêtre du jeu en arrière-plan : encodeur matériel (NVENC, AMF ou Quick Sync), pas d'audio, aucune fenêtre, priorité réduite.
3. À la fin de la partie, la chronologie et l'état des 10 joueurs sont sauvegardés.
4. Dans l'interface, choisissez une partie récente (client LoL) et générez les clips : découpe sans ré-encodage, silencieuse et rapide.
5. Parcourez les clips par thème ; le panneau latéral montre les items, KDA et sorts de chaque joueur.
6. Au démarrage, l'application se met à jour vers la dernière release GitHub (jamais pendant une partie).

Seules les parties jouées **pendant que l'application tourne** ont une vidéo ([ADR 0001](docs/adr/0001-recording-during-game.md)).

## Limites connues

- Les ultimates et les Flashs ne sont détectés que pour **vous**, par analyse de l'interface enregistrée ([ADR 0003](docs/adr/0003-ultimate-detection.md)) : aucune API Riot n'expose les lancements de sorts. L'échelle de l'interface est calibrée automatiquement ; si elle n'est pas reconnue, l'application l'indique au lieu d'inventer des clips. Le Flash n'est pas détecté pour une partie dont la vidéo a été supprimée.
- Les moments clés ([ADR 0007](docs/adr/0007-key-moments.md)) sont un ace, ou un « écart d'or » **estimé** : Riot ne donne pas l'or des autres joueurs, il est évalué à partir des kills, tours, inhibiteurs et du Baron (montants approximatifs). Ces clips ne concernent pas forcément votre personnage. L'équipe et l'auteur d'un ace ne sont connus que pour les parties enregistrées avec cette version.
- Stockage ([ADR 0004](docs/adr/0004-capture-and-storage-budget.md)) : environ 1 Go par partie enregistrée. Le panneau « Stockage » permet de supprimer la vidéo après les clips, de compresser les clips (environ −45 %, en tâche de fond hors partie), de fixer un plafond (20 Go par défaut, la plus ancienne partie est supprimée d'abord) et de **faire de la place pour la partie en cours** (activé par défaut) : au début d'une partie, si le disque ou le plafond n'a plus la place d'environ 4 Go, les données les plus anciennes sont supprimées pour que la partie soit enregistrée ; la dernière partie enregistrée n'est jamais touchée.
- La fenêtre du jeu est capturée sur n'importe quel écran (repli sur l'écran principal). Non validé sous Riot Vanguard en partie réelle.
- Les cooldowns ne sont pas affichés (abandonnés pour cette version) : seuls les items, le niveau, le KDA et les sorts d'invocateur de chaque joueur le sont.
- L'or total des joueurs n'est pas fourni par Riot (seul votre or non dépensé l'est, pour les parties enregistrées après cette fonctionnalité) : l'analyse d'un clip affiche la valeur des items des joueurs impliqués. Pour le Flash, seul l'équipement est connu, pas sa disponibilité.
- Il faut lancer l'application avant la partie (pas de démarrage automatique avec Windows pour l'instant).

## Architecture

| Élément | Rôle |
|---|---|
| [crates/core](crates/core) | Événements, fenêtres de clips, modèle de session, stockage fichiers |
| [crates/lcu](crates/lcu) | API locale du client LoL : compte actif, parties récentes |
| [crates/live](crates/live) | Live Client Data API, suivi de session pendant la partie |
| [crates/media](crates/media) | ffmpeg : enregistrement silencieux, découpe des clips |
| [crates/engine](crates/engine) | Moniteur de partie, génération des clips, liste des parties |
| [src-tauri](src-tauri) | Coque Tauri : commandes IPC, zone de notification, mise à jour |
| [src](src) | Interface TypeScript (sans framework) |

Spécifications : [produit](docs/specs/product.md), [contrat IPC](docs/specs/ipc.md).

## Développement

Prérequis : Rust stable (MSVC), Node.js 22+, Build Tools Visual Studio (charge de travail C++), ffmpeg (`winget install Gyan.FFmpeg`, ou variable `LGR_FFMPEG`).

```powershell
npm ci
npm run dev            # interface seule, avec backend simulé dans le navigateur
npm run tauri dev      # application complète
npm run typecheck; npm run lint; npm test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`LGR_REQUIRE_FFMPEG=1` fait échouer (au lieu d'ignorer) les tests qui ont besoin de ffmpeg.

### Tester sans toucher à une installation réelle

`LGR_SANDBOX=1` (ou `LGR_SANDBOX=<dossier>`) lance une instance isolée : dossier de données et WebView séparés, aucun enregistrement, aucune mise à jour, pas de verrou d'instance unique. Elle peut donc tourner à côté de l'application installée et pendant une partie sans jamais l'enregistrer.

```powershell
$env:LGR_SANDBOX = "1"; .\target\release\lol-game-review.exe
```

La logique métier est testée en CI sur des données synthétiques ; seul ce qui exige une vraie partie (client LoL, Live API, capture sous l'anti-cheat) ne peut pas l'être hors d'un poste de jeu.

## Release et mise à jour

Pousser un tag `vX.Y.Z` égal à la version de `src-tauri/tauri.conf.json` déclenche [release.yml](.github/workflows/release.yml) : build, ffmpeg LGPL embarqué, installeur NSIS signé et `latest.json` publiés sur la release. Le dépôt doit contenir les secrets `TAURI_SIGNING_PRIVATE_KEY` et `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

L'application vérifie la dernière release au démarrage, réessaie en cas d'échec (30 s, 2 min, 10 min) puis re-vérifie toutes les 6 h. Elle n'installe jamais une mise à jour pendant une partie. Le journal est dans `%LOCALAPPDATA%\com.petitclg.lolgamereview\logs\updater.log`.

### Annonce sur Discord

Une fois la release publiée, le workflow poste les nouveautés dans un salon Discord ([ADR 0006](docs/adr/0006-discord-release-announcement.md)).

1. Discord : *Paramètres du salon → Intégrations → Webhooks → Nouveau webhook*, puis copier l'URL.
2. Dépôt : `gh secret set DISCORD_WEBHOOK_URL` (coller l'URL ; ne jamais la commiter).
3. Les nouveautés viennent des commits `feat` / `fix` / `perf` depuis la version précédente. Pour un texte rédigé à la main, créer `docs/release-notes/vX.Y.Z.md` avant de poser le tag.
4. Essai à blanc : `node scripts/announce-release.mjs vX.Y.Z --dry-run`. Republier : *Actions → Announce release → Run workflow*.

## Conventions

- Commits : [Conventional Commits](https://www.conventionalcommits.org/).
- Décisions d'architecture : [docs/adr](docs/adr).
- Règles de travail pour les agents Copilot (périmètre, tests, publications, données) : [.github/copilot-instructions.md](.github/copilot-instructions.md).

## Licence

MIT, voir [LICENSE](LICENSE). Ce projet n'est pas affilié à Riot Games. FFmpeg (LGPL) est embarqué dans les releases, voir [THIRD_PARTY.md](THIRD_PARTY.md).
