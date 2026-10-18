//! Event-sourced ledger: the dust and cards of each collector, and the dust taken out of the
//! game by the auction houses.
//!
//! Each aggregate is a [`Decider`]: `decide` turns a command into events without changing the
//! state, `evolve` folds a recorded event into the state. Every movement carries the [`OpId`]
//! of the Agora call, so a replayed call records nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use agora_domain::{Money, OpId, PlayerRef};
use event_sourcing::Decider;

use crate::{CardId, Collector, TakenReason, TcgError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectorCommand {
    Open {
        dust: Money,
        cards: BTreeMap<CardId, u32>,
    },
    Credit {
        op: OpId,
        amount: Money,
    },
    Debit {
        op: OpId,
        amount: Money,
    },
    Deposit {
        op: OpId,
        card: CardId,
        copies: u32,
    },
    Withdraw {
        op: OpId,
        card: CardId,
        copies: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectorEvent {
    Opened {
        collector: PlayerRef,
        dust: Money,
        cards: BTreeMap<CardId, u32>,
    },
    DustCredited {
        op: OpId,
        amount: Money,
    },
    DustDebited {
        op: OpId,
        amount: Money,
    },
    CardDeposited {
        op: OpId,
        card: CardId,
        copies: u32,
    },
    CardWithdrawn {
        op: OpId,
        card: CardId,
        copies: u32,
    },
}

impl CollectorEvent {
    /// The Agora call that caused this event, `None` for [`CollectorEvent::Opened`].
    pub fn op(&self) -> Option<OpId> {
        match self {
            CollectorEvent::Opened { .. } => None,
            CollectorEvent::DustCredited { op, .. }
            | CollectorEvent::DustDebited { op, .. }
            | CollectorEvent::CardDeposited { op, .. }
            | CollectorEvent::CardWithdrawn { op, .. } => Some(*op),
        }
    }
}

impl Collector {
    /// Whether the movement of `event` is allowed for this collector. Changes nothing.
    pub fn check(&self, event: &CollectorEvent) -> Result<(), TcgError> {
        match event {
            CollectorEvent::Opened { .. } => Ok(()),
            CollectorEvent::DustCredited { amount, .. } => self.check_credit(*amount),
            CollectorEvent::DustDebited { amount, .. } => self.check_debit(*amount),
            CollectorEvent::CardDeposited { card, copies, .. } => self.check_deposit(card, *copies),
            CollectorEvent::CardWithdrawn { card, copies, .. } => {
                self.check_withdraw(card, *copies)
            }
        }
    }

    /// Folds one recorded event into the collector. Shared by the aggregate and the read
    /// model. Never fails: `decide` checked the event before it was recorded, so the
    /// saturating operations below never saturate.
    pub fn apply(collector: Option<Collector>, event: &CollectorEvent) -> Option<Collector> {
        if let CollectorEvent::Opened {
            collector,
            dust,
            cards,
        } = event
        {
            return Some(Collector {
                id: collector.clone(),
                dust: *dust,
                cards: cards.clone(),
            });
        }
        let mut collector = collector?;
        match event {
            CollectorEvent::Opened { .. } => {}
            CollectorEvent::DustCredited { amount, .. } => {
                collector.dust = Money(collector.dust.0.saturating_add(amount.0));
            }
            CollectorEvent::DustDebited { amount, .. } => {
                collector.dust = Money(collector.dust.0.saturating_sub(amount.0));
            }
            CollectorEvent::CardDeposited { card, copies, .. } if *copies > 0 => {
                let held = collector.cards.entry(card.clone()).or_default();
                *held = held.saturating_add(*copies);
            }
            CollectorEvent::CardDeposited { .. } => {}
            CollectorEvent::CardWithdrawn { card, copies, .. } => {
                let held = collector.copies(card).saturating_sub(*copies);
                if held == 0 {
                    collector.cards.remove(card);
                } else {
                    collector.cards.insert(card.clone(), held);
                }
            }
        }
        Some(collector)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectorState {
    /// `None` until the collector is opened.
    pub collector: Option<Collector>,
    pub applied: BTreeSet<OpId>,
}

/// One stream per collector, identified by its [`PlayerRef`].
pub struct CollectorDecider;

impl Decider for CollectorDecider {
    type Id = PlayerRef;
    type Command = CollectorCommand;
    type Event = CollectorEvent;
    type State = CollectorState;
    type Error = TcgError;

    fn decide(
        id: &PlayerRef,
        command: &CollectorCommand,
        state: &CollectorState,
    ) -> Result<Vec<CollectorEvent>, TcgError> {
        let event = match command {
            CollectorCommand::Open { dust, cards } => {
                return match state.collector {
                    Some(_) => Err(TcgError::CollectorExists(id.clone())),
                    None => Ok(vec![CollectorEvent::Opened {
                        collector: id.clone(),
                        dust: *dust,
                        cards: cards.clone(),
                    }]),
                };
            }
            CollectorCommand::Credit { op, amount } => CollectorEvent::DustCredited {
                op: *op,
                amount: *amount,
            },
            CollectorCommand::Debit { op, amount } => CollectorEvent::DustDebited {
                op: *op,
                amount: *amount,
            },
            CollectorCommand::Deposit { op, card, copies } => CollectorEvent::CardDeposited {
                op: *op,
                card: card.clone(),
                copies: *copies,
            },
            CollectorCommand::Withdraw { op, card, copies } => CollectorEvent::CardWithdrawn {
                op: *op,
                card: card.clone(),
                copies: *copies,
            },
        };
        let collector = state
            .collector
            .as_ref()
            .ok_or_else(|| TcgError::UnknownCollector(id.clone()))?;
        if event.op().is_some_and(|op| state.applied.contains(&op)) {
            return Ok(Vec::new());
        }
        collector.check(&event)?;
        Ok(vec![event])
    }

    fn evolve(mut state: CollectorState, event: &CollectorEvent) -> CollectorState {
        if let Some(op) = event.op() {
            state.applied.insert(op);
        }
        state.collector = Collector::apply(state.collector, event);
        state
    }
}

/// Identifies the single treasury stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TreasuryId;

impl fmt::Display for TreasuryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("treasury")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryCommand {
    TakeDust {
        op: OpId,
        amount: Money,
        reason: TakenReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryEvent {
    DustTaken {
        op: OpId,
        amount: Money,
        reason: TakenReason,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreasuryState {
    pub total: Money,
    pub applied: BTreeSet<OpId>,
}

pub struct TreasuryDecider;

impl Decider for TreasuryDecider {
    type Id = TreasuryId;
    type Command = TreasuryCommand;
    type Event = TreasuryEvent;
    type State = TreasuryState;
    type Error = TcgError;

    fn decide(
        _id: &TreasuryId,
        command: &TreasuryCommand,
        state: &TreasuryState,
    ) -> Result<Vec<TreasuryEvent>, TcgError> {
        let TreasuryCommand::TakeDust { op, amount, reason } = *command;
        if state.applied.contains(&op) {
            return Ok(Vec::new());
        }
        state.total.checked_add(amount).ok_or(TcgError::Overflow)?;
        Ok(vec![TreasuryEvent::DustTaken { op, amount, reason }])
    }

    fn evolve(mut state: TreasuryState, event: &TreasuryEvent) -> TreasuryState {
        let TreasuryEvent::DustTaken { op, amount, .. } = *event;
        state.applied.insert(op);
        state.total = Money(state.total.0.saturating_add(amount.0));
        state
    }
}

#[cfg(test)]
mod tests {
    use agora_domain::ListingId;

    use super::*;

    fn alice() -> PlayerRef {
        "alice".into()
    }

    fn drake() -> CardId {
        "fire_drake".into()
    }

    fn op(n: u128) -> OpId {
        OpId(uuid::Uuid::from_u128(n))
    }

    fn commission() -> TakenReason {
        TakenReason::Commission {
            listing: ListingId(Default::default()),
        }
    }

    /// Folds the events of a decision into the state, as the event store would replay them.
    fn run(state: CollectorState, command: CollectorCommand) -> Result<CollectorState, TcgError> {
        let events = CollectorDecider::decide(&alice(), &command, &state)?;
        Ok(events.iter().fold(state, CollectorDecider::evolve))
    }

    fn opened() -> CollectorState {
        let open = CollectorCommand::Open {
            dust: Money(100),
            cards: BTreeMap::from([(drake(), 2)]),
        };
        run(CollectorState::default(), open).unwrap()
    }

    #[test]
    fn movements_need_an_opened_collector() {
        let debit = CollectorCommand::Debit {
            op: op(1),
            amount: Money(1),
        };
        assert_eq!(
            CollectorDecider::decide(&alice(), &debit, &CollectorState::default()),
            Err(TcgError::UnknownCollector(alice()))
        );
    }

    #[test]
    fn open_twice_fails() {
        let open = CollectorCommand::Open {
            dust: Money(0),
            cards: BTreeMap::new(),
        };
        assert_eq!(
            CollectorDecider::decide(&alice(), &open, &opened()),
            Err(TcgError::CollectorExists(alice()))
        );
    }

    #[test]
    fn decide_rejects_without_events() {
        let debit = CollectorCommand::Debit {
            op: op(1),
            amount: Money(101),
        };
        assert_eq!(
            CollectorDecider::decide(&alice(), &debit, &opened()),
            Err(TcgError::InsufficientDust)
        );
    }

    #[test]
    fn replayed_op_records_nothing() {
        let debit = CollectorCommand::Debit {
            op: op(1),
            amount: Money(30),
        };
        let state = run(opened(), debit.clone()).unwrap();
        assert_eq!(
            CollectorDecider::decide(&alice(), &debit, &state),
            Ok(Vec::new())
        );
        assert_eq!(state.collector.unwrap().dust, Money(70));
    }

    #[test]
    fn evolve_moves_dust_and_cards() {
        let commands = [
            CollectorCommand::Credit {
                op: op(1),
                amount: Money(5),
            },
            CollectorCommand::Withdraw {
                op: op(2),
                card: drake(),
                copies: 2,
            },
            CollectorCommand::Deposit {
                op: op(3),
                card: drake(),
                copies: 1,
            },
        ];
        let state = commands.into_iter().try_fold(opened(), run).unwrap();
        let collector = state.collector.unwrap();
        assert_eq!(collector.dust, Money(105));
        assert_eq!(collector.copies(&drake()), 1);
    }

    #[test]
    fn apply_removes_cards_without_copies() {
        let withdraw = CollectorCommand::Withdraw {
            op: op(1),
            card: drake(),
            copies: 2,
        };
        let collector = run(opened(), withdraw).unwrap().collector.unwrap();
        assert!(collector.cards.is_empty());

        let deposit_none = CollectorEvent::CardDeposited {
            op: op(2),
            card: drake(),
            copies: 0,
        };
        let collector = Collector::apply(Some(collector), &deposit_none).unwrap();
        assert!(collector.cards.is_empty());
    }

    #[test]
    fn treasury_takes_each_op_once() {
        let take = TreasuryCommand::TakeDust {
            op: op(1),
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
