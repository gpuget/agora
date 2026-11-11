use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{AuctionError, Bps, HouseId, Money};

/// A listing duration offered by a house, with its weight in the deposit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationOption {
    /// Time from creation to the end of the listing. Greater than zero.
    pub duration: Duration,
    /// Factor applied to the deposit of a listing with this duration. `0` means no deposit.
    pub deposit_multiplier: u32,
}

/// Settings of an auction house.
///
/// A listing copies `commission`, `min_increment` and `increment` at creation, so later
/// changes do not affect it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseConfig {
    pub name: String,
    /// Taken from the final price of a sale.
    ///
    /// Applied to the total price of each sale, rounded down, and sent to the sink.
    pub commission: Bps,
    /// Absolute part of the minimum bid increment, per unit. At least 1.
    pub min_increment: Money,
    /// Relative part of the minimum bid increment.
    ///
    /// Applied to the unit price of the high bid, rounded down. The increment is the larger of
    /// this part and `min_increment`.
    pub increment: Bps,
    /// Deposit rate, applied to the total base value of the stack.
    ///
    /// The deposit is `base_value * quantity * deposit`, rounded down, then multiplied by the
    /// [`DurationOption::deposit_multiplier`] of the chosen duration.
    pub deposit: Bps,
    /// Allowed listing durations.
    pub durations: Vec<DurationOption>,
}

impl HouseConfig {
    /// Checks the config before a house is created.
    ///
    /// # Errors
    ///
    /// [`AuctionError::InvalidConfig`] when the commission is above 100 %, `min_increment` is
    /// zero, `durations` is empty or holds a zero duration. `increment` and `deposit` are not
    /// bounded.
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

    /// The option whose duration equals `duration` exactly, `None` if the house does not
    /// offer it.
    pub fn duration_option(&self, duration: Duration) -> Option<&DurationOption> {
        self.durations.iter().find(|o| o.duration == duration)
    }
}

/// An auction house: a market with its own config and listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionHouse {
    pub id: HouseId,
    pub config: HouseConfig,
}
