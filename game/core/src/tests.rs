use std::sync::atomic::{AtomicBool, Ordering};
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
use game_domain::{
    CatalogEntry, GameError, Player, PlayerEvent, TakenReason, TreasuryEvent, TreasuryId,
};
use game_spi::{EventStore, PlayerView, Projection, Recorded, TreasuryView};
use uuid::Uuid;

use crate::memory::{
    InMemoryEventStore, InMemoryHouseRegistry, InMemoryPlayerProjection,
    InMemoryTreasuryProjection, StaticCatalog,
};
use crate::{CaughtUpView, GameService, LedgerService, Projector};

type Agora = AuctionService<
    InMemoryAuctionRepository,
    InMemoryInventory,
    InMemoryWallet,
    RecordingMoneySink,
    ManualClock,
>;
type Game = GameService<
    InMemoryPlayerProjection,
    StaticCatalog,
    InMemoryHouseRegistry,
    InMemoryTreasuryProjection,
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
        InMemoryPlayerProjection::default(),
        catalog,
        InMemoryHouseRegistry::default(),
        InMemoryTreasuryProjection::default(),
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

fn op(n: u128) -> OpId {
    OpId(Uuid::from_u128(n))
}

type PlayerEvents = InMemoryEventStore<PlayerRef, PlayerEvent>;
type TreasuryEvents = InMemoryEventStore<TreasuryId, TreasuryEvent>;

/// The ledger with Alice opened (100 copper, 5 cloths), and projectors feeding fresh read
/// models from its event stores.
struct Ledger {
    ledger: LedgerService<PlayerEvents, TreasuryEvents>,
    players: InMemoryPlayerProjection,
    player_projector: Projector<PlayerEvents, InMemoryPlayerProjection, PlayerRef, PlayerEvent>,
    treasury: InMemoryTreasuryProjection,
    treasury_projector:
        Projector<TreasuryEvents, InMemoryTreasuryProjection, TreasuryId, TreasuryEvent>,
}

impl Ledger {
    async fn new() -> Self {
        let player_events = PlayerEvents::default();
        let treasury_events = TreasuryEvents::default();
        let ledger = LedgerService::new(player_events.clone(), treasury_events.clone());
        ledger
            .open_player(Player::new(alice(), Money(100)).with_item(cloth(), 5))
            .await
            .unwrap();
        let players = InMemoryPlayerProjection::default();
        let treasury = InMemoryTreasuryProjection::default();
        Self {
            ledger,
            player_projector: Projector::new(player_events, players.clone()),
            players,
            treasury_projector: Projector::new(treasury_events, treasury.clone()),
            treasury,
        }
    }

    /// Alice as seen by the read model once it caught up with the ledger.
    async fn alice(&self) -> Player {
        self.player_projector.catch_up().await.unwrap();
        self.players.get(&alice()).unwrap()
    }
}

#[tokio::test]
async fn ledger_moves_gold_and_items() {
    let fixture = Ledger::new().await;
    let ledger = &fixture.ledger;
    ledger.debit(op(1), &alice(), Money(30)).await.unwrap();
    ledger.credit(op(2), &alice(), Money(5)).await.unwrap();
    ledger
        .withdraw_item(op(3), &alice(), &cloth(), 5)
        .await
        .unwrap();
    ledger
        .deposit_item(op(4), &alice(), &cloth(), 2)
        .await
        .unwrap();
    // The projector has not run yet.
    assert_eq!(fixture.players.list().await.unwrap(), vec![]);

    let alice = fixture.alice().await;
    assert_eq!(alice.gold, Money(75));
    assert_eq!(alice.quantity(&cloth()), 2);
}

#[tokio::test]
async fn replayed_op_is_applied_once() {
    let fixture = Ledger::new().await;
    for _ in 0..2 {
        fixture
            .ledger
            .debit(op(1), &alice(), Money(30))
            .await
            .unwrap();
    }
    assert_eq!(fixture.alice().await.gold, Money(70));
}

#[tokio::test]
async fn failed_ledger_call_leaves_player_unchanged() {
    let fixture = Ledger::new().await;
    let err = fixture
        .ledger
        .debit(op(1), &alice(), Money(101))
        .await
        .unwrap_err();
    assert_eq!(err, GameError::InsufficientFunds);
    assert_eq!(fixture.alice().await.gold, Money(100));

    let err = fixture
        .ledger
        .debit(op(2), &"nobody".into(), Money(1))
        .await
        .unwrap_err();
    assert_eq!(err, GameError::UnknownPlayer("nobody".into()));
}

