use serde::{Deserialize, Serialize};

/// Amount in the smallest unit of the game currency.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Money(pub u64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub fn checked_add(self, other: Money) -> Option<Money> {
        self.0.checked_add(other.0).map(Money)
    }

    pub fn checked_sub(self, other: Money) -> Option<Money> {
        self.0.checked_sub(other.0).map(Money)
    }

    pub fn checked_mul(self, factor: u64) -> Option<Money> {
        self.0.checked_mul(factor).map(Money)
    }

    /// `self * bps / 10_000`, rounded down. Saturates at `u64::MAX`.
    pub fn apply_bps(self, bps: Bps) -> Money {
        let value = u128::from(self.0) * u128::from(bps.0) / 10_000;
        Money(u64::try_from(value).unwrap_or(u64::MAX))
    }
}

/// Rate in basis points: `500` is 5 %.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Bps(pub u32);

impl Bps {
    pub const MAX: Bps = Bps(10_000);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_bps_rounds_down() {
        assert_eq!(Money(199).apply_bps(Bps(500)), Money(9));
        assert_eq!(Money(200).apply_bps(Bps(500)), Money(10));
    }

    #[test]
    fn apply_bps_does_not_overflow() {
        assert_eq!(Money(u64::MAX).apply_bps(Bps::MAX), Money(u64::MAX));
    }
}
