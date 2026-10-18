//! Game core: implements the inbound ports of `game-api` on top of the outbound ports of
//! `game-spi`.

mod caught_up;
mod ledger;
#[cfg(any(test, feature = "memory"))]
pub mod memory;
mod service;

pub use caught_up::CaughtUpView;
pub use ledger::LedgerService;
pub use service::GameService;

#[cfg(test)]
mod tests;
