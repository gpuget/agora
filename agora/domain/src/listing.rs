use serde::{Deserialize, Serialize};

use crate::{Bps, HouseId, ItemStack, ListingId, Money, PlayerRef, Timestamp};

/// A bid on a listing.
///
/// The total of the bid (unit price times the quantity of the stack) is debited from the
/// bidder when the bid is placed and held in escrow. It is refunded when the bid is outbid or
/// the listing is bought out. It pays the seller, less the commission, when the listing is
/// settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bid {
    pub bidder: PlayerRef,
    pub unit_price: Money,
}

/// Where a listing is in its life. Only an Active listing changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ListingStatus {
    /// On sale. Stays Active after its end time until it is settled.
    Active,
    /// Bought out, or settled with a high bid.
    ///
    /// After partial buyouts of a commodity, holds the buyer and unit price of the last
    /// buyout only.
    Sold {
        buyer: PlayerRef,
        unit_price: Money,
    },
    /// Ended without bids.
    ///
    /// Also the end of a commodity sold in part: the unsold items go back to the seller.
    Expired,
    /// Withdrawn by the seller before any bid.
    Cancelled,
}

/// A stack put up for sale. All prices are per unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    pub id: ListingId,
    pub house: HouseId,
    pub seller: PlayerRef,
    /// The items held in escrow. `item.quantity` is what is still on sale.
    pub item: ItemStack,
    /// Lowest unit price of the first bid. Greater than zero.
    pub start_unit_price: Money,
    /// Unit price at which the listing can be bought at once. At least the start price.
    /// Always set on a commodity.
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
    /// End time. From then on the listing refuses bids, buyouts and cancellation and waits
    /// for settlement.
    pub ends_at: Timestamp,
    /// Highest bid, `None` until the first bid. Never set on a commodity. Left as is when the
    /// listing is bought out.
    pub high_bid: Option<Bid>,
    pub status: ListingStatus,
    /// Optimistic locking version, incremented on every update.
    pub version: u64,
}

impl Listing {
    /// `unit_price * quantity`, `None` on overflow.
    ///
    /// Uses the quantity still on sale.
    pub fn total(&self, unit_price: Money) -> Option<Money> {
        unit_price.checked_mul(u64::from(self.item.quantity))
    }

    /// Part of the deposit that comes back when `quantity` of the remaining items are sold.
    /// The whole deposit when `quantity` is all of them, so nothing is lost to rounding.
    ///
    /// Otherwise `deposit * quantity / remaining quantity`, rounded down.
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
    ///
    /// The start price without a bid. Otherwise the high bid plus the larger of
    /// `min_increment` and `high bid * increment` (rounded down). `None` on overflow.
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
