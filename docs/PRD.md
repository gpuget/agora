# Agora : PRD du core d'enchères

Statut : prototype. Date : 2026-10-14.

## 1. Objectif

Agora est un hôtel des ventes inspiré de celui de World of Warcraft. Le core d'enchères est indépendant de tout jeu : il définit des ports, et le jeu fournit les implémentations (clean architecture / hexagonal).

Le jeu est un client de confiance. Les joueurs n'appellent jamais Agora directement : ils passent par le serveur de jeu.

## 2. Hors périmètre (pour l'instant)

- Événements (`EventPublisher` : vente conclue, enchère dépassée, etc.).
- Résistance aux pannes : outbox transactionnelle et idempotence réelle des ports (voir section 8).
- Repository persistant (Postgres).
- Historique de prix, filtres sur `attributes`, recherche texte ou par catégorie.
- Multi-jeux (multi-tenant).
- Achat partiel d'une pile.

## 3. Architecture

| Crate | Rôle | Dépend de |
|---|---|---|
| `agora-domain` | Types et règles pures : `Money`, `Bps`, `ItemStack`, `Listing`, `HouseConfig`, `SearchQuery`, erreurs | - |
| `agora-api` | Ports entrants : trait `AuctionApi` (cas d'usage) et commandes | `agora-domain` |
| `agora-spi` | Ports sortants : `Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock` | `agora-domain` |
| `agora-core` | `AuctionService` : implémente `AuctionApi` à partir des ports SPI. Feature `memory` : adaptateurs en mémoire (repository, faux ports, horloge manuelle) | `agora-domain`, `agora-api`, `agora-spi` |
| `game-domain` | Domaine du jeu : `Player` (or, objets), `CatalogEntry`, `GameError` | `agora-domain` |
| `game-api` | Ports entrants du jeu : `GameApi` (cas d'usage joueur) et `LedgerApi` (mouvements d'or et d'objets demandés par Agora) | `game-domain`, `agora-api` |
| `game-spi` | Ports sortants du jeu : `PlayerRepository`, `Catalog`, `HouseRegistry`, `Treasury`, et `AuctionApi` d'Agora (système externe pour le jeu) | `game-domain`, `agora-api` |
| `game-core` | `GameService` et `LedgerService`. Feature `memory` : adaptateurs en mémoire | `game-api`, `game-spi` |
| `game-server` | Binaire : adaptateurs (HTTP axum, pont qui implémente les ports SPI d'Agora via `LedgerApi`, horloge système), câblage, déclenchement du règlement | tout |
| `game-dto` | Requêtes et réponses HTTP entre `game-server` et `game-ui` | `agora-domain` |
| `game-ui` | Interface web (Yew) branchée sur le HTTP de `game-server` | `agora-domain`, `game-dto` |

Les ports sont asynchrones (`async fn` dans les traits, futures `Send`). `AuctionService` est générique sur ses ports : pas de `dyn`.

## 4. Modèle

- **Monnaie** : `Money(u64)` dans la plus petite unité (cuivre). L'affichage (po/pa/pc) relève du jeu.
- **Taux** : `Bps(u32)`, points de base (`500` = 5 %). Tous les calculs de taux arrondissent à l'unité inférieure.
- **Objet** : `ItemStack { item_type, quantity, base_value, attributes }`.
  - `item_type` : id opaque défini par le jeu.
  - `base_value` : valeur unitaire fournie par le jeu, sert au calcul du dépôt. Le core fait confiance à l'appelant : un adaptateur exposé à des clients joueurs doit la renseigner côté serveur, jamais depuis la requête.
  - `attributes` : JSON opaque, stocké et renvoyé tel quel.
- **Maison** (`AuctionHouse`) : un jeu crée autant de maisons qu'il veut (par faction, neutre...). Les marchés sont isolés. Configuration :
  - `commission` (bps), `deposit` (bps) ;
  - incrément minimum : `min_increment` (montant absolu, au moins 1) et `increment` (bps) ;
  - `durations` : durées autorisées, chacune avec un `deposit_multiplier`.
- **Vente** (`Listing`) : une pile vendue en bloc. Prix toujours exprimés **par unité** ; le total vaut `prix_unitaire * quantité`. La vente fige à sa création la commission, l'incrément et le dépôt.
- **Statuts** : `Active`, `Sold { buyer, unit_price }`, `Expired` (sans enchère), `Cancelled`.
- **Joueur** : `PlayerRef(String)` opaque, fourni par le jeu.

## 5. Cas d'usage (`AuctionApi`)

| Cas d'usage | Effet |
|---|---|
| `create_auction_house(config)` | Valide la config, crée la maison. |
| `create_listing(cmd)` | Retire la pile de l'inventaire du vendeur (séquestre), débite le dépôt, crée la vente. |
| `cancel_listing(seller, id)` | Rend la pile au vendeur. Le dépôt est perdu (`MoneySink`). |
| `place_bid(bidder, id, unit_price)` | Débite l'enchérisseur, rembourse le précédent meilleur enchérisseur. Une enchère supérieure ou égale à l'achat immédiat devient un achat immédiat. |
| `buyout(buyer, id)` | Achat immédiat au prix `buyout_unit_price`. |
| `settle_expired()` | Règle les ventes échues (lot de 100). À appeler périodiquement par l'extérieur. |
| `search(query)` | Ventes actives d'une maison, filtrées et paginées. |

## 6. Règles métier

1. Le vendeur ne peut ni enchérir sur sa vente ni l'acheter.
2. Le meilleur enchérisseur ne peut pas surenchérir sur lui-même (il peut en revanche acheter immédiatement).
3. Une enchère supérieure ou égale à l'achat immédiat est traitée comme un achat immédiat, au prix d'achat immédiat.
4. L'achat immédiat est possible même avec une enchère en cours ; le meilleur enchérisseur est remboursé.
5. La première enchère doit être supérieure ou égale à la mise à prix. Les suivantes doivent atteindre `enchère_courante + max(min_increment, enchère_courante * increment / 10 000)`.
6. Expiration sans enchère : la pile revient au vendeur, le dépôt est perdu.
7. Expiration avec enchère : la pile va au meilleur enchérisseur ; le vendeur reçoit `total - commission + dépôt`.
8. Achat immédiat : le vendeur reçoit `total - commission + dépôt`.
9. Annulation possible seulement sans enchère : la pile revient au vendeur, le dépôt est perdu.
10. La commission est calculée sur le total final, au taux figé à la création.
11. Le dépôt est figé à la création : `base_value * quantité * deposit / 10 000 * deposit_multiplier`.
12. Commission et dépôts perdus vont au port `MoneySink` ; le jeu décide de leur sort.
13. Une vente dont `ends_at` est passé n'accepte plus ni enchère, ni achat, ni annulation, même si elle n'est pas encore réglée.
14. Validations à la création : quantité > 0, mise à prix > 0, achat immédiat supérieur ou égal à la mise à prix, durée autorisée par la maison, totaux et dépôt sans dépassement de `u64`.

Recherche : seules les ventes `Active` sont renvoyées. Filtres : `item_type`, `seller`, `high_bidder`. Tri par prix unitaire croissant (achat immédiat s'il existe, sinon l'enchère courante ou la mise à prix), puis par `ends_at`. Pagination `limit` (1 à 100) / `offset`.

## 7. Concurrence

Verrouillage optimiste : chaque vente porte une `version`. `AuctionRepository::update_listing(listing, expected_version)` renvoie `RepoError::Conflict` si la version stockée a changé. Le cas d'usage rejoue alors sa logique (3 tentatives au maximum, puis `AuctionError::Conflict`).

Ordre dans `place_bid` et `buyout` :

1. charger et valider ;
2. débiter l'acheteur (échec = rien n'a bougé) ;
3. sauvegarder avec la version attendue ; en cas de conflit ou d'erreur, recréditer l'acheteur ;
4. rembourser l'ancien meilleur enchérisseur ;
5. (achat) livrer la pile, payer le vendeur, envoyer la commission au `MoneySink`.

## 8. Pannes

Prototype : aucune garantie si un port échoue au milieu d'un cas d'usage (de l'or ou un objet peut être perdu).

Cible : outbox transactionnelle. Les effets externes sont écrits en base dans la même transaction que le changement d'état (`pending_operations`), puis exécutés et rejoués jusqu'à succès. Pour préparer cette cible, tous les appels `Inventory`, `Wallet` et `MoneySink` portent déjà un `OpId`, et le jeu ignore un `OpId` déjà appliqué. Un effet d'un changement d'annonce enregistré (remboursement, livraison, paiement du vendeur, commission, retour de la pile, dépôt perdu) reçoit un `OpId` dérivé de l'annonce, de sa version et de l'effet : le rejouer donne le même `OpId`. Un débit qui ouvre une nouvelle tentative reçoit un `OpId` neuf, et sa compensation un `OpId` dérivé du débit.

## 9. Jeu de démonstration

- Le jeu suit lui aussi une clean architecture : Agora est pour lui un système externe (port sortant `AuctionApi`), et Agora pilote son grand livre (`LedgerApi`) via un adaptateur pont.
- Joueurs, or et inventaires en mémoire, catalogue d'objets avec leur valeur de base. La valeur de base est lue dans le catalogue par `GameService::sell`, jamais reçue du client.
- Deux maisons : « Alliance » (commission 5 %) et « Neutre » (commission 15 %).
- Le règlement tourne toutes les secondes.
- HTTP sur `127.0.0.1:8080`, préfixe `/api`. L'interface `game-ui` choisit le joueur courant dans une liste (pas d'authentification).

## 10. Critères d'acceptation

- Les règles de la section 6 sont couvertes par des tests de `agora-core`, sur les adaptateurs en mémoire et une horloge manuelle.
- Un conflit de version pendant une enchère ne débite pas l'enchérisseur deux fois.
- `agora-domain`, `agora-api`, `agora-spi` et `agora-core` ne dépendent ni de tokio, ni d'axum, ni d'un format de transport.
- Depuis l'interface : mettre en vente, enchérir, acheter, annuler, et voir le règlement à l'expiration.

## 11. Limites connues

- **Pannes** : aucune protection (section 8). Si un port échoue au milieu d'un cas d'usage, de l'or ou un objet peut être perdu ou dupliqué.
- **Idempotence** : le grand livre du jeu ignore un `OpId` déjà appliqué, mais les adaptateurs en mémoire d'`agora-core` ne dédupliquent pas. Sans outbox, Agora ne rejoue encore aucun effet : la déduplication ne servira qu'avec elle.
- **Persistance** : le repository, les joueurs, l'or et les inventaires sont en mémoire. Tout l'état est perdu au redémarrage du serveur `game-server`.
- **Authentification** : aucune. L'interface choisit le joueur dans une liste, et l'API HTTP du jeu accepte n'importe quel `PlayerRef`.
- **Règlement** : `settle_expired` traite au plus 100 ventes par appel ; le surplus est traité aux appels suivants. Le règlement à l'expiration est couvert par les tests de `agora-core`, mais n'a pas été vérifié de bout en bout via l'API HTTP.
- **Attributs** : `attributes` est stocké et renvoyé tel quel, mais ignoré par `InMemoryInventory` et par le jeu de démo, qui identifient un objet seulement par `item_type`.
