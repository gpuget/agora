use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{AuctionError, Bps, HouseId, Money};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationOption {
    pub duration: Duration,
    pub deposit_multiplier: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseConfig {
    pub name: String,
    /// Taken from the final price of a sale.
    pub commission: Bps,
    /// Absolute part of the minimum bid increment, per unit. At least 1.
    pub min_increment: Money,
    /// Relative part of the minimum bid increment.
    pub increment: Bps,
    /// Deposit rate, applied to the total base value of the stack.
    pub deposit: Bps,
    /// Allowed listing durations.
    pub durations: Vec<DurationOption>,
}

impl HouseConfig {
    pub fn validate(&self) -> Result<(), AuctionError> {
        if self.commission > Bps::MAX {
            return Err(AuctionError::InvalidConfig("commission above 100%"));
        }
        if self.min_increment == Money::ZERO {
            return Err(AuctionError::InvalidConfig(
                "min_increment must be at least 1",
            ));
        }
        if self.durations.is_empty() {
            return Err(AuctionError::InvalidConfig(
                "at least one duration is required",
            ));
        }
        if self.durations.iter().any(|o| o.duration.is_zero()) {
            return Err(AuctionError::InvalidConfig(
                "durations must be greater than zero",
            ));
        }
        Ok(())
    }

    pub fn duration_option(&self, duration: Duration) -> Option<&DurationOption> {
        self.durations.iter().find(|o| o.duration == duration)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionHouse {
    pub id: HouseId,
    pub config: HouseConfig,
}
