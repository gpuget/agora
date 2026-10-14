use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agora_api::{AuctionApi, BidOutcome, CreateListing};
use agora_domain::{
    AuctionError, AuctionHouse, Bps, DurationOption, HouseConfig, HouseId, ItemStack, ItemType,
    Listing, ListingId, ListingStatus, Money, Page, PlayerRef, PortError, SearchQuery, Timestamp,
};
use agora_spi::{AuctionRepository, RepoError, SinkReason};

use crate::AuctionService;
use crate::memory::{
    InMemoryAuctionRepository, InMemoryInventory, InMemoryWallet, ManualClock, RecordingMoneySink,
};

type Service<R = InMemoryAuctionRepository> =
    AuctionService<R, InMemoryInventory, InMemoryWallet, RecordingMoneySink, ManualClock>;

const SHORT: Duration = Duration::from_secs(12 * 3600);
const LONG: Duration = Duration::from_secs(48 * 3600);
const START_BALANCE: Money = Money(1_000_000);

fn alice() -> PlayerRef {
    "alice".into()
}

fn bob() -> PlayerRef {
    "bob".into()
}

fn carol() -> PlayerRef {
    "carol".into()
}

fn cloth() -> ItemType {
    "linen_cloth".into()
}

fn config() -> HouseConfig {
    HouseConfig {
        name: "neutral".into(),
        commission: Bps(500),
        min_increment: Money(1),
        increment: Bps(500),
        deposit: Bps(1_000),
        durations: vec![
            DurationOption {
                duration: SHORT,
                deposit_multiplier: 1,
            },
            DurationOption {
                duration: LONG,
                deposit_multiplier: 4,
            },
        ],
    }
}

fn service_with<R: AuctionRepository>(repository: R) -> Service<R> {
    let service = AuctionService::new(
        repository,
        InMemoryInventory::default(),
        InMemoryWallet::default(),
        RecordingMoneySink::default(),
        ManualClock::new(Timestamp(1_000_000)),
    );
    for player in [alice(), bob(), carol()] {
        service.wallet().set_balance(&player, START_BALANCE);
    }
    service.inventory().give(&alice(), &cloth(), 100);
    service
}

async fn setup() -> (Service, HouseId) {
    let service = service_with(InMemoryAuctionRepository::default());
    let house = service.create_auction_house(config()).await.unwrap();
    (service, house)
}

/// 20 cloths with a base value of 10 each: the deposit is 20 on the short duration.
fn listing_cmd(house: HouseId, start: u64, buyout: Option<u64>) -> CreateListing {
    CreateListing {
        house,
        seller: alice(),
        item: ItemStack {
            item_type: cloth(),
            quantity: 20,
            base_value: Money(10),
            attributes: serde_json::Value::Null,
        },
        start_unit_price: Money(start),
        buyout_unit_price: buyout.map(Money),
        duration: SHORT,
    }
}

fn stored(service: &Service, id: ListingId) -> Listing {
    service.repository().listing(id).unwrap()
}

fn balance<R: AuctionRepository>(service: &Service<R>, player: &PlayerRef) -> Money {
    service.wallet().balance(player)
}

fn query(house: HouseId) -> SearchQuery {
    SearchQuery {
        house,
        item_type: None,
        seller: None,
        high_bidder: None,
        limit: 50,
        offset: 0,
    }
}

// ---- auction houses ----

#[tokio::test]
async fn house_config_is_validated() {
    let (service, _) = setup().await;
    let cases = [
        HouseConfig {
            durations: vec![],
            ..config()
        },
        HouseConfig {
            min_increment: Money::ZERO,
            ..config()
        },
        HouseConfig {
            commission: Bps(10_001),
            ..config()
        },
    ];
    for config in cases {
        let err = service.create_auction_house(config).await.unwrap_err();
        assert!(matches!(err, AuctionError::InvalidConfig(_)), "{err:?}");
    }
}

// ---- listing creation ----

