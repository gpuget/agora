use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agora_core::AuctionService;
use agora_core::memory::{
    InMemoryAuctionRepository, InMemoryInventory, InMemoryWallet, ManualClock, RecordingMoneySink,
};
use agora_domain::{
    Bps, DurationOption, HouseConfig, HouseId, ListingId, Money, OpId, PlayerRef, SearchQuery,
    Timestamp,
};
use event_sourcing::memory::InMemoryEventStore;
use event_sourcing::{EventStore, Projection, Projector, Recorded, StoreError};
use tcg_api::{LedgerApi, SellCommand, TcgApi};
use tcg_domain::{
    CardEntry, CardId, Collector, CollectorEvent, Rarity, TakenReason, TcgError, TreasuryEvent,
    TreasuryId,
};
use tcg_spi::{CollectorView, TreasuryView};
use uuid::Uuid;

use crate::memory::{
    InMemoryCollectorProjection, InMemoryHouseRegistry, InMemoryTreasuryProjection, StaticCatalog,
};
use crate::{CaughtUpView, LedgerService, TcgService};

type Agora = AuctionService<
    InMemoryAuctionRepository,
    InMemoryInventory,
    InMemoryWallet,
    RecordingMoneySink,
    ManualClock,
>;
type Tcg = TcgService<
    InMemoryCollectorProjection,
    StaticCatalog,
    InMemoryHouseRegistry,
    InMemoryTreasuryProjection,
    Agora,
>;

const DURATION: Duration = Duration::from_secs(600);

fn alice() -> PlayerRef {
    "alice".into()
}

fn drake() -> CardId {
    "fire_drake".into()
}

fn config() -> HouseConfig {
    HouseConfig {
        name: "Marché".into(),
        commission: Bps(500),
        min_increment: Money(1),
        increment: Bps(500),
        deposit: Bps(1_500),
        durations: vec![DurationOption {
            duration: DURATION,
            deposit_multiplier: 1,
        }],
    }
}

/// The game wired on Agora with fake Agora ports: Alice holds 3 drakes and 1000 dust.
fn tcg() -> Tcg {
    let agora = AuctionService::new(
        InMemoryAuctionRepository::default(),
        InMemoryInventory::default(),
        InMemoryWallet::default(),
        RecordingMoneySink::default(),
        ManualClock::new(Timestamp(0)),
    );
    agora.inventory().give(&alice(), &drake(), 3);
    agora.wallet().set_balance(&alice(), Money(1_000));
    let catalog = StaticCatalog::new(vec![CardEntry {
        card: drake(),
        name: "Drake de feu".into(),
        rarity: Rarity::Legendary,
        base_value: Money(1_600),
    }]);
    TcgService::new(
        InMemoryCollectorProjection::default(),
        catalog,
        InMemoryHouseRegistry::default(),
        InMemoryTreasuryProjection::default(),
        agora,
    )
}

