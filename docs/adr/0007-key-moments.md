# ADR 0007 — Moments clés : aces et écarts d'or estimés

- Statut : accepté
- Date : 2026-11-01

## Contexte

En plus des moments où le joueur intervient (kills, morts, assists, objectifs, tours), on veut retrouver les **moments décisifs de la partie**, même quand le joueur n'y est pas : un ace, et une « variation importante de l'or d'équipe ».

## Ce que Riot fournit

- **Ace** : l'événement `Ace` de la Live Client API donne l'équipe (`AcingTeam`) et l'auteur (`Acer`).
- **Or des équipes** : **non disponible**. L'API ne donne que l'or non dépensé du joueur local. La valeur des items achetés ne convient pas non plus : elle ne change qu'au retour à la base, pas pendant le combat qui a rapporté l'or.

## Décision

Thème `key_moment`, deux sortes de moments (module `lgr-core::moments`, tests unitaires) :

1. **Ace** : un clip par événement `Ace`, nommé « Ace de votre équipe » ou « Ace adverse ».
2. **Écart d'or estimé** : l'or est **estimé à partir des événements qui rapportent un montant connu à une équipe**.

   | Événement | Équipe qui gagne | Or estimé |
   |---|---|---|
   | Kill de champion | l'équipe opposée à la victime | 300 |
   | Tour détruite | l'équipe opposée à la tour (`Turret_T1_…` = côté bleu, `T2` = côté rouge) | 500 |
   | Inhibiteur détruit | l'équipe opposée | 250 |
   | Baron | l'équipe du tueur | 1 500 |

   Quand, sur **30 s**, le solde entre les deux équipes atteint **1 500**, c'est un moment clé « Écart d'or +1 800 » (en faveur de votre équipe) ou « Écart d'or −1 800 ». Un seul moment par combat (45 s minimum entre deux), et un ace à moins de 30 s le remplace.

   **Ces montants sont des ordres de grandeur, pas des chiffres Riot.** Les primes de série, les assists, les plaques de tour et les dragons ne sont pas comptés. Le seuil vise un combat décisif (par exemple 5 kills en 20 s, ou 3 kills et une tour), pas une valeur exacte. Le libellé indique « écart d'or » mais c'est une estimation.

- Fenêtre du clip : 20 s avant, 8 s après (on montre comment le combat s'est construit).
- Les clips ne dépendent pas de la vidéo analysée : ils sont donc générés pour toutes les parties enregistrées.

## Limites

- Les montants n'ont pas été vérifiés contre les valeurs officielles de la saison : ils sont approximatifs, et un changement de règles les rendrait moins justes (constantes en tête de `moments.rs`).
- Les noms d'événements non reconnus sont ignorés (aucun montant inventé).
- Les noms des joueurs sont reliés à leur équipe via les captures de la partie ; un joueur absent des captures n'a pas d'équipe, son kill est alors rattaché à la victime si elle est connue.