#[tokio::test]
async fn create_listing_escrows_stack_and_charges_deposit() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();

    assert_eq!(service.inventory().count(&alice(), &cloth()), 80);
    // 20 * 10 * 10 % * 1
    assert_eq!(balance(&service, &alice()), Money(START_BALANCE.0 - 20));
    let listing = stored(&service, id);
    assert_eq!(listing.deposit, Money(20));
    assert_eq!(listing.status, ListingStatus::Active);
    assert_eq!(listing.ends_at, Timestamp(1_000_000 + 12 * 3600 * 1000));
}

#[tokio::test]
async fn deposit_uses_duration_multiplier() {
    let (service, house) = setup().await;
    let cmd = CreateListing {
        duration: LONG,
        ..listing_cmd(house, 5, None)
    };
    let id = service.create_listing(cmd).await.unwrap();
    assert_eq!(stored(&service, id).deposit, Money(80));
}

#[tokio::test]
async fn create_listing_rejects_invalid_input() {
    let (service, house) = setup().await;
    let mut zero_quantity = listing_cmd(house, 5, None);
    zero_quantity.item.quantity = 0;
    let mut overflow = listing_cmd(house, u64::MAX / 10, None);
    overflow.item.base_value = Money(0);
    let cases = [
        (zero_quantity, AuctionError::InvalidQuantity),
        (listing_cmd(house, 0, None), AuctionError::InvalidPrice),
        (
            listing_cmd(house, 5, Some(4)),
            AuctionError::BuyoutBelowStart,
        ),
        (
            CreateListing {
                duration: Duration::from_secs(60),
                ..listing_cmd(house, 5, None)
            },
            AuctionError::DurationNotAllowed,
        ),
        (
            CreateListing {
                house: HouseId(uuid::Uuid::nil()),
                ..listing_cmd(house, 5, None)
            },
            AuctionError::HouseNotFound,
        ),
        (overflow, AuctionError::Overflow),
    ];
    for (cmd, expected) in cases {
        assert_eq!(service.create_listing(cmd).await.unwrap_err(), expected);
    }
    assert_eq!(service.inventory().count(&alice(), &cloth()), 100);
    assert_eq!(balance(&service, &alice()), START_BALANCE);
}

#[tokio::test]
async fn create_listing_fails_without_the_items() {
    let (service, house) = setup().await;
    let cmd = CreateListing {
        seller: bob(),
        ..listing_cmd(house, 5, None)
    };
    let err = service.create_listing(cmd).await.unwrap_err();
    assert_eq!(err, AuctionError::Port(PortError::ItemUnavailable));
}

#[tokio::test]
async fn create_listing_returns_stack_when_deposit_cannot_be_paid() {
    let (service, house) = setup().await;
    service.wallet().set_balance(&alice(), Money(19));
    let err = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap_err();
    assert_eq!(err, AuctionError::Port(PortError::InsufficientFunds));
    assert_eq!(service.inventory().count(&alice(), &cloth()), 100);
}

// ---- bids ----

#[tokio::test]
async fn first_bid_must_reach_start_price_and_debits_the_total() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();

    let err = service.place_bid(bob(), id, Money(4)).await.unwrap_err();
    assert_eq!(err, AuctionError::BidTooLow { minimum: Money(5) });

    let outcome = service.place_bid(bob(), id, Money(5)).await.unwrap();
    assert_eq!(outcome, BidOutcome::HighBidder);
    assert_eq!(balance(&service, &bob()), Money(START_BALANCE.0 - 100));
}

#[tokio::test]
async fn outbid_refunds_previous_bidder() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 100, None))
        .await
        .unwrap();
    service.place_bid(bob(), id, Money(100)).await.unwrap();

    // 100 + max(1, 100 * 5 %) = 105
    let err = service
        .place_bid(carol(), id, Money(104))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        AuctionError::BidTooLow {
            minimum: Money(105)
        }
    );

    service.place_bid(carol(), id, Money(105)).await.unwrap();
    assert_eq!(balance(&service, &bob()), START_BALANCE);
    assert_eq!(balance(&service, &carol()), Money(START_BALANCE.0 - 2_100));
    assert_eq!(stored(&service, id).high_bid.unwrap().bidder, carol());
}

