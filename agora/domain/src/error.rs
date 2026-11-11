use thiserror::Error;

use crate::Money;

/// Failure reported by an outbound port implemented by the game.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortError {
    /// A debit exceeds the balance of the player.
    #[error("insufficient funds")]
    InsufficientFunds,
    /// The player does not hold the stack to withdraw.
    #[error("item unavailable")]
    ItemUnavailable,
    /// Any other failure, described by the port.
    #[error("{0}")]
    Other(String),
}

/// Failure of a use case of the auction core.
///
/// Errors found while checking a request are returned before any money or item moves.
/// [`AuctionError::Port`] and [`AuctionError::Repository`] can come later: see each variant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuctionError {
    /// The house config breaks a rule of [`HouseConfig::validate`](crate::HouseConfig::validate).
    /// The message names the rule.
    #[error("invalid auction house config: {0}")]
    InvalidConfig(&'static str),
    /// No house has the id given to create a listing.
    #[error("auction house not found")]
    HouseNotFound,
    /// No listing has the given id.
    #[error("listing not found")]
    ListingNotFound,
    /// A new listing has a stack of zero items.
    #[error("quantity must be greater than zero")]
    InvalidQuantity,
    /// A new listing has a start price of zero.
    #[error("start price must be greater than zero")]
    InvalidPrice,
    /// A new listing has a buyout price below its start price.
    #[error("buyout price is below start price")]
    BuyoutBelowStart,
    /// The duration of a new listing is not one of the house durations.
    #[error("duration not allowed by this auction house")]
    DurationNotAllowed,
    /// An amount or a timestamp does not fit in a `u64`, for example the total price of a
    /// stack or the end time of a listing.
    #[error("amount overflow")]
    Overflow,
    /// The listing is Sold, Expired or Cancelled.
    #[error("listing is not active")]
    NotActive,
    /// The listing is still Active but its end time is reached: it only waits for
    /// settlement.
    #[error("listing has expired")]
    Expired,
    /// The seller tried to bid on or buy their own listing.
    #[error("seller cannot bid on or buy their own listing")]
    SellerCannotBuy,
    /// The bidder already holds the high bid and bids below the buyout price.
    #[error("already the highest bidder")]
    AlreadyHighBidder,
    /// The bid is below the minimum for the next bid.
    #[error("bid too low, minimum unit price is {}", minimum.0)]
    BidTooLow {
        /// Lowest accepted unit price, from
        /// [`Listing::min_next_bid`](crate::Listing::min_next_bid).
        minimum: Money,
    },
    /// Someone other than the seller tried to cancel the listing.
    #[error("only the seller can cancel this listing")]
    NotSeller,
    /// The listing to cancel has a high bid.
    #[error("listing has bids and cannot be cancelled")]
    HasBids,
    /// A buyout targets a listing without a buyout price.
    #[error("listing has no buyout price")]
    NoBuyout,
    /// A new commodity listing has no buyout price.
    #[error("a commodity listing needs a buyout price")]
    CommodityNeedsBuyout,
    /// A bid targets a commodity listing.
    #[error("a commodity cannot receive bids")]
    BidsNotAllowed,
    /// A commodity buyout asks for zero items or more than are left.
    #[error("quantity must be between 1 and {available}")]
    QuantityOutOfRange {
        /// Items still on sale.
        available: u32,
    },
    /// A buyout of an item that is not a commodity asks for less or more than the whole
    /// stack.
    #[error("this item can only be bought as a whole, quantity must be {available}")]
    PartialNotAllowed {
        /// Items of the stack.
        available: u32,
    },
    /// The search limit is zero or above the maximum.
    #[error("page limit must be between 1 and {max}")]
    InvalidPage {
        /// Largest accepted limit.
        max: u32,
    },
    /// Every attempt of the use case lost a race with another write of the same listing, or
    /// the repository reported a version conflict.
    #[error("listing changed concurrently, try again")]
    Conflict,
    /// An outbound port failed. A failed debit or withdrawal has no effect, and a stack
    /// withdrawn before a failed deposit debit is given back. A failure after the listing was
    /// saved leaves the change saved and skips the effects that were still to run.
    #[error(transparent)]
    Port(#[from] PortError),
    /// The repository failed for a reason other than a version conflict. When storing a new
    /// listing fails, the stack and the deposit already taken from the seller are not given
    /// back.
    #[error("repository error: {0}")]
    Repository(String),
}
