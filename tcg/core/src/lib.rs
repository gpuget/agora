//! Trading card game core: implements the inbound ports of `tcg-api` on top of the outbound
//! ports of `tcg-spi`.
//!
//! [`TcgService`] forwards the market use cases to Agora and reads collectors, catalog,
//! houses and treasury through the game ports. [`CaughtUpView`] adapts a ledger projection
//! so that reads never lag. The `memory` feature adds the `memory` module: in-memory
//! adapters of the outbound ports.

mod caught_up;
#[cfg(any(test, feature = "memory"))]
pub mod memory;
mod service;

pub use caught_up::CaughtUpView;
pub use service::TcgService;

#[cfg(test)]
mod tests;
