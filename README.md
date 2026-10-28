# LoL Game Review

Application de bureau Windows (Tauri) pour revoir les moments forts de ses parties de League of Legends.

## État

Projet en démarrage. Livré à ce stade :

- `crates/core` (`lgr-core`) : logique métier pure et testée
  - classification des événements de la Live Client Data API (kill, mort, assist, objectifs, tours) ;
  - fenêtres de clips (pré/post-roll, fusion des chevauchements) ;
  - calcul des cooldowns (ability haste).
- `crates/lcu` (`lgr-lcu`) : client de l'API locale du client LoL
  - découverte et parsing du `lockfile` (mot de passe jamais journalisé) ;
  - compte actif et parties récentes, testés contre un serveur HTTP local factice.

Le reste est décrit dans [docs/specs/product.md](docs/specs/product.md) et suivi dans le plan de développement.

## Principes

- Silencieux : aucune fenêtre, aucun son, aucun vol de focus.
- Léger : enregistrement par encodeur matériel, processus à priorité basse.
- Pas d'interférence avec une partie en cours.
- Respect de la politique développeur de Riot Games : aucune automatisation de l'interface du jeu, aucune donnée cachée.

## Développement

Prérequis : Rust stable (MSVC), Node.js 22+, Build Tools Visual Studio (charge de travail C++).

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Conventions

- Commits : [Conventional Commits](https://www.conventionalcommits.org/).
- Toute décision d'architecture structurante est consignée dans [docs/adr](docs/adr).

## Licence

MIT, voir [LICENSE](LICENSE). Ce projet n'est pas affilié à Riot Games.
