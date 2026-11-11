//! The money taken out of the game by the auction houses.

use std::collections::BTreeSet;
use std::fmt;

use agora_domain::{Money, OpId};
use event_sourcing::Decider;

use crate::{LedgerError, TakenReason};

/// Identifies the single treasury stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TreasuryId;

impl fmt::Display for TreasuryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("treasury")
    }
}

/// Command of [`TreasuryDecider`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryCommand {
    /// Records `amount` as taken out of the game. Fails with [`LedgerError::Overflow`] if
    /// the total would exceed `u64::MAX`.
    TakeMoney {
        op: OpId,
        amount: Money,
        reason: TakenReason,
    },
}

/// Event of the treasury stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryEvent {
    /// `amount` left the game for `reason`.
    MoneyTaken {
        op: OpId,
        amount: Money,
        reason: TakenReason,
    },
}

/// State folded from the treasury stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreasuryState {
    /// Sum of every [`TreasuryEvent::MoneyTaken`] amount.
    pub total: Money,
    /// Operation ids already recorded. A command with one of them records nothing.
    pub applied: BTreeSet<OpId>,
}

/// The treasury aggregate: the money taken out of the game by every auction house. Its
/// single stream is identified by [`TreasuryId`].
///
/// A command whose [`OpId`] is already applied records nothing.
pub struct TreasuryDecider;

impl Decider for TreasuryDecider {
    type Id = TreasuryId;
    type Command = TreasuryCommand;
    type Event = TreasuryEvent;
    type State = TreasuryState;
    type Error = LedgerError;

    fn decide(
        _id: &TreasuryId,
        command: &TreasuryCommand,
        state: &TreasuryState,
    ) -> Result<Vec<TreasuryEvent>, LedgerError> {
        let TreasuryCommand::TakeMoney { op, amount, reason } = *command;
        if state.applied.contains(&op) {
            return Ok(Vec::new());
        }
        state
            .total
            .checked_add(amount)
            .ok_or(LedgerError::Overflow)?;
        Ok(vec![TreasuryEvent::MoneyTaken { op, amount, reason }])
    }

    fn evolve(mut state: TreasuryState, event: &TreasuryEvent) -> TreasuryState {
        let TreasuryEvent::MoneyTaken { op, amount, .. } = *event;
        state.applied.insert(op);
        state.total = Money(state.total.0.saturating_add(amount.0));
        state
    }
}

#[cfg(test)]
mod tests {
    use agora_domain::ListingId;

    use super::*;

    fn commission() -> TakenReason {
        TakenReason::Commission {
            listing: ListingId(Default::default()),
        }
    }

    #[test]
    fn treasury_takes_each_op_once() {
        let take = TreasuryCommand::TakeMoney {
            op: OpId(uuid::Uuid::from_u128(1)),
            amount: Money(7),
            reason: commission(),
        };
        let events =
            TreasuryDecider::decide(&TreasuryId, &take, &TreasuryState::default()).unwrap();
        let state = events
            .iter()
            .fold(TreasuryState::default(), TreasuryDecider::evolve);
        assert_eq!(state.total, Money(7));
        assert_eq!(
            TreasuryDecider::decide(&TreasuryId, &take, &state),
            Ok(Vec::new())
        );
    }
}
