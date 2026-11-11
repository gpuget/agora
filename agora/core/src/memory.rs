//! In-memory adapters, for tests and demos. State is lost on restart.
//!
//! The inventory, wallet and sink ignore the [`OpId`]: a replayed call is applied again.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use agora_domain::{
    AuctionHouse, HouseId, ItemStack, ItemType, Listing, ListingId, ListingStatus, Money, OpId,
    Page, PlayerRef, PortError, SearchQuery, Timestamp,
};
use agora_spi::{AuctionRepository, Clock, Inventory, MoneySink, RepoError, SinkReason, Wallet};

/// Houses and listings in hash maps. `update_listing` checks the version and fails with
/// [`RepoError::Other`] for an unknown listing. Inserts replace an entry with the same id.
#[derive(Default)]
pub struct InMemoryAuctionRepository {
    houses: Mutex<HashMap<HouseId, AuctionHouse>>,
    listings: Mutex<HashMap<ListingId, Listing>>,
}

impl InMemoryAuctionRepository {
    /// Every listing, whatever its status.
    pub fn listing(&self, id: ListingId) -> Option<Listing> {
        self.listings.lock().unwrap().get(&id).cloned()
    }
}

impl AuctionRepository for InMemoryAuctionRepository {
    async fn insert_house(&self, house: &AuctionHouse) -> Result<(), RepoError> {
        self.houses.lock().unwrap().insert(house.id, house.clone());
        Ok(())
    }

    async fn get_house(&self, id: HouseId) -> Result<Option<AuctionHouse>, RepoError> {
        Ok(self.houses.lock().unwrap().get(&id).cloned())
    }

    async fn insert_listing(&self, listing: &Listing) -> Result<(), RepoError> {
        self.listings
            .lock()
            .unwrap()
            .insert(listing.id, listing.clone());
        Ok(())
    }

    async fn get_listing(&self, id: ListingId) -> Result<Option<Listing>, RepoError> {
        Ok(self.listing(id))
    }

    async fn update_listing(
        &self,
        listing: &Listing,
        expected_version: u64,
    ) -> Result<(), RepoError> {
        let mut listings = self.listings.lock().unwrap();
        let stored = listings
            .get_mut(&listing.id)
            .ok_or_else(|| RepoError::Other(format!("listing {} not found", listing.id)))?;
        if stored.version != expected_version {
            return Err(RepoError::Conflict);
        }
        *stored = listing.clone();
        Ok(())
    }

    async fn find_expired(&self, now: Timestamp, limit: u32) -> Result<Vec<Listing>, RepoError> {
        let listings = self.listings.lock().unwrap();
        let mut expired: Vec<Listing> = listings
            .values()
            .filter(|l| l.status == ListingStatus::Active && l.is_past_end(now))
            .cloned()
            .collect();
        expired.sort_by_key(|l| (l.ends_at, l.id));
        expired.truncate(limit as usize);
        Ok(expired)
    }

    async fn search(&self, query: &SearchQuery) -> Result<Page<Listing>, RepoError> {
        let listings = self.listings.lock().unwrap();
        let mut matches: Vec<&Listing> = listings
            .values()
            .filter(|l| {
                l.house == query.house
                    && l.status == ListingStatus::Active
                    && query
                        .item_type
                        .as_ref()
                        .is_none_or(|t| &l.item.item_type == t)
                    && query.seller.as_ref().is_none_or(|s| &l.seller == s)
                    && query
                        .high_bidder
                        .as_ref()
                        .is_none_or(|b| l.high_bid.as_ref().is_some_and(|h| &h.bidder == b))
            })
            .collect();
        matches.sort_by_key(|l| (l.sort_unit_price(), l.ends_at, l.id));
        let total = matches.len() as u64;
        let items = matches
            .into_iter()
            .skip(query.offset as usize)
            .take(query.limit as usize)
            .cloned()
            .collect();
        Ok(Page { items, total })
    }
}

/// Inventory keyed by player and item type. Ignores `attributes`.
#[derive(Default)]
pub struct InMemoryInventory {
    stacks: Mutex<HashMap<(PlayerRef, ItemType), u32>>,
}