#[tokio::test]
async fn opening_a_player_twice_fails() {
    let fixture = Ledger::new().await;
    let err = fixture
        .ledger
        .open_player(Player::new(alice(), Money(0)))
        .await
        .unwrap_err();
    assert_eq!(err, GameError::PlayerExists(alice()));
}

#[tokio::test]
async fn taken_money_goes_to_the_treasury_once() {
    let fixture = Ledger::new().await;
    let listing = ListingId(Default::default());
    let commission = TakenReason::Commission { listing };
    for _ in 0..2 {
        fixture
            .ledger
            .take_money(op(1), Money(7), commission)
            .await
            .unwrap();
    }
    fixture
        .ledger
        .take_money(op(2), Money(3), TakenReason::ForfeitedDeposit { listing })
        .await
        .unwrap();
    fixture.treasury_projector.catch_up().await.unwrap();
    assert_eq!(fixture.treasury.total().await.unwrap(), Money(10));
}

#[tokio::test]
async fn projection_rebuilds_from_the_events() {
    let player_events = PlayerEvents::default();
    let ledger = LedgerService::new(player_events.clone(), TreasuryEvents::default());
    ledger
        .open_player(Player::new(alice(), Money(100)))
        .await
        .unwrap();
    for n in 1..=300 {
        ledger.credit(op(n), &alice(), Money(1)).await.unwrap();
    }

    let players = InMemoryPlayerProjection::default();
    let projector = Projector::new(player_events, players.clone());
    assert_eq!(projector.catch_up().await.unwrap(), 301);
    assert_eq!(projector.catch_up().await.unwrap(), 0);
    assert_eq!(players.get(&alice()).unwrap().gold, Money(400));
}

#[tokio::test]
async fn projection_skips_replayed_events() {
    let player_events = PlayerEvents::default();
    let ledger = LedgerService::new(player_events.clone(), TreasuryEvents::default());
    ledger
        .open_player(Player::new(alice(), Money(100)))
        .await
        .unwrap();
    ledger.credit(op(1), &alice(), Money(5)).await.unwrap();

    let players = InMemoryPlayerProjection::default();
    let batch = player_events.read_all(0, 10).await.unwrap();
    players.apply(&batch).await.unwrap();
    players.apply(&batch).await.unwrap();
    assert_eq!(players.get(&alice()).unwrap().gold, Money(105));
}

/// Fails the first append with a conflict, as if another write happened in between.
struct ConflictOnce<S> {
    inner: S,
    conflicted: AtomicBool,
}

impl<S: EventStore<PlayerRef, PlayerEvent>> EventStore<PlayerRef, PlayerEvent> for ConflictOnce<S> {
    async fn load(&self, stream: &PlayerRef) -> Result<Vec<PlayerEvent>, GameError> {
        self.inner.load(stream).await
    }

    async fn append(
        &self,
        stream: &PlayerRef,
        expected: u64,
        events: Vec<PlayerEvent>,
    ) -> Result<u64, GameError> {
        if !self.conflicted.swap(true, Ordering::SeqCst) {
            return Err(GameError::Conflict);
        }
        self.inner.append(stream, expected, events).await
    }

    async fn read_all(
        &self,
        after: u64,
        limit: usize,
    ) -> Result<Vec<Recorded<PlayerRef, PlayerEvent>>, GameError> {
        self.inner.read_all(after, limit).await
    }
}

#[tokio::test]
async fn ledger_retries_on_conflict() {
    let player_events = PlayerEvents::default();
    let store = ConflictOnce {
        inner: player_events.clone(),
        conflicted: AtomicBool::new(false),
    };
    let ledger = LedgerService::new(store, TreasuryEvents::default());
    ledger
        .open_player(Player::new(alice(), Money(100)))
        .await
        .unwrap();
    assert_eq!(player_events.load(&alice()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn caught_up_views_show_every_recorded_event() {
    let player_events = PlayerEvents::default();
    let treasury_events = TreasuryEvents::default();
    let ledger = LedgerService::new(player_events.clone(), treasury_events.clone());
    let players = CaughtUpView::new(player_events, InMemoryPlayerProjection::default());
    let treasury = CaughtUpView::new(treasury_events, InMemoryTreasuryProjection::default());

    ledger
        .open_player(Player::new(alice(), Money(100)))
        .await
        .unwrap();
    ledger.debit(op(1), &alice(), Money(30)).await.unwrap();
    let listing = ListingId(Default::default());
    ledger
        .take_money(op(2), Money(7), TakenReason::Commission { listing })
        .await
        .unwrap();

    let listed = players.list().await.unwrap();
    assert_eq!(listed, vec![Player::new(alice(), Money(70))]);
    assert_eq!(treasury.total().await.unwrap(), Money(7));
}
