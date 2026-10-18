//! Trading card game core: implements the inbound ports of `tcg-api` on top of the outbound
//! ports of `tcg-spi`.

mod caught_up;
mod ledger;
#[cfg(any(test, feature = "memory"))]
pub mod memory;
mod service;

pub use caught_up::CaughtUpView;
pub use ledger::LedgerService;
pub use service::TcgService;

#[cfg(test)]
mod tests;
