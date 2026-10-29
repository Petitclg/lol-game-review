# LoL Game Review

Application de bureau Windows (Tauri) pour revoir les moments forts de ses parties de League of Legends : kills, morts, assists, objectifs et tours, avec les items et les rangs de sorts au moment de l'action.

## Fonctionnement

1. L'application reste active dans la zone de notification (fermer la fenêtre ne la quitte pas).
2. Quand une partie démarre (Live Client Data API), elle enregistre l'écran en arrière-plan : encodeur matériel (NVENC, AMF ou Quick Sync), pas d'audio, aucune fenêtre, priorité réduite.
3. À la fin de la partie, la chronologie et l'état des 10 joueurs sont sauvegardés.
4. Dans l'interface, choisissez une partie récente (client LoL) et générez les clips : découpe sans ré-encodage, silencieuse et rapide.
5. Parcourez les clips par thème ; le panneau latéral montre les items, KDA et sorts de chaque joueur.
6. Au démarrage, l'application se met à jour vers la dernière release GitHub (jamais pendant une partie).

Seules les parties jouées **pendant que l'application tourne** ont une vidéo ([ADR 0001](docs/adr/0001-recording-during-game.md)).

## Limites connues

- Les ultimates ne sont détectés que pour **vous**, par analyse de l'interface enregistrée ([ADR 0003](docs/adr/0003-ultimate-detection.md)) : aucune API Riot n'expose les lancements de sorts. Cela suppose l'interface par défaut en 16:9 sur l'écran principal ; les parties déjà générées les obtiennent en cliquant sur « Régénérer les clips ».
- Les cooldowns ne sont pas affichés (abandonnés pour cette version) : seuls les items, le niveau, le KDA et les sorts d'invocateur de chaque joueur le sont.
- L'or total des joueurs n'est pas fourni par Riot (seul votre or non dépensé l'est, pour les parties enregistrées après cette fonctionnalité) : l'analyse d'un clip affiche la valeur des items des joueurs impliqués. Pour le Flash, seul l'équipement est connu, pas sa disponibilité.
- Il faut lancer l'application avant la partie (pas de démarrage automatique avec Windows pour l'instant).
- La compatibilité de la capture avec Riot Vanguard doit être validée en conditions réelles.

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

## Release et mise à jour

Pousser un tag `vX.Y.Z` égal à la version de `src-tauri/tauri.conf.json` déclenche [release.yml](.github/workflows/release.yml) : build, ffmpeg LGPL embarqué, installeur NSIS signé et `latest.json` publiés sur la release. Le dépôt doit contenir les secrets `TAURI_SIGNING_PRIVATE_KEY` et `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

L'application vérifie la dernière release au démarrage, réessaie en cas d'échec (30 s, 2 min, 10 min) puis re-vérifie toutes les 6 h. Elle n'installe jamais une mise à jour pendant une partie. Le journal est dans `%LOCALAPPDATA%\com.petitclg.lolgamereview\logs\updater.log`.

## Conventions

- Commits : [Conventional Commits](https://www.conventionalcommits.org/).
- Décisions d'architecture : [docs/adr](docs/adr).

## Licence

MIT, voir [LICENSE](LICENSE). Ce projet n'est pas affilié à Riot Games. FFmpeg (LGPL) est embarqué dans les releases, voir [THIRD_PARTY.md](THIRD_PARTY.md).
