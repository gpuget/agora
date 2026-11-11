# Agora : PRD du core d'enchères

Statut : prototype. Date : 2026-11-04.

## 1. Objectif

Agora est un hôtel des ventes. Le core d'enchères est indépendant de tout jeu : il définit des ports, et le jeu fournit les implémentations (clean architecture / hexagonal).

Le jeu est un client de confiance. Les joueurs n'appellent jamais Agora directement : ils passent par le serveur de jeu.

Deux jeux de démonstration s'appuient sur Agora et sur un grand livre événementiel commun : `game` (or et piles d'objets) et `tcg` (jeu de cartes : poussière et cartes).

## 2. Hors périmètre (pour l'instant)

- Événements d'Agora (`EventPublisher` : vente conclue, enchère dépassée, etc.).
- Résistance aux pannes : outbox transactionnelle et rejeu des effets par Agora (voir section 9).
- Stockage persistant (Postgres) du repository d'Agora et des event stores.
- Historique de prix, filtres sur `attributes`, recherche texte ou par catégorie.
- Multi-jeux (multi-tenant) dans une même instance d'Agora.
- Authentification des joueurs.

## 3. Architecture

| Crate | Rôle | Dépend de |
|---|---|---|
| `agora-domain` | Types et règles pures : `Money`, `Bps`, `ItemStack`, `Listing`, `HouseConfig`, `SearchQuery`, erreurs | - |
| `agora-api` | Ports entrants : trait `AuctionApi` (cas d'usage) et commandes | `agora-domain` |
| `agora-spi` | Ports sortants : `Inventory`, `Wallet`, `MoneySink`, `AuctionRepository`, `Clock` | `agora-domain` |
| `agora-core` | `AuctionService` : implémente `AuctionApi` à partir des ports SPI. Feature `memory` : adaptateurs en mémoire (repository, faux ports, horloge manuelle) | `agora-domain`, `agora-api`, `agora-spi` |
| `event-sourcing` | Event sourcing indépendant de tout domaine : `Decider`, ports `EventStore` et `Projection`, `execute`, `Projector`. Feature `memory` : event store en mémoire | - |
| `ledger` | Grand livre événementiel partagé par les jeux : `Account` (fonds et objets d'un joueur), deciders des comptes et du trésor, `LedgerApi`, `LedgerService`, et `AgoraBridge` qui implémente les ports SPI d'Agora via `LedgerApi` | `agora-domain`, `agora-spi`, `event-sourcing` |
| `game-domain` | Domaine du jeu : `Player` (un `Account` dont les fonds sont l'or), `CatalogEntry`, `GameError` | `agora-domain`, `ledger` |
| `game-api` | Ports entrants du jeu : `GameApi` (cas d'usage joueur et hôte) | `game-domain`, `agora-api` |
| `game-spi` | Ports sortants du jeu : `PlayerView`, `TreasuryView`, `Catalog`, `HouseRegistry`, et `AuctionApi` d'Agora (système externe pour le jeu) | `game-domain`, `agora-api` |
| `game-core` | `GameService`, `CaughtUpView`. Feature `memory` : projections et adaptateurs en mémoire | `game-api`, `game-spi`, `ledger`, `event-sourcing` |
| `game-server` | Binaire : adaptateurs (HTTP axum, horloge système), lecture de `game.toml`, câblage, déclenchement du règlement | tout |
| `game-dto` | Requêtes et réponses HTTP entre `game-server` et `game-ui` | `agora-domain` |
| `game-ui` | Interface web (Yew, Trunk) branchée sur le HTTP de `game-server` | `agora-domain`, `game-dto` |
| `tcg-*` | Jeu de cartes : mêmes crates que `game-*` (`tcg-domain`, `tcg-api`, `tcg-spi`, `tcg-core`, `tcg-server`, `tcg-dto`, `tcg-ui`), au-dessus du même `ledger` | idem `game-*` |

Les ports sont asynchrones (`async fn` dans les traits, futures `Send`). `AuctionService` est générique sur ses ports : pas de `dyn`.

## 4. Modèle

- **Monnaie** : `Money(u64)` dans la plus petite unité (cuivre pour `game`, poussière pour `tcg`). L'affichage relève du jeu.
- **Taux** : `Bps(u32)`, points de base (`500` = 5 %). Tous les calculs de taux arrondissent à l'unité inférieure.
- **Objet** : `ItemStack { item_type, quantity, commodity, base_value, attributes }`.
  - `item_type` : id opaque défini par le jeu.
  - `commodity` : fourni par le jeu. Une commodité (ressource, consommable) ne reçoit pas d'enchère et peut être achetée en partie. Tout autre objet (équipement, objet rare, carte) est mis aux enchères et acheté en entier.
  - `base_value` : valeur unitaire fournie par le jeu, sert au calcul du dépôt. Le core fait confiance à l'appelant : un adaptateur exposé à des clients joueurs doit la renseigner côté serveur, jamais depuis la requête.
  - `attributes` : JSON opaque, stocké et renvoyé tel quel par Agora.
- **Maison** (`AuctionHouse`) : un jeu crée autant de maisons qu'il veut (par faction, neutre...). Les marchés sont isolés. Configuration :
  - `commission` (bps), `deposit` (bps) ;
  - incrément minimum : `min_increment` (montant absolu, au moins 1) et `increment` (bps) ;
  - `durations` : durées autorisées, chacune avec un `deposit_multiplier`.
- **Vente** (`Listing`) : une pile vendue en bloc, ou en partie s'il s'agit d'une commodité. Prix toujours exprimés **par unité** ; le total vaut `prix_unitaire * quantité`. La vente fige à sa création la commission, l'incrément et le dépôt.
- **Statuts** : `Active`, `Sold { buyer, unit_price }`, `Expired` (sans enchère), `Cancelled`.
- **Joueur** : `PlayerRef(String)` opaque, fourni par le jeu.
- **Identifiant d'opération** : `OpId(Uuid)`, porté par chaque appel qui déplace de l'argent ou des objets (section 9).

## 5. Cas d'usage (`AuctionApi`)

| Cas d'usage | Effet |
|---|---|
| `create_auction_house(config)` | Valide la config, crée la maison. |
| `create_listing(cmd)` | Retire la pile de l'inventaire du vendeur (séquestre), débite le dépôt, crée la vente. |
| `cancel_listing(seller, id)` | Rend le restant au vendeur. Le dépôt restant est perdu (`MoneySink`). |
| `place_bid(bidder, id, unit_price)` | Débite l'enchérisseur, rembourse le précédent meilleur enchérisseur. Une enchère supérieure ou égale à l'achat immédiat devient un achat immédiat de toute la pile. |
| `buyout(buyer, id, quantity)` | Achat immédiat de `quantity` objets au prix `buyout_unit_price`. Commodité : `1..=restant`, la vente reste ouverte avec le reste. Autre objet : `quantity` doit être le restant. |
| `settle_expired()` | Règle les ventes échues (lot de 100) et renvoie leur nombre. À appeler périodiquement par l'extérieur. |
| `search(query)` | Ventes actives d'une maison, filtrées et paginées. |

## 6. Règles métier

1. Le vendeur ne peut ni enchérir sur sa vente ni l'acheter.
2. Le meilleur enchérisseur ne peut pas surenchérir sur lui-même (il peut en revanche acheter immédiatement).
3. Une enchère supérieure ou égale à l'achat immédiat est traitée comme un achat immédiat de toute la pile, au prix d'achat immédiat.
4. L'achat immédiat est possible même avec une enchère en cours ; le meilleur enchérisseur est remboursé.
5. La première enchère doit être supérieure ou égale à la mise à prix. Les suivantes doivent atteindre `enchère_courante + max(min_increment, enchère_courante * increment / 10 000)`.
6. Expiration sans enchère : la pile revient au vendeur, le dépôt est perdu.
7. Expiration avec enchère : la pile va au meilleur enchérisseur ; le vendeur reçoit `total - commission + dépôt`.
8. Achat immédiat : le vendeur reçoit `prix unitaire * quantité achetée - commission + part du dépôt`. La part du dépôt est proportionnelle à la quantité achetée (`dépôt * quantité / restant`, arrondi inférieur) et tout le dépôt restant revient à l'achat du dernier lot. La vente garde le dépôt qui couvre le restant.
9. Annulation possible seulement sans enchère : le restant revient au vendeur, le dépôt restant est perdu.
10. La commission est calculée sur le total final, au taux figé à la création. Elle est calculée à chaque achat partiel sur le total de ce lot.
11. Le dépôt est figé à la création : `base_value * quantité * deposit / 10 000 * deposit_multiplier`.
12. Commission et dépôts perdus vont au port `MoneySink` ; le jeu décide de leur sort.
13. Une vente dont `ends_at` est atteint n'accepte plus ni enchère, ni achat, ni annulation, même si elle n'est pas encore réglée.
14. Une commodité n'accepte aucune enchère (`BidsNotAllowed`) et exige un prix d'achat immédiat à la création (`CommodityNeedsBuyout`). Un autre objet refuse l'achat partiel (`PartialNotAllowed`).
15. Validations à la création : quantité > 0, mise à prix > 0, achat immédiat supérieur ou égal à la mise à prix, durée autorisée par la maison, totaux et dépôt sans dépassement de `u64`.

Recherche : seules les ventes `Active` sont renvoyées. Filtres : `item_type`, `seller`, `high_bidder`. Tri par prix unitaire croissant (achat immédiat s'il existe, sinon l'enchère courante ou la mise à prix), puis par `ends_at`. Pagination `limit` (1 à 100) / `offset`.

## 7. Concurrence

Verrouillage optimiste : chaque vente porte une `version`. `AuctionRepository::update_listing(listing, expected_version)` renvoie `RepoError::Conflict` si la version stockée a changé. Le cas d'usage rejoue alors sa logique (3 tentatives au maximum, puis `AuctionError::Conflict`). Le règlement ignore une vente en conflit : elle réapparaît au passage suivant si elle est toujours échue.

Ordre dans `place_bid` et `buyout` :

1. charger et valider ;
2. débiter l'acheteur (échec = rien n'a bougé) ;
3. sauvegarder avec la version attendue ; en cas de conflit ou d'erreur, recréditer l'acheteur ;
4. rembourser l'ancien meilleur enchérisseur ;
5. (achat) livrer la pile, payer le vendeur, envoyer la commission au `MoneySink`.

Le grand livre applique le même principe par flux : `event_sourcing::execute` charge les événements d'un flux, décide, puis ajoute avec le nombre d'événements attendu (`StoreError::Conflict` sinon). 3 tentatives au maximum.

## 8. Event sourcing et grand livre

`event-sourcing` ne connaît aucun domaine :

- `Decider` : `decide(id, command, state)` renvoie les événements à enregistrer (liste vide si la commande est déjà appliquée), `evolve(state, event)` applique un événement et n'échoue jamais.
- `EventStore<I, E>` : `load` d'un flux, `append` avec version attendue, `read_all` du journal global (positions à partir de 1).
- `Projection<I, E>` : modèle de lecture avec un point de reprise (`checkpoint`) ; `apply` ignore les événements déjà vus, donc un lot rejoué est sans effet.
- `Projector` : `catch_up` applique les événements enregistrés depuis le point de reprise, par lots de 256.

`ledger` s'appuie dessus :

- Un flux par joueur (`AccountDecider`, événements `AccountEvent`) et un flux unique pour le trésor (`TreasuryDecider`, `TreasuryId`).
- `LedgerApi` : `open_account`, `withdraw_item`, `deposit_item`, `debit`, `credit`, `take_money`. Chaque mouvement porte l'`OpId` de l'appel Agora ; l'état de chaque flux garde l'ensemble des `OpId` appliqués, et un appel rejoué n'enregistre rien.
- `AgoraBridge` implémente `Inventory`, `Wallet` et `MoneySink` d'Agora avec une `LedgerApi`. Les erreurs du grand livre deviennent des `PortError`.
- Le trésor cumule ce qu'Agora retire du jeu (commissions et dépôts perdus, `TakenReason`).

Côté jeu, les joueurs et le total du trésor sont lus dans des projections du grand livre. `CaughtUpView` rattrape le journal à chaque lecture : une lecture voit tous les événements enregistrés avant l'appel.

## 9. Pannes

Prototype : aucune garantie si un port échoue au milieu d'un cas d'usage (de l'or ou un objet peut être perdu).

Cible : outbox transactionnelle. Les effets externes sont écrits en base dans la même transaction que le changement d'état (`pending_operations`), puis exécutés et rejoués jusqu'à succès. Pour préparer cette cible, tous les appels `Inventory`, `Wallet` et `MoneySink` portent déjà un `OpId`, et le grand livre ignore un `OpId` déjà appliqué. Un effet d'un changement d'annonce enregistré (remboursement, livraison, paiement du vendeur, commission, retour de la pile, dépôt perdu) reçoit un `OpId` dérivé de l'annonce, de sa version et de l'effet (UUID v5) : le rejouer donne le même `OpId`. Un débit qui ouvre une nouvelle tentative reçoit un `OpId` neuf (UUID v7), et sa compensation un `OpId` dérivé du débit.

## 10. Jeux de démonstration

Les deux jeux suivent la même clean architecture : Agora est pour eux un système externe (port sortant `AuctionApi`), et Agora pilote leur grand livre (crate `ledger`, `LedgerApi`) via `AgoraBridge`. Joueurs, or, inventaires, repository d'Agora et event stores sont en mémoire. Le règlement tourne toutes les secondes par défaut (`settle_every_secs`). L'interface choisit le joueur courant dans une liste (pas d'authentification) et se rafraîchit toutes les secondes.

Configuration par fichier TOML dans le répertoire courant (`GAME_CONFIG` ou `TCG_CONFIG` pour un autre chemin) : adresse d'écoute, délai de règlement, niveaux de log par cible (`[log]`), maisons (`[[house]]`), catalogue. Le catalogue n'a pas de valeur par défaut : sans lui le serveur refuse de démarrer. `BIND_ADDR` surcharge l'adresse, `RUST_LOG` la table de logs.

### 10.1 `game`

- Joueurs avec de l'or et des piles d'objets. La valeur de base est lue dans le catalogue par `GameService::sell`, jamais reçue du client.
- Le catalogue marque chaque entrée `commodity` ou non. Une commodité se vend à prix fixe et s'achète en partie ; un autre objet est mis aux enchères et acheté en entier.
- Deux maisons par défaut : « Alliance » (commission 5 %) et « Neutre » (commission 15 %). Montants en cuivre (1 or = 100 argent = 10 000 cuivre).
- Trois joueurs de départ (`alice`, `bob`, `carol`) avec 50 à 200 or et 1 à 3 types d'objets tirés au hasard.
- HTTP sur `127.0.0.1:8080`, préfixe `/api` : `GET /catalog`, `/houses`, `/players`, `/sink`, `/houses/{house}/listings` ; `POST /listings`, `/listings/{id}/bid`, `/listings/{id}/buyout`, `/listings/{id}/cancel`. Interface sur le port 3000.

### 10.2 `tcg`

- Un collectionneur détient de la poussière (la monnaie, une unité par poussière) et des exemplaires de cartes, interchangeables.
- Une vente porte toujours une seule carte, achetée en entier : `SellCommand` n'a pas de quantité, les prix sont ceux de la carte, `commodity` vaut toujours `false`. Les cartes sont donc toujours aux enchères.
- La valeur de base d'une carte vient de sa rareté (`common`, `rare`, `epic`, `legendary`), table `rarity` du fichier de config. La table est facultative (par défaut : 40, 100, 400, 1 600) ; si elle est présente, chaque rareté est obligatoire et supérieure à zéro.
- Une maison par défaut : « Marché » (commission 5 %).
- Trois collectionneurs de départ (`alice`, `bob`, `carol`) avec 1 000 à 5 000 de poussière et 3 à 6 cartes différentes tirées au hasard, de 1 à 3 exemplaires chacune.
- HTTP sur `127.0.0.1:8081`, préfixe `/api` : mêmes routes que `game`, sauf `GET /collectors` à la place de `/players`. Interface sur le port 3001.

## 11. Critères d'acceptation

- Les règles de la section 6 sont couvertes par des tests de `agora-core`, sur les adaptateurs en mémoire et une horloge manuelle.
- Un conflit de version pendant une enchère ne débite pas l'enchérisseur deux fois.
- `agora-domain`, `agora-api`, `agora-spi` et `agora-core` ne dépendent ni de tokio, ni d'axum, ni d'un format de transport. `event-sourcing` ne dépend d'aucun domaine ni de tokio hors tests.
- Un `OpId` rejoué sur le grand livre n'a aucun effet, et les projections se reconstruisent depuis les événements (tests de `ledger`, `game-core` et `tcg-core`).
- Depuis l'interface de chaque jeu : mettre en vente, enchérir, acheter, annuler, et voir le règlement à l'expiration. Dans `game`, acheter une partie d'une commodité.

## 12. Limites connues

- **Pannes** : aucune protection (section 9). Si un port échoue au milieu d'un cas d'usage, de l'or ou un objet peut être perdu ou dupliqué.
- **Idempotence** : le grand livre ignore un `OpId` déjà appliqué, mais les adaptateurs en mémoire d'`agora-core` ne dédupliquent pas. Sans outbox, Agora ne rejoue encore aucun effet : la déduplication ne servira qu'avec elle.
- **Grand livre** : l'ensemble des `OpId` appliqués grandit avec chaque flux et n'est jamais purgé. `execute` recharge tout le flux à chaque commande, sans instantané.
- **Persistance** : le repository d'Agora, les event stores, les projections et les maisons du jeu sont en mémoire. Tout l'état est perdu au redémarrage d'un serveur.
- **Authentification** : aucune. L'interface choisit le joueur dans une liste, et l'API HTTP de chaque jeu accepte n'importe quel `PlayerRef`.
- **Règlement** : `settle_expired` traite au plus 100 ventes par appel ; le surplus est traité aux appels suivants. Le règlement à l'expiration est couvert par les tests de `agora-core`, mais n'a pas été vérifié de bout en bout via l'API HTTP.
- **Attributs** : `attributes` est stocké et renvoyé tel quel par Agora, mais le grand livre compte les objets par `item_type` seulement. Les attributs ne survivent donc pas à un retrait ou un dépôt d'objet, et les jeux de démo n'en renseignent aucun.
- **Une carte par vente (`tcg`)** : un joueur qui veut vendre plusieurs copies crée autant de ventes.