fn sell(house: HouseId, card: CardId) -> SellCommand {
    SellCommand {
        collector: alice(),
        house,
        card,
        start_price: Money(500),
        buyout_price: Some(Money(900)),
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
    let tcg = tcg();
    let house = tcg.open_auction_house(config()).await.unwrap();
    assert_eq!(tcg.auction_houses().await.unwrap(), vec![house]);
}

#[tokio::test]
async fn sell_lists_a_single_card() {
    let tcg = tcg();
    let house = tcg.open_auction_house(config()).await.unwrap();
    let id = tcg.sell(sell(house.id, drake())).await.unwrap();

    let page = tcg.search_market(query(house.id)).await.unwrap();
    let listing = &page.items[0];
    assert_eq!(listing.id, id);
    assert_eq!(listing.item.quantity, 1);
    assert_eq!(listing.item.base_value, Money(1_600));
    assert_eq!(listing.start_unit_price, Money(500));
    assert_eq!(listing.buyout_unit_price, Some(Money(900)));
    // 1 * 1600 * 15 %
    assert_eq!(listing.deposit, Money(240));
}

#[tokio::test]
async fn each_copy_is_its_own_listing() {
    let tcg = tcg();
    let house = tcg.open_auction_house(config()).await.unwrap();
    for _ in 0..3 {
        tcg.sell(sell(house.id, drake())).await.unwrap();
    }
    let err = tcg.sell(sell(house.id, drake())).await.unwrap_err();
    assert!(matches!(err, TcgError::Auction(_)), "{err:?}");

    let page = tcg.search_market(query(house.id)).await.unwrap();
    assert_eq!(page.total, 3);
    assert!(page.items.iter().all(|l| l.item.quantity == 1));
}

#[tokio::test]
async fn sell_rejects_unknown_card() {
    let tcg = tcg();
    let house = tcg.open_auction_house(config()).await.unwrap();
    let err = tcg
        .sell(sell(house.id, "black_lotus".into()))
        .await
        .unwrap_err();
    assert_eq!(err, TcgError::UnknownCard("black_lotus".into()));
}

#[tokio::test]
async fn auction_errors_are_passed_through() {
    let tcg = tcg();
    let house = tcg.open_auction_house(config()).await.unwrap();
    tcg.sell(sell(house.id, drake())).await.unwrap();
    let missing = ListingId(Default::default());
    let err = tcg.buyout(alice(), missing).await.unwrap_err();
    assert_eq!(
        err,
        TcgError::Auction(agora_domain::AuctionError::ListingNotFound)
    );
}

fn op(n: u128) -> OpId {
    OpId(Uuid::from_u128(n))
}

type CollectorEvents = InMemoryEventStore<PlayerRef, CollectorEvent>;
type TreasuryEvents = InMemoryEventStore<TreasuryId, TreasuryEvent>;

/// The ledger with Alice opened (100 dust, 2 drakes), and projectors feeding fresh read
/// models from its event stores.
struct Ledger {
    ledger: LedgerService<CollectorEvents, TreasuryEvents>,
    collectors: InMemoryCollectorProjection,
    collector_projector:
        Projector<CollectorEvents, InMemoryCollectorProjection, PlayerRef, CollectorEvent>,
    treasury: InMemoryTreasuryProjection,
    treasury_projector:
        Projector<TreasuryEvents, InMemoryTreasuryProjection, TreasuryId, TreasuryEvent>,
}

impl Ledger {
    async fn new() -> Self {
        let collector_events = CollectorEvents::default();
        let treasury_events = TreasuryEvents::default();
        let ledger = LedgerService::new(collector_events.clone(), treasury_events.clone());
        ledger
            .open_collector(Collector::new(alice(), Money(100)).with_card(drake(), 2))
            .await
            .unwrap();
        let collectors = InMemoryCollectorProjection::default();
        let treasury = InMemoryTreasuryProjection::default();
        Self {
            ledger,
            collector_projector: Projector::new(collector_events, collectors.clone()),
            collectors,
            treasury_projector: Projector::new(treasury_events, treasury.clone()),
            treasury,
        }
    }

    /// Alice as seen by the read model once it caught up with the ledger.
    async fn alice(&self) -> Collector {
        self.collector_projector.catch_up().await.unwrap();
        self.collectors.get(&alice()).unwrap()
    }
}

#[tokio::test]
async fn ledger_moves_dust_and_cards() {
    let fixture = Ledger::new().await;
    let ledger = &fixture.ledger;
    ledger.debit(op(1), &alice(), Money(30)).await.unwrap();
    ledger.credit(op(2), &alice(), Money(5)).await.unwrap();
    ledger
        .withdraw_card(op(3), &alice(), &drake(), 1)
        .await
        .unwrap();
    ledger
        .deposit_card(op(4), &alice(), &drake(), 1)
        .await
        .unwrap();
    // The projector has not run yet.
    assert_eq!(fixture.collectors.list().await.unwrap(), vec![]);

    let alice = fixture.alice().await;
    assert_eq!(alice.dust, Money(75));
    assert_eq!(alice.copies(&drake()), 2);
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
    assert_eq!(fixture.alice().await.dust, Money(70));
}

#[tokio::test]
async fn failed_ledger_call_leaves_collector_unchanged() {
    let fixture = Ledger::new().await;
    let err = fixture
        .ledger
        .debit(op(1), &alice(), Money(101))
        .await
        .unwrap_err();
    assert_eq!(err, TcgError::InsufficientDust);
    assert_eq!(fixture.alice().await.dust, Money(100));

    let err = fixture
        .ledger
        .debit(op(2), &"nobody".into(), Money(1))
        .await
        .unwrap_err();
    assert_eq!(err, TcgError::UnknownCollector("nobody".into()));
}

#[tokio::test]
async fn opening_a_collector_twice_fails() {
    let fixture = Ledger::new().await;
    let err = fixture
        .ledger
        .open_collector(Collector::new(alice(), Money(0)))
        .await
        .unwrap_err();
    assert_eq!(err, TcgError::CollectorExists(alice()));
}

#[tokio::test]
async fn taken_dust_goes_to_the_treasury_once() {
    let fixture = Ledger::new().await;
    let listing = ListingId(Default::default());
    let commission = TakenReason::Commission { listing };
    for _ in 0..2 {
        fixture
            .ledger
            .take_dust(op(1), Money(7), commission)
            .await
            .unwrap();
    }
    fixture
        .ledger
        .take_dust(op(2), Money(3), TakenReason::ForfeitedDeposit { listing })
        .await
        .unwrap();
    fixture.treasury_projector.catch_up().await.unwrap();
    assert_eq!(fixture.treasury.total().await.unwrap(), Money(10));
}

#[tokio::test]
async fn projection_rebuilds_from_the_events() {
    let collector_events = CollectorEvents::default();
    let ledger = LedgerService::new(collector_events.clone(), TreasuryEvents::default());
    ledger
        .open_collector(Collector::new(alice(), Money(100)))
        .await
        .unwrap();
    for n in 1..=300 {
        ledger.credit(op(n), &alice(), Money(1)).await.unwrap();
    }

    let collectors = InMemoryCollectorProjection::default();
    let projector = Projector::new(collector_events, collectors.clone());
    assert_eq!(projector.catch_up().await.unwrap(), 301);
    assert_eq!(projector.catch_up().await.unwrap(), 0);
    assert_eq!(collectors.get(&alice()).unwrap().dust, Money(400));
}

#[tokio::test]
async fn projection_skips_replayed_events() {
    let collector_events = CollectorEvents::default();
    let ledger = LedgerService::new(collector_events.clone(), TreasuryEvents::default());
    ledger
        .open_collector(Collector::new(alice(), Money(100)))
        .await
        .unwrap();
    ledger.credit(op(1), &alice(), Money(5)).await.unwrap();

    let collectors = InMemoryCollectorProjection::default();
    let batch = collector_events.read_all(0, 10).await.unwrap();
    collectors.apply(&batch).await.unwrap();
    collectors.apply(&batch).await.unwrap();
    assert_eq!(collectors.get(&alice()).unwrap().dust, Money(105));
}

/// Fails the first append with a conflict, as if another write happened in between.
struct ConflictOnce<S> {
    inner: S,
    conflicted: AtomicBool,
}

impl<S: EventStore<PlayerRef, CollectorEvent>> EventStore<PlayerRef, CollectorEvent>
    for ConflictOnce<S>
{
    async fn load(&self, stream: &PlayerRef) -> Result<Vec<CollectorEvent>, StoreError> {
        self.inner.load(stream).await
    }

    async fn append(
        &self,
        stream: &PlayerRef,
        expected: u64,
        events: Vec<CollectorEvent>,
    ) -> Result<u64, StoreError> {
        if !self.conflicted.swap(true, Ordering::SeqCst) {
            return Err(StoreError::Conflict);
        }
        self.inner.append(stream, expected, events).await
    }

    async fn read_all(
        &self,
        after: u64,
        limit: usize,
    ) -> Result<Vec<Recorded<PlayerRef, CollectorEvent>>, StoreError> {
        self.inner.read_all(after, limit).await
    }
}

#[tokio::test]
async fn ledger_retries_on_conflict() {
    let collector_events = CollectorEvents::default();
    let store = ConflictOnce {
        inner: collector_events.clone(),
        conflicted: AtomicBool::new(false),
    };
    let ledger = LedgerService::new(store, TreasuryEvents::default());
    ledger
        .open_collector(Collector::new(alice(), Money(100)))
        .await
        .unwrap();
    assert_eq!(collector_events.load(&alice()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn caught_up_views_show_every_recorded_event() {
    let collector_events = CollectorEvents::default();
    let treasury_events = TreasuryEvents::default();
    let ledger = LedgerService::new(collector_events.clone(), treasury_events.clone());
    let collectors = CaughtUpView::new(collector_events, InMemoryCollectorProjection::default());
    let treasury = CaughtUpView::new(treasury_events, InMemoryTreasuryProjection::default());

    ledger
        .open_collector(Collector::new(alice(), Money(100)))
        .await
        .unwrap();
    ledger.debit(op(1), &alice(), Money(30)).await.unwrap();
    let listing = ListingId(Default::default());
    ledger
        .take_dust(op(2), Money(7), TakenReason::Commission { listing })
        .await
        .unwrap();

    let listed = collectors.list().await.unwrap();
    assert_eq!(listed, vec![Collector::new(alice(), Money(70))]);
    assert_eq!(treasury.total().await.unwrap(), Money(7));
}