#[tokio::test]
async fn min_increment_applies_on_small_prices() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();
    service.place_bid(bob(), id, Money(5)).await.unwrap();
    // 5 * 5 % rounds down to 0, so the absolute minimum of 1 applies.
    let err = service.place_bid(carol(), id, Money(5)).await.unwrap_err();
    assert_eq!(err, AuctionError::BidTooLow { minimum: Money(6) });
}

#[tokio::test]
async fn seller_cannot_bid_or_buy() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();
    assert_eq!(
        service.place_bid(alice(), id, Money(5)).await.unwrap_err(),
        AuctionError::SellerCannotBuy
    );
    assert_eq!(
        service.buyout(alice(), id).await.unwrap_err(),
        AuctionError::SellerCannotBuy
    );
}

#[tokio::test]
async fn high_bidder_cannot_outbid_themselves() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();
    service.place_bid(bob(), id, Money(5)).await.unwrap();
    assert_eq!(
        service.place_bid(bob(), id, Money(10)).await.unwrap_err(),
        AuctionError::AlreadyHighBidder
    );
}

#[tokio::test]
async fn insufficient_funds_leave_listing_unchanged() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();
    service.wallet().set_balance(&bob(), Money(99));
    let err = service.place_bid(bob(), id, Money(5)).await.unwrap_err();
    assert_eq!(err, AuctionError::Port(PortError::InsufficientFunds));
    assert_eq!(stored(&service, id).high_bid, None);
}

#[tokio::test]
async fn bid_reaching_buyout_buys_at_buyout_price() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();

    let outcome = service.place_bid(bob(), id, Money(50)).await.unwrap();
    assert_eq!(outcome, BidOutcome::Bought);
    assert_eq!(balance(&service, &bob()), Money(START_BALANCE.0 - 160));
    assert_eq!(service.inventory().count(&bob(), &cloth()), 20);
    assert_eq!(
        stored(&service, id).status,
        ListingStatus::Sold {
            buyer: bob(),
            unit_price: Money(8)
        }
    );
}

#[tokio::test]
async fn bid_after_end_is_rejected_before_settlement() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();
    service.clock().advance(SHORT);
    assert_eq!(
        service.place_bid(bob(), id, Money(5)).await.unwrap_err(),
        AuctionError::Expired
    );
    assert_eq!(
        service.buyout(bob(), id).await.unwrap_err(),
        AuctionError::Expired
    );
    assert_eq!(
        service.cancel_listing(alice(), id).await.unwrap_err(),
        AuctionError::Expired
    );
}

// ---- buyout ----

#[tokio::test]
async fn buyout_pays_seller_and_sink() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();
    service.buyout(bob(), id).await.unwrap();

    // total 160, commission 8, deposit 20 returned
    assert_eq!(
        balance(&service, &alice()),
        Money(START_BALANCE.0 - 20 + 160 - 8 + 20)
    );
    assert_eq!(balance(&service, &bob()), Money(START_BALANCE.0 - 160));
    assert_eq!(service.inventory().count(&bob(), &cloth()), 20);
    assert_eq!(
        service.sink().records(),
        vec![(house, Money(8), SinkReason::Commission { listing: id })]
    );
}

#[tokio::test]
async fn buyout_refunds_current_high_bidder() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();
    service.place_bid(bob(), id, Money(5)).await.unwrap();
    service.buyout(carol(), id).await.unwrap();

    assert_eq!(balance(&service, &bob()), START_BALANCE);
    assert_eq!(service.inventory().count(&carol(), &cloth()), 20);
}

