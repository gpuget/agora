//! Event-sourced ledger: the gold and items of each player, and the money taken out of the
//! game by the auction houses.
//!
//! Each aggregate is a [`Decider`]: `decide` turns a command into events without changing the
//! state, `evolve` folds a recorded event into the state. Every movement carries the [`OpId`]
//! of the Agora call, so a replayed call records nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use agora_domain::{ItemType, Money, OpId, PlayerRef};
use event_sourcing::Decider;

use crate::{GameError, Player, TakenReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    Open {
        gold: Money,
        items: BTreeMap<ItemType, u32>,
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
        item_type: ItemType,
        quantity: u32,
    },
    Withdraw {
        op: OpId,
        item_type: ItemType,
        quantity: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    Opened {
        player: PlayerRef,
        gold: Money,
        items: BTreeMap<ItemType, u32>,
    },
    GoldCredited {
        op: OpId,
        amount: Money,
    },
    GoldDebited {
        op: OpId,
        amount: Money,
    },
    ItemDeposited {
        op: OpId,
        item_type: ItemType,
        quantity: u32,
    },
    ItemWithdrawn {
        op: OpId,
        item_type: ItemType,
        quantity: u32,
    },
}

impl PlayerEvent {
    /// The Agora call that caused this event, `None` for [`PlayerEvent::Opened`].
    pub fn op(&self) -> Option<OpId> {
        match self {
            PlayerEvent::Opened { .. } => None,
            PlayerEvent::GoldCredited { op, .. }
            | PlayerEvent::GoldDebited { op, .. }
            | PlayerEvent::ItemDeposited { op, .. }
            | PlayerEvent::ItemWithdrawn { op, .. } => Some(*op),
        }
    }
}

impl Player {
    /// Whether the movement of `event` is allowed for this player. Changes nothing.
    pub fn check(&self, event: &PlayerEvent) -> Result<(), GameError> {
        match event {
            PlayerEvent::Opened { .. } => Ok(()),
            PlayerEvent::GoldCredited { amount, .. } => self.check_credit(*amount),
            PlayerEvent::GoldDebited { amount, .. } => self.check_debit(*amount),
            PlayerEvent::ItemDeposited {
                item_type,
                quantity,
                ..
            } => self.check_deposit(item_type, *quantity),
            PlayerEvent::ItemWithdrawn {
                item_type,
                quantity,
                ..
            } => self.check_withdraw(item_type, *quantity),
        }
    }

    /// Folds one recorded event into the player. Shared by the aggregate and the read model.
    /// Never fails: `decide` checked the event before it was recorded, so the saturating
    /// operations below never saturate.
    pub fn apply(player: Option<Player>, event: &PlayerEvent) -> Option<Player> {
        if let PlayerEvent::Opened {
            player,
            gold,
            items,
        } = event
        {
            return Some(Player {
                id: player.clone(),
                gold: *gold,
                items: items.clone(),
            });
        }
        let mut player = player?;
        match event {
            PlayerEvent::Opened { .. } => {}
            PlayerEvent::GoldCredited { amount, .. } => {
                player.gold = Money(player.gold.0.saturating_add(amount.0));
            }
            PlayerEvent::GoldDebited { amount, .. } => {
                player.gold = Money(player.gold.0.saturating_sub(amount.0));
            }
            PlayerEvent::ItemDeposited {
                item_type,
                quantity,
                ..
            } if *quantity > 0 => {
                let held = player.items.entry(item_type.clone()).or_default();
                *held = held.saturating_add(*quantity);
            }
            PlayerEvent::ItemDeposited { .. } => {}
            PlayerEvent::ItemWithdrawn {
                item_type,
                quantity,
                ..
            } => {
                let held = player.quantity(item_type).saturating_sub(*quantity);
                if held == 0 {
                    player.items.remove(item_type);
                } else {
                    player.items.insert(item_type.clone(), held);
                }
            }
        }
        Some(player)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerState {
    /// `None` until the player is opened.
    pub player: Option<Player>,
    pub applied: BTreeSet<OpId>,
}

/// One stream per player, identified by its [`PlayerRef`].
pub struct PlayerDecider;

impl Decider for PlayerDecider {
    type Id = PlayerRef;
    type Command = PlayerCommand;
    type Event = PlayerEvent;
    type State = PlayerState;
    type Error = GameError;

    fn decide(
        id: &PlayerRef,
        command: &PlayerCommand,
        state: &PlayerState,
    ) -> Result<Vec<PlayerEvent>, GameError> {
        let event = match command {
            PlayerCommand::Open { gold, items } => {
                return match state.player {
                    Some(_) => Err(GameError::PlayerExists(id.clone())),
                    None => Ok(vec![PlayerEvent::Opened {
                        player: id.clone(),
                        gold: *gold,
                        items: items.clone(),
                    }]),
                };
            }
            PlayerCommand::Credit { op, amount } => PlayerEvent::GoldCredited {
                op: *op,
                amount: *amount,
            },
            PlayerCommand::Debit { op, amount } => PlayerEvent::GoldDebited {
                op: *op,
                amount: *amount,
            },
            PlayerCommand::Deposit {
                op,
                item_type,
                quantity,
            } => PlayerEvent::ItemDeposited {
                op: *op,
                item_type: item_type.clone(),
                quantity: *quantity,
            },
            PlayerCommand::Withdraw {
                op,
                item_type,
                quantity,
            } => PlayerEvent::ItemWithdrawn {
                op: *op,
                item_type: item_type.clone(),
                quantity: *quantity,
            },
        };
        let player = state
            .player
            .as_ref()
            .ok_or_else(|| GameError::UnknownPlayer(id.clone()))?;
        if event.op().is_some_and(|op| state.applied.contains(&op)) {
            return Ok(Vec::new());
        }
        player.check(&event)?;
        Ok(vec![event])
    }

    fn evolve(mut state: PlayerState, event: &PlayerEvent) -> PlayerState {
        if let Some(op) = event.op() {
            state.applied.insert(op);
        }
        state.player = Player::apply(state.player, event);
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
    TakeMoney {
        op: OpId,
        amount: Money,
        reason: TakenReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreasuryEvent {
    MoneyTaken {
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
    type Error = GameError;

    fn decide(
        _id: &TreasuryId,
        command: &TreasuryCommand,
        state: &TreasuryState,
    ) -> Result<Vec<TreasuryEvent>, GameError> {
        let TreasuryCommand::TakeMoney { op, amount, reason } = *command;
        if state.applied.contains(&op) {
            return Ok(Vec::new());
        }
        state.total.checked_add(amount).ok_or(GameError::Overflow)?;
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

    fn alice() -> PlayerRef {
        "alice".into()
    }

    fn cloth() -> ItemType {
        "linen_cloth".into()
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
    fn run(state: PlayerState, command: PlayerCommand) -> Result<PlayerState, GameError> {
        let events = PlayerDecider::decide(&alice(), &command, &state)?;
        Ok(events.iter().fold(state, PlayerDecider::evolve))
    }

    fn opened() -> PlayerState {
        let open = PlayerCommand::Open {
            gold: Money(100),
            items: BTreeMap::from([(cloth(), 5)]),
        };
        run(PlayerState::default(), open).unwrap()
    }

    #[test]
    fn movements_need_an_opened_player() {
        let debit = PlayerCommand::Debit {
            op: op(1),
            amount: Money(1),
        };
        assert_eq!(
            PlayerDecider::decide(&alice(), &debit, &PlayerState::default()),
            Err(GameError::UnknownPlayer(alice()))
        );
    }

    #[test]
    fn open_twice_fails() {
        let open = PlayerCommand::Open {
            gold: Money(0),
            items: BTreeMap::new(),
        };
        assert_eq!(
            PlayerDecider::decide(&alice(), &open, &opened()),
            Err(GameError::PlayerExists(alice()))
        );
    }

    #[test]
    fn decide_rejects_without_events() {
        let debit = PlayerCommand::Debit {
            op: op(1),
            amount: Money(101),
        };
        assert_eq!(
            PlayerDecider::decide(&alice(), &debit, &opened()),
            Err(GameError::InsufficientFunds)
        );
    }

    #[test]
    fn replayed_op_records_nothing() {
        let debit = PlayerCommand::Debit {
            op: op(1),
            amount: Money(30),
        };
        let state = run(opened(), debit.clone()).unwrap();
        assert_eq!(
            PlayerDecider::decide(&alice(), &debit, &state),
            Ok(Vec::new())
        );
        assert_eq!(state.player.unwrap().gold, Money(70));
    }

    #[test]
    fn evolve_moves_gold_and_items() {
        let commands = [
            PlayerCommand::Credit {
                op: op(1),
                amount: Money(5),
            },
            PlayerCommand::Withdraw {
                op: op(2),
                item_type: cloth(),
                quantity: 5,
            },
            PlayerCommand::Deposit {
                op: op(3),
                item_type: cloth(),
                quantity: 2,
            },
        ];
        let state = commands.into_iter().try_fold(opened(), run).unwrap();
        let player = state.player.unwrap();
        assert_eq!(player.gold, Money(105));
        assert_eq!(player.quantity(&cloth()), 2);
    }

    #[test]
    fn apply_removes_empty_stacks() {
        let withdraw = PlayerCommand::Withdraw {
            op: op(1),
            item_type: cloth(),
            quantity: 5,
        };
        let player = run(opened(), withdraw).unwrap().player.unwrap();
        assert!(player.items.is_empty());

        let deposit_none = PlayerEvent::ItemDeposited {
            op: op(2),
            item_type: cloth(),
            quantity: 0,
        };
        let player = Player::apply(Some(player), &deposit_none).unwrap();
        assert!(player.items.is_empty());
    }

    #[test]
    fn treasury_takes_each_op_once() {
        let take = TreasuryCommand::TakeMoney {
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
