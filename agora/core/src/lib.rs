//! Auction core: implements the inbound ports of `agora-api` on top of the outbound
//! ports of `agora-spi`.
//!
//! The application layer of the Agora hexagon. [`AuctionService`] implements
//! [`AuctionApi`](agora_api::AuctionApi) and owns no state: listings live in the
//! [`AuctionRepository`](agora_spi::AuctionRepository), money and items in the game behind
//! the other ports.
//!
//! Feature `memory`: enables the `memory` module, in-memory adapters for every outbound port,
//! for tests and demos.
//!
//! Business rules: see `docs/functional.md`.

#[cfg(any(test, feature = "memory"))]
pub mod memory;
mod service;

pub use service::AuctionService;

#[cfg(test)]
mod tests;