#[tokio::test]
async fn buyout_requires_a_buyout_price() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();
    assert_eq!(
        service.buyout(bob(), id).await.unwrap_err(),
        AuctionError::NoBuyout
    );
    assert_eq!(balance(&service, &bob()), START_BALANCE);
}

#[tokio::test]
async fn sold_listing_is_closed() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();
    service.buyout(bob(), id).await.unwrap();
    assert_eq!(
        service.buyout(carol(), id).await.unwrap_err(),
        AuctionError::NotActive
    );
    assert_eq!(
        service.place_bid(carol(), id, Money(9)).await.unwrap_err(),
        AuctionError::NotActive
    );
}

// ---- cancellation ----

#[tokio::test]
async fn cancel_returns_stack_and_forfeits_deposit() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();
    service.cancel_listing(alice(), id).await.unwrap();

    assert_eq!(service.inventory().count(&alice(), &cloth()), 100);
    assert_eq!(balance(&service, &alice()), Money(START_BALANCE.0 - 20));
    assert_eq!(
        service.sink().records(),
        vec![(
            house,
            Money(20),
            SinkReason::ForfeitedDeposit { listing: id }
        )]
    );
    assert_eq!(stored(&service, id).status, ListingStatus::Cancelled);
}

#[tokio::test]
async fn cancel_is_refused_with_bids_or_by_someone_else() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();
    assert_eq!(
        service.cancel_listing(bob(), id).await.unwrap_err(),
        AuctionError::NotSeller
    );
    service.place_bid(bob(), id, Money(5)).await.unwrap();
    assert_eq!(
        service.cancel_listing(alice(), id).await.unwrap_err(),
        AuctionError::HasBids
    );
}

// ---- settlement ----

#[tokio::test]
async fn settle_without_bid_returns_stack_and_forfeits_deposit() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();

    assert_eq!(service.settle_expired().await.unwrap(), 0);
    service.clock().advance(SHORT);
    assert_eq!(service.settle_expired().await.unwrap(), 1);

    assert_eq!(stored(&service, id).status, ListingStatus::Expired);
    assert_eq!(service.inventory().count(&alice(), &cloth()), 100);
    assert_eq!(service.sink().total(), Money(20));
    assert_eq!(service.settle_expired().await.unwrap(), 0);
}

#[tokio::test]
async fn settle_with_bid_sells_to_high_bidder() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 100, None))
        .await
        .unwrap();
    service.place_bid(bob(), id, Money(100)).await.unwrap();
    service.clock().advance(SHORT);
    service.settle_expired().await.unwrap();

    // total 2000, commission 100, deposit 20 returned
    assert_eq!(
        stored(&service, id).status,
        ListingStatus::Sold {
            buyer: bob(),
            unit_price: Money(100)
        }
    );
    assert_eq!(service.inventory().count(&bob(), &cloth()), 20);
    assert_eq!(
        balance(&service, &alice()),
        Money(START_BALANCE.0 - 20 + 2_000 - 100 + 20)
    );
    assert_eq!(service.sink().total(), Money(100));
}

#[tokio::test]
async fn commission_is_frozen_at_creation() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, Some(8)))
        .await
        .unwrap();
    // Simulate a config change after creation.
    let mut changed = service
        .repository()
        .get_house(house)
        .await
        .unwrap()
        .unwrap();
    changed.config.commission = Bps(5_000);
    service
        .repository()
        .insert_house(&AuctionHouse { ..changed })
        .await
        .unwrap();

    service.buyout(bob(), id).await.unwrap();
    assert_eq!(service.sink().total(), Money(8));
}

// ---- search ----

