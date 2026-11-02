use serde::{Deserialize, Serialize};

use crate::{Bps, HouseId, ItemStack, ListingId, Money, PlayerRef, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bid {
    pub bidder: PlayerRef,
    pub unit_price: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ListingStatus {
    Active,
    Sold {
        buyer: PlayerRef,
        unit_price: Money,
    },
    /// Ended without bids.
    Expired,
    Cancelled,
}

/// A stack put up for sale. All prices are per unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    pub id: ListingId,
    pub house: HouseId,
    pub seller: PlayerRef,
    pub item: ItemStack,
    pub start_unit_price: Money,
    pub buyout_unit_price: Option<Money>,
    /// Frozen from the house config at creation.
    pub commission: Bps,
    /// Frozen from the house config at creation.
    pub min_increment: Money,
    /// Frozen from the house config at creation.
    pub increment: Bps,
    /// Paid by the seller at creation, for the quantity still on sale. Returned in proportion
    /// to the quantity sold, forfeited for what stays unsold.
    pub deposit: Money,
    pub created_at: Timestamp,
    pub ends_at: Timestamp,
    pub high_bid: Option<Bid>,
    pub status: ListingStatus,
    /// Optimistic locking version, incremented on every update.
    pub version: u64,
}

impl Listing {
    /// `unit_price * quantity`, `None` on overflow.
    pub fn total(&self, unit_price: Money) -> Option<Money> {
        unit_price.checked_mul(u64::from(self.item.quantity))
    }

    /// Part of the deposit that comes back when `quantity` of the remaining items are sold.
    /// The whole deposit when `quantity` is all of them, so nothing is lost to rounding.
    pub fn deposit_share(&self, quantity: u32) -> Money {
        if quantity >= self.item.quantity {
            return self.deposit;
        }
        let share = u128::from(self.deposit.0) * u128::from(quantity)
            / u128::from(self.item.quantity);
        // `share` is below `deposit`, so it fits.
        Money(share as u64)
    }

    /// Minimum unit price of the next bid.
    pub fn min_next_bid(&self) -> Option<Money> {
        match &self.high_bid {
            None => Some(self.start_unit_price),
            Some(bid) => {
                let step = self
                    .min_increment
                    .max(bid.unit_price.apply_bps(self.increment));
                bid.unit_price.checked_add(step)
            }
        }
    }

    /// Highest bid, or the start price when there is no bid.
    pub fn current_unit_price(&self) -> Money {
        self.high_bid
            .as_ref()
            .map_or(self.start_unit_price, |b| b.unit_price)
    }

    /// Unit price used to sort search results: buyout if any, else the current price.
    pub fn sort_unit_price(&self) -> Money {
        self.buyout_unit_price
            .unwrap_or_else(|| self.current_unit_price())
    }

    /// `true` once `ends_at` is reached, even if the listing is not settled yet.
    pub fn is_past_end(&self, now: Timestamp) -> bool {
        now >= self.ends_at
    }
}
