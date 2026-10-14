//! Auction core: implements the inbound ports of `agora-api` on top of the outbound
//! ports of `agora-spi`.

#[cfg(any(test, feature = "memory"))]
pub mod memory;
mod service;

pub use service::AuctionService;

#[cfg(test)]
mod tests;
