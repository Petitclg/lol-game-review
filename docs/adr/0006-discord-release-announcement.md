# ADR 0006 — Annonce des releases sur Discord

- Statut : accepté
- Date : 2026-11-01

## Contexte

À chaque release, les nouveautés doivent être publiées dans un salon textuel Discord.

## Décision

**Un webhook Discord appelé par le workflow de release**, plutôt qu'un bot hébergé.

- Un bot (jeton, processus permanent, hébergement, supervision) n'apporte rien ici : on ne lit rien dans Discord, on y écrit un message par version. Un webhook publie dans le salon sous le nom « LoL Game Review », sans serveur à maintenir ni jeton à garder.
- `Release` publie l'installeur, puis appelle `announce.yml` (job `announce`, qui ne démarre que si la release a réussi). Les événements créés avec `GITHUB_TOKEN` ne déclenchent pas d'autres workflows : un déclencheur `release: published` ne fonctionnerait pas, d'où l'appel direct.
- `announce.yml` se lance aussi à la main (`workflow_dispatch`, paramètre `tag`) pour republier une annonce.
- L'URL du webhook est le secret `DISCORD_WEBHOOK_URL`. Sans lui, l'annonce est ignorée (un message dans le journal) et la release n'échoue pas.

## Contenu du message

1. S'il existe, `docs/release-notes/<tag>.md` (texte rédigé à la main en Markdown Discord), lu dans l'arbre du tag.
2. Sinon, les commits entre le tag précédent (le plus grand tag `vX.Y.Z` inférieur, comparé par version) et le tag annoncé, d'après les Conventional Commits : `feat` → « Nouveautés », `fix` → « Corrections », `perf` → « Performances ». Les autres types (`chore`, `docs`, `test`, `ci`, `refactor`…) sont omis. Sans changement visible : « Mises à jour internes et corrections mineures. »

Le message est un embed (titre, lien vers la release, date). Au-delà de 4096 caractères, il est coupé à une ligne entière avec « … et N autres ».

## Sécurité

- `allowed_mentions: {parse: []}` : un message de commit ne peut jamais notifier `@everyone` ni un rôle.
- L'URL doit avoir la forme `https://discord.com/api/webhooks/<id>/<jeton>` (ou `discordapp.com`, `ptb.`, `canary.`) ; sinon le script refuse de l'appeler. Elle n'apparaît jamais dans un message d'erreur.
- Le tag est transmis au script par variable d'environnement, jamais interpolé dans la commande du workflow.
- Droits du workflow : `contents: read`.

## Fiabilité

Réessais sur limitation de débit (en respectant `retry_after`) et sur erreur serveur, 4 tentatives. Les autres refus (webhook supprimé, 401, 404) ne sont pas réessayés. Un échec fait échouer le job `announce` (visible), sans toucher à la release déjà publiée.

## Tests

`scripts/release-notes.test.mjs` (vitest, exécuté par la CI) : lecture des commits, regroupement, tag précédent, limites Discord, mentions, validation de l'URL, réessais, secret absent. Essai à blanc : `node scripts/announce-release.mjs vX.Y.Z --dry-run`.
