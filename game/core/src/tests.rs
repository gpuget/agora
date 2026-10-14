use std::time::Duration;

use agora_core::AuctionService;
use agora_core::memory::{
    InMemoryAuctionRepository, InMemoryInventory, InMemoryWallet, ManualClock, RecordingMoneySink,
};
use agora_domain::{
    Bps, DurationOption, HouseConfig, HouseId, ItemType, ListingId, Money, OpId, PlayerRef,
    SearchQuery, Timestamp,
};
use game_api::{GameApi, LedgerApi, SellCommand};
use game_domain::{CatalogEntry, GameError, Player, TakenReason};
use game_spi::{PlayerRepository, Treasury};

use crate::memory::{
    InMemoryHouseRegistry, InMemoryPlayerRepository, InMemoryTreasury, StaticCatalog,
};
use crate::{GameService, LedgerService};

type Agora = AuctionService<
    InMemoryAuctionRepository,
    InMemoryInventory,
    InMemoryWallet,
    RecordingMoneySink,
    ManualClock,
>;
type Game = GameService<
    InMemoryPlayerRepository,
    StaticCatalog,
    InMemoryHouseRegistry,
    InMemoryTreasury,
    Agora,
>;

const DURATION: Duration = Duration::from_secs(600);

fn alice() -> PlayerRef {
    "alice".into()
}

fn cloth() -> ItemType {
    "linen_cloth".into()
}

fn config() -> HouseConfig {
    HouseConfig {
        name: "Neutre".into(),
        commission: Bps(1_500),
        min_increment: Money(1),
        increment: Bps(500),
        deposit: Bps(1_000),
        durations: vec![DurationOption {
            duration: DURATION,
            deposit_multiplier: 1,
        }],
    }
}

/// The game wired on Agora with fake Agora ports: Alice holds 20 cloths and 1000 copper.
fn game() -> Game {
    let agora = AuctionService::new(
        InMemoryAuctionRepository::default(),
        InMemoryInventory::default(),
        InMemoryWallet::default(),
        RecordingMoneySink::default(),
        ManualClock::new(Timestamp(0)),
    );
    agora.inventory().give(&alice(), &cloth(), 20);
    agora.wallet().set_balance(&alice(), Money(1_000));
    let catalog = StaticCatalog::new(vec![CatalogEntry {
        item_type: cloth(),
        name: "Étoffe de lin".into(),
        base_value: Money(13),
    }]);
    GameService::new(
        InMemoryPlayerRepository::default(),
        catalog,
        InMemoryHouseRegistry::default(),
        InMemoryTreasury::default(),
        agora,
    )
}

fn sell(house: HouseId, item_type: ItemType) -> SellCommand {
    SellCommand {
        player: alice(),
        house,
        item_type,
        quantity: 20,
        start_unit_price: Money(10),
        buyout_unit_price: None,
        duration: DURATION,
    }
}

fn query(house: HouseId) -> SearchQuery {
    SearchQuery {
        house,
        item_type: None,
        seller: None,
        high_bidder: None,
        limit: 10,
        offset: 0,
    }
}

#[tokio::test]
async fn opened_auction_houses_are_registered() {
    let game = game();
    let house = game.open_auction_house(config()).await.unwrap();
    assert_eq!(game.auction_houses().await.unwrap(), vec![house]);
}

#[tokio::test]
async fn sell_takes_base_value_from_catalog() {
    let game = game();
    let house = game.open_auction_house(config()).await.unwrap();
    let id = game.sell(sell(house.id, cloth())).await.unwrap();

    let page = game.search_market(query(house.id)).await.unwrap();
    let listing = &page.items[0];
    assert_eq!(listing.id, id);
    assert_eq!(listing.item.base_value, Money(13));
    // 20 * 13 * 10 %
    assert_eq!(listing.deposit, Money(26));
}

#[tokio::test]
async fn sell_rejects_unknown_item_type() {
    let game = game();
    let house = game.open_auction_house(config()).await.unwrap();
    let err = game
        .sell(sell(house.id, "dragon_egg".into()))
        .await
        .unwrap_err();
    assert_eq!(err, GameError::UnknownItemType("dragon_egg".into()));
}

#[tokio::test]
async fn auction_errors_are_passed_through() {
    let game = game();
    let house = game.open_auction_house(config()).await.unwrap();
    game.sell(sell(house.id, cloth())).await.unwrap();
    let missing = ListingId(Default::default());
    let err = game.buyout(alice(), missing).await.unwrap_err();
    assert_eq!(
        err,
        GameError::Auction(agora_domain::AuctionError::ListingNotFound)
    );
}

fn op() -> OpId {
    OpId(Default::default())
}

async fn ledger() -> (
    LedgerService<InMemoryPlayerRepository, InMemoryTreasury>,
    InMemoryPlayerRepository,
    InMemoryTreasury,
) {
    let players = InMemoryPlayerRepository::default();
    players
        .insert(Player::new(alice(), Money(100)).with_item(cloth(), 5))
        .await
        .unwrap();
    let treasury = InMemoryTreasury::default();
    (
        LedgerService::new(players.clone(), treasury.clone()),
        players,
        treasury,
    )
}

#[tokio::test]
async fn ledger_moves_gold_and_items() {
    let (ledger, players, _) = ledger().await;
    ledger.debit(op(), &alice(), Money(30)).await.unwrap();
    ledger.credit(op(), &alice(), Money(5)).await.unwrap();
    ledger
        .withdraw_item(op(), &alice(), &cloth(), 5)
        .await
        .unwrap();
    ledger
        .deposit_item(op(), &alice(), &cloth(), 2)
        .await
        .unwrap();

    let alice = players.get(&alice()).unwrap();
    assert_eq!(alice.gold, Money(75));
    assert_eq!(alice.quantity(&cloth()), 2);
}

#[tokio::test]
async fn failed_ledger_call_leaves_player_unchanged() {
    let (ledger, players, _) = ledger().await;
    let err = ledger.debit(op(), &alice(), Money(101)).await.unwrap_err();
    assert_eq!(err, GameError::InsufficientFunds);
    assert_eq!(players.get(&alice()).unwrap().gold, Money(100));

    let err = ledger
        .debit(op(), &"nobody".into(), Money(1))
        .await
        .unwrap_err();
    assert_eq!(err, GameError::UnknownPlayer("nobody".into()));
}

#[tokio::test]
async fn taken_money_goes_to_the_treasury() {
    let (ledger, _, treasury) = ledger().await;
    let listing = ListingId(Default::default());
    ledger
        .take_money(op(), Money(7), TakenReason::Commission { listing })
        .await
        .unwrap();
    ledger
        .take_money(op(), Money(3), TakenReason::ForfeitedDeposit { listing })
        .await
        .unwrap();
    assert_eq!(treasury.total().await.unwrap(), Money(10));
}
