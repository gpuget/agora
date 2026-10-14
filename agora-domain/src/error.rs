use thiserror::Error;

use crate::Money;

/// Failure reported by an outbound port implemented by the game.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortError {
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("item unavailable")]
    ItemUnavailable,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuctionError {
    #[error("invalid auction house config: {0}")]
    InvalidConfig(&'static str),
    #[error("auction house not found")]
    HouseNotFound,
    #[error("listing not found")]
    ListingNotFound,
    #[error("quantity must be greater than zero")]
    InvalidQuantity,
    #[error("start price must be greater than zero")]
    InvalidPrice,
    #[error("buyout price is below start price")]
    BuyoutBelowStart,
    #[error("duration not allowed by this auction house")]
    DurationNotAllowed,
    #[error("amount overflow")]
    Overflow,
    #[error("listing is not active")]
    NotActive,
    #[error("listing has expired")]
    Expired,
    #[error("seller cannot bid on or buy their own listing")]
    SellerCannotBuy,
    #[error("already the highest bidder")]
    AlreadyHighBidder,
    #[error("bid too low, minimum unit price is {}", minimum.0)]
    BidTooLow { minimum: Money },
    #[error("only the seller can cancel this listing")]
    NotSeller,
    #[error("listing has bids and cannot be cancelled")]
    HasBids,
    #[error("listing has no buyout price")]
    NoBuyout,
    #[error("page limit must be between 1 and {max}")]
    InvalidPage { max: u32 },
    #[error("listing changed concurrently, try again")]
    Conflict,
    #[error(transparent)]
    Port(#[from] PortError),
    #[error("repository error: {0}")]
    Repository(String),
}
