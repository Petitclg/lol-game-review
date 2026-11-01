# ADR 0008 — Fusion des clips qui se recoupent et annotations des moments

- Statut : accepté
- Date : 2026-11-01

## Contexte

Deux demandes :

1. Un kill suivi d'une mort, avec un ultimate entre les deux, donnait trois clips presque identiques (un par thème). On veut **un seul clip**.
2. On veut **annoter** les moments pour noter ses erreurs (face-check, erreur mécanique, infériorité numérique, catch, mauvais jungle tracking…) et ses bons réflexes.

## Décision : fusion

Après la construction des clips par thème (qui regroupent déjà les moments proches d'un même thème, par exemple un multikill), `lgr-core::session::merge_overlapping` fusionne **tous les clips dont les fenêtres vidéo se touchent ou se chevauchent, quel que soit leur thème**.

- Un clip fusionné couvre l'union des fenêtres, et **ne dépasse pas 60 s** : enchaîner chaque combat d'une longue escarmouche donnerait un clip que personne ne regarde. Au-delà, un nouveau clip commence.
- Il apparaît **dans chacun de ses thèmes** (onglets, comptes) et porte des pastilles (« Mort · Kill · Ultimate ») ; `theme` est le thème principal (mort d'abord, puis kill, assist, ultimate, Flash, objectif, tour, moment clé), `themes` les liste tous.
- Son libellé joint ceux de ses moments (« Kill + Ultimate + Mort », trois au plus puis « +n »). L'état de la partie montré (joueurs, items, or) est celui du **premier** moment ; les rôles des joueurs impliqués sont réunis.
- Identifiants : un clip non fusionné garde `thème-n` ; un clip fusionné s'appelle `seq-n`. Le fichier est coupé une seule fois.
- Les fichiers des clips remplacés (`kill-0.mp4`…) sont supprimés à la génération, et leur état « compressé » oublié.

**Compatibilité.** Les clips coupés par une version sans fusion (leur index n'a pas de `parts`) restent affichés tels qu'ils ont été coupés : `load_clips` les reconstruit sans fusion (`build_clips_unmerged`) pour que les fichiers correspondent encore. Ils sont fusionnés à la prochaine génération (« Régénérer »), qui nécessite la vidéo source. Une partie dont la vidéo a été supprimée garde donc ses clips d'origine.

## Décision : annotations

- Une annotation (étiquettes + note libre) se rattache à un **moment** (`ClipPart`), pas à un clip : un clip fusionné contient plusieurs moments, et l'on annote la mort avec les erreurs de mort et le kill avec les bons réflexes.
- L'identifiant d'un moment est `thème-temps de jeu à une décimale` (`death-160.0`) : il ne dépend ni de la fenêtre, ni de la fusion, ni de la régénération, donc **les annotations survivent** à tout cela, et à la suppression des clips. Limite : si l'analyse vidéo est refaite et déplace un ultimate ou un Flash, le moment change d'identifiant et son annotation n'est plus rattachée (les morts, kills, etc. viennent de la chronologie du jeu et sont stables).
- Stockage : `annotations.json` dans le dossier de la partie, écrit de façon atomique ; les lectures-écritures sont sérialisées par un verrou.
- Le backend ne connaît que des **identifiants** d'étiquettes (`a-z`, `0-9`, `_`, 48 caractères au plus, 24 étiquettes au plus, note de 2 000 caractères au plus) et les valide ; le **catalogue et les libellés** vivent dans l'interface (`src/annotations.ts`), car ce sont des choix de présentation. Un identifiant inconnu d'une version plus récente reste affiché lisiblement.
- Catalogue : pour une **mort**, les erreurs classiques (face-check, erreur mécanique, infériorité numérique, catch, mauvais jungle tracking, ennemi imprévu, absence de vision, greed, cooldowns clés indisponibles, trade perdant, engage sans équipe, mort à l'objectif, gank subi, aurait dû recall), « mort inévitable » et « bonne mort » ; pour les **autres** moments, bon trade, bon outplay, bon roam, kill facile / snowball ; pour **tous**, bon et mauvais teamfight.
- L'interface envoie chaque annotation **complète** du moment (étiquettes et note) ; l'affichage est mis à jour tout de suite et les enregistrements partent **dans l'ordre**, un par un. Un échec est signalé et les annotations sont relues depuis le disque.

## Conséquences

- Hors périmètre pour l'instant : filtrer ou compter les clips par étiquette (par exemple « mes morts par face-check »), exporter les annotations.
- Un identifiant d'étiquette ne doit jamais être modifié (seul le libellé peut l'être) : il est écrit sur le disque.