impl InMemoryInventory {
    /// Adds `quantity` items of `item_type` to the player.
    pub fn give(&self, player: &PlayerRef, item_type: &ItemType, quantity: u32) {
        *self
            .stacks
            .lock()
            .unwrap()
            .entry((player.clone(), item_type.clone()))
            .or_default() += quantity;
    }

    /// Items of `item_type` held by the player, 0 if none.
    pub fn count(&self, player: &PlayerRef, item_type: &ItemType) -> u32 {
        self.stacks
            .lock()
            .unwrap()
            .get(&(player.clone(), item_type.clone()))
            .copied()
            .unwrap_or(0)
    }
}

impl Inventory for InMemoryInventory {
    async fn withdraw_item(
        &self,
        _op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> Result<(), PortError> {
        let mut stacks = self.stacks.lock().unwrap();
        let held = stacks
            .get_mut(&(player.clone(), item.item_type.clone()))
            .filter(|held| **held >= item.quantity)
            .ok_or(PortError::ItemUnavailable)?;
        *held -= item.quantity;
        Ok(())
    }

    async fn deposit_item(
        &self,
        _op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> Result<(), PortError> {
        self.give(player, &item.item_type, item.quantity);
        Ok(())
    }
}

/// Balances by player. An unknown player has a balance of zero.
#[derive(Default)]
pub struct InMemoryWallet {
    balances: Mutex<HashMap<PlayerRef, Money>>,
}

impl InMemoryWallet {
    /// Replaces the balance of the player.
    pub fn set_balance(&self, player: &PlayerRef, amount: Money) {
        self.balances.lock().unwrap().insert(player.clone(), amount);
    }

    /// Balance of the player.
    pub fn balance(&self, player: &PlayerRef) -> Money {
        self.balances
            .lock()
            .unwrap()
            .get(player)
            .copied()
            .unwrap_or(Money::ZERO)
    }
}

impl Wallet for InMemoryWallet {
    async fn debit(&self, _op: OpId, player: &PlayerRef, amount: Money) -> Result<(), PortError> {
        let mut balances = self.balances.lock().unwrap();
        let balance = balances.entry(player.clone()).or_default();
        *balance = balance
            .checked_sub(amount)
            .ok_or(PortError::InsufficientFunds)?;
        Ok(())
    }

    async fn credit(&self, _op: OpId, player: &PlayerRef, amount: Money) -> Result<(), PortError> {
        let mut balances = self.balances.lock().unwrap();
        let balance = balances.entry(player.clone()).or_default();
        *balance = balance
            .checked_add(amount)
            .ok_or_else(|| PortError::Other("balance overflow".into()))?;
        Ok(())
    }
}

/// Records every amount it receives.
#[derive(Default)]
pub struct RecordingMoneySink {
    records: Mutex<Vec<(HouseId, Money, SinkReason)>>,
}

impl RecordingMoneySink {
    /// Every call received, in order.
    pub fn records(&self) -> Vec<(HouseId, Money, SinkReason)> {
        self.records.lock().unwrap().clone()
    }

    /// Sum of every amount received.
    pub fn total(&self) -> Money {
        Money(
            self.records
                .lock()
                .unwrap()
                .iter()
                .map(|(_, m, _)| m.0)
                .sum(),
        )
    }
}

impl MoneySink for RecordingMoneySink {
    async fn absorb(
        &self,
        _op: OpId,
        house: HouseId,
        amount: Money,
        reason: SinkReason,
    ) -> Result<(), PortError> {
        self.records.lock().unwrap().push((house, amount, reason));
        Ok(())
    }
}

/// Clock that only moves when told to.
pub struct ManualClock(AtomicU64);

impl ManualClock {
    /// A clock showing `start`.
    pub fn new(start: Timestamp) -> Self {
        Self(AtomicU64::new(start.0))
    }

    /// Moves the clock forward by `duration`.
    ///
    /// # Panics
    ///
    /// If `duration` in milliseconds does not fit in a `u64`.
    pub fn advance(&self, duration: Duration) {
        let millis = u64::try_from(duration.as_millis()).expect("duration fits in u64 millis");
        self.0.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.0.load(Ordering::SeqCst))
    }
}