#[tokio::test]
async fn search_filters_and_sorts_by_unit_price() {
    let (service, house) = setup().await;
    service.inventory().give(&bob(), &cloth(), 20);
    let expensive = service
        .create_listing(listing_cmd(house, 9, None))
        .await
        .unwrap();
    let cheap = service
        .create_listing(listing_cmd(house, 5, Some(7)))
        .await
        .unwrap();
    let by_bob = service
        .create_listing(CreateListing {
            seller: bob(),
            ..listing_cmd(house, 6, None)
        })
        .await
        .unwrap();
    service
        .place_bid(carol(), expensive, Money(9))
        .await
        .unwrap();

    let ids = |page: Page<Listing>| page.items.into_iter().map(|l| l.id).collect::<Vec<_>>();

    // Sort keys: cheap 7 (buyout), by_bob 6, expensive 9.
    let all = service.search(query(house)).await.unwrap();
    assert_eq!(all.total, 3);
    assert_eq!(ids(all), vec![by_bob, cheap, expensive]);

    let mine = service
        .search(SearchQuery {
            seller: Some(bob()),
            ..query(house)
        })
        .await
        .unwrap();
    assert_eq!(ids(mine), vec![by_bob]);

    let bids = service
        .search(SearchQuery {
            high_bidder: Some(carol()),
            ..query(house)
        })
        .await
        .unwrap();
    assert_eq!(ids(bids), vec![expensive]);

    let page = service
        .search(SearchQuery {
            limit: 1,
            offset: 1,
            ..query(house)
        })
        .await
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(ids(page), vec![cheap]);

    let none = service
        .search(SearchQuery {
            item_type: Some("copper_ore".into()),
            ..query(house)
        })
        .await
        .unwrap();
    assert_eq!(none.total, 0);
}

#[tokio::test]
async fn search_hides_closed_listings_and_checks_limit() {
    let (service, house) = setup().await;
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();
    service.cancel_listing(alice(), id).await.unwrap();
    assert_eq!(service.search(query(house)).await.unwrap().total, 0);

    let err = service
        .search(SearchQuery {
            limit: 0,
            ..query(house)
        })
        .await
        .unwrap_err();
    assert_eq!(err, AuctionError::InvalidPage { max: 100 });
}

// ---- concurrency ----

/// Fails the first `update_listing` with a conflict, as if another writer won the race.
#[derive(Default)]
struct ConflictOnce {
    inner: InMemoryAuctionRepository,
    conflicted: AtomicBool,
}

impl AuctionRepository for ConflictOnce {
    async fn insert_house(&self, house: &AuctionHouse) -> Result<(), RepoError> {
        self.inner.insert_house(house).await
    }

    async fn get_house(&self, id: HouseId) -> Result<Option<AuctionHouse>, RepoError> {
        self.inner.get_house(id).await
    }

    async fn insert_listing(&self, listing: &Listing) -> Result<(), RepoError> {
        self.inner.insert_listing(listing).await
    }

    async fn get_listing(&self, id: ListingId) -> Result<Option<Listing>, RepoError> {
        self.inner.get_listing(id).await
    }

    async fn update_listing(
        &self,
        listing: &Listing,
        expected_version: u64,
    ) -> Result<(), RepoError> {
        if !self.conflicted.swap(true, Ordering::SeqCst) {
            return Err(RepoError::Conflict);
        }
        self.inner.update_listing(listing, expected_version).await
    }

    async fn find_expired(&self, now: Timestamp, limit: u32) -> Result<Vec<Listing>, RepoError> {
        self.inner.find_expired(now, limit).await
    }

    async fn search(&self, query: &SearchQuery) -> Result<Page<Listing>, RepoError> {
        self.inner.search(query).await
    }
}

#[tokio::test]
async fn bid_conflict_is_retried_without_double_debit() {
    let service = service_with(ConflictOnce::default());
    let house = service.create_auction_house(config()).await.unwrap();
    let id = service
        .create_listing(listing_cmd(house, 5, None))
        .await
        .unwrap();

    service.place_bid(bob(), id, Money(5)).await.unwrap();

    assert_eq!(balance(&service, &bob()), Money(START_BALANCE.0 - 100));
    let listing = service.repository().inner.listing(id).unwrap();
    assert_eq!(listing.high_bid.unwrap().bidder, bob());
    assert_eq!(listing.version, 1);
}
