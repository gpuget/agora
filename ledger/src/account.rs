//! The money and items of each player.
//!
//! The aggregate is a [`Decider`]: `decide` turns a command into events without changing the
//! state, `evolve` folds a recorded event into the state. Every movement carries the [`OpId`]
//! of the Agora call, so a replayed call records nothing.

use std::collections::{BTreeMap, BTreeSet};

use agora_domain::{ItemType, Money, OpId, PlayerRef};
use event_sourcing::Decider;

use crate::LedgerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: PlayerRef,
    pub funds: Money,
    /// Only item types held in a quantity greater than zero.
    pub items: BTreeMap<ItemType, u32>,
}

impl Account {
    pub fn new(id: PlayerRef, funds: Money) -> Self {
        Self {
            id,
            funds,
            items: BTreeMap::new(),
        }
    }

    pub fn with_item(mut self, item_type: ItemType, quantity: u32) -> Self {
        if quantity > 0 {
            self.items.insert(item_type, quantity);
        }
        self
    }

    pub fn quantity(&self, item_type: &ItemType) -> u32 {
        self.items.get(item_type).copied().unwrap_or(0)
    }

    pub fn check_withdraw(&self, item_type: &ItemType, quantity: u32) -> Result<(), LedgerError> {
        if self.quantity(item_type) < quantity {
            return Err(LedgerError::ItemUnavailable);
        }
        Ok(())
    }

    pub fn check_deposit(&self, item_type: &ItemType, quantity: u32) -> Result<(), LedgerError> {
        self.quantity(item_type)
            .checked_add(quantity)
            .ok_or(LedgerError::Overflow)?;
        Ok(())
    }

    pub fn check_debit(&self, amount: Money) -> Result<(), LedgerError> {
        self.funds
            .checked_sub(amount)
            .ok_or(LedgerError::InsufficientFunds)?;
        Ok(())
    }

    pub fn check_credit(&self, amount: Money) -> Result<(), LedgerError> {
        self.funds
            .checked_add(amount)
            .ok_or(LedgerError::Overflow)?;
        Ok(())
    }

    /// Whether the movement of `event` is allowed for this account. Changes nothing.
    pub fn check(&self, event: &AccountEvent) -> Result<(), LedgerError> {
        match event {
            AccountEvent::Opened { .. } => Ok(()),
            AccountEvent::Credited { amount, .. } => self.check_credit(*amount),
            AccountEvent::Debited { amount, .. } => self.check_debit(*amount),
            AccountEvent::Deposited {
                item_type,
                quantity,
                ..
            } => self.check_deposit(item_type, *quantity),
            AccountEvent::Withdrawn {
                item_type,
                quantity,
                ..
            } => self.check_withdraw(item_type, *quantity),
        }
    }

    /// Folds one recorded event into the account. Shared by the aggregate and the read model.
    /// Never fails: `decide` checked the event before it was recorded, so the saturating
    /// operations below never saturate.
    pub fn apply(account: Option<Account>, event: &AccountEvent) -> Option<Account> {
        if let AccountEvent::Opened {
            account,
            funds,
            items,
        } = event
        {
            return Some(Account {
                id: account.clone(),
                funds: *funds,
                items: items.clone(),
            });
        }
        let mut account = account?;
        match event {
            AccountEvent::Opened { .. } => {}
            AccountEvent::Credited { amount, .. } => {
                account.funds = Money(account.funds.0.saturating_add(amount.0));
            }
            AccountEvent::Debited { amount, .. } => {
                account.funds = Money(account.funds.0.saturating_sub(amount.0));
            }
            AccountEvent::Deposited {
                item_type,
                quantity,
                ..
            } if *quantity > 0 => {
                let held = account.items.entry(item_type.clone()).or_default();
                *held = held.saturating_add(*quantity);
            }
            AccountEvent::Deposited { .. } => {}
            AccountEvent::Withdrawn {
                item_type,
                quantity,
                ..
            } => {
                let held = account.quantity(item_type).saturating_sub(*quantity);
                if held == 0 {
                    account.items.remove(item_type);
                } else {
                    account.items.insert(item_type.clone(), held);
                }
            }
        }
        Some(account)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountCommand {
    Open {
        funds: Money,
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
pub enum AccountEvent {
    Opened {
        account: PlayerRef,
        funds: Money,
        items: BTreeMap<ItemType, u32>,
    },
    Credited {
        op: OpId,
        amount: Money,
    },
    Debited {
        op: OpId,
        amount: Money,
    },
    Deposited {
        op: OpId,
        item_type: ItemType,
        quantity: u32,
    },
    Withdrawn {
        op: OpId,
        item_type: ItemType,
        quantity: u32,
    },
}

impl AccountEvent {
    /// The Agora call that caused this event, `None` for [`AccountEvent::Opened`].
    pub fn op(&self) -> Option<OpId> {
        match self {
            AccountEvent::Opened { .. } => None,
            AccountEvent::Credited { op, .. }
            | AccountEvent::Debited { op, .. }
            | AccountEvent::Deposited { op, .. }
            | AccountEvent::Withdrawn { op, .. } => Some(*op),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountState {
    /// `None` until the account is opened.
    pub account: Option<Account>,
    pub applied: BTreeSet<OpId>,
}

/// One stream per player, identified by its [`PlayerRef`].
pub struct AccountDecider;

impl Decider for AccountDecider {
    type Id = PlayerRef;
    type Command = AccountCommand;
    type Event = AccountEvent;
    type State = AccountState;
    type Error = LedgerError;

    fn decide(
        id: &PlayerRef,
        command: &AccountCommand,
        state: &AccountState,
    ) -> Result<Vec<AccountEvent>, LedgerError> {
        let event = match command {
            AccountCommand::Open { funds, items } => {
                return match state.account {
                    Some(_) => Err(LedgerError::AccountExists(id.clone())),
                    None => Ok(vec![AccountEvent::Opened {
                        account: id.clone(),
                        funds: *funds,
                        items: items.clone(),
                    }]),
                };
            }
            AccountCommand::Credit { op, amount } => AccountEvent::Credited {
                op: *op,
                amount: *amount,
            },
            AccountCommand::Debit { op, amount } => AccountEvent::Debited {
                op: *op,
                amount: *amount,
            },
            AccountCommand::Deposit {
                op,
                item_type,
                quantity,
            } => AccountEvent::Deposited {
                op: *op,
                item_type: item_type.clone(),
                quantity: *quantity,
            },
            AccountCommand::Withdraw {
                op,
                item_type,
                quantity,
            } => AccountEvent::Withdrawn {
                op: *op,
                item_type: item_type.clone(),
                quantity: *quantity,
            },
        };
        let account = state
            .account
            .as_ref()
            .ok_or_else(|| LedgerError::UnknownAccount(id.clone()))?;
        if event.op().is_some_and(|op| state.applied.contains(&op)) {
            return Ok(Vec::new());
        }
        account.check(&event)?;
        Ok(vec![event])
    }

    fn evolve(mut state: AccountState, event: &AccountEvent) -> AccountState {
        if let Some(op) = event.op() {
            state.applied.insert(op);
        }
        state.account = Account::apply(state.account, event);
        state
    }
}

#[cfg(test)]
mod tests {
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

    /// Folds the events of a decision into the state, as the event store would replay them.
    fn run(state: AccountState, command: AccountCommand) -> Result<AccountState, LedgerError> {
        let events = AccountDecider::decide(&alice(), &command, &state)?;
        Ok(events.iter().fold(state, AccountDecider::evolve))
    }

    fn opened() -> AccountState {
        let open = AccountCommand::Open {
            funds: Money(100),
            items: BTreeMap::from([(cloth(), 5)]),
        };
        run(AccountState::default(), open).unwrap()
    }

    #[test]
    fn withdraw_needs_the_items() {
        let account = Account::new(alice(), Money(100)).with_item(cloth(), 5);
        assert_eq!(account.check_withdraw(&cloth(), 5), Ok(()));
        assert_eq!(
            account.check_withdraw(&cloth(), 6),
            Err(LedgerError::ItemUnavailable)
        );
        assert_eq!(
            account.check_withdraw(&"iron_sword".into(), 1),
            Err(LedgerError::ItemUnavailable)
        );
    }

    #[test]
    fn debit_needs_the_funds() {
        let account = Account::new(alice(), Money(100));
        assert_eq!(account.check_debit(Money(100)), Ok(()));
        assert_eq!(
            account.check_debit(Money(101)),
            Err(LedgerError::InsufficientFunds)
        );
    }

    #[test]
    fn credit_and_deposit_detect_overflow() {
        let account = Account::new(alice(), Money(100)).with_item(cloth(), 5);
        assert_eq!(
            account.check_credit(Money(u64::MAX)),
            Err(LedgerError::Overflow)
        );
        assert_eq!(
            account.check_deposit(&cloth(), u32::MAX),
            Err(LedgerError::Overflow)
        );
    }

    #[test]
    fn movements_need_an_opened_account() {
        let debit = AccountCommand::Debit {
            op: op(1),
            amount: Money(1),
        };
        assert_eq!(
            AccountDecider::decide(&alice(), &debit, &AccountState::default()),
            Err(LedgerError::UnknownAccount(alice()))
        );
    }

    #[test]
    fn open_twice_fails() {
        let open = AccountCommand::Open {
            funds: Money(0),
            items: BTreeMap::new(),
        };
        assert_eq!(
            AccountDecider::decide(&alice(), &open, &opened()),
            Err(LedgerError::AccountExists(alice()))
        );
    }

    #[test]
    fn decide_rejects_without_events() {
        let debit = AccountCommand::Debit {
            op: op(1),
            amount: Money(101),
        };
        assert_eq!(
            AccountDecider::decide(&alice(), &debit, &opened()),
            Err(LedgerError::InsufficientFunds)
        );
    }

    #[test]
    fn replayed_op_records_nothing() {
        let debit = AccountCommand::Debit {
            op: op(1),
            amount: Money(30),
        };
        let state = run(opened(), debit.clone()).unwrap();
        assert_eq!(
            AccountDecider::decide(&alice(), &debit, &state),
            Ok(Vec::new())
        );
        assert_eq!(state.account.unwrap().funds, Money(70));
    }

    #[test]
    fn evolve_moves_funds_and_items() {
        let commands = [
            AccountCommand::Credit {
                op: op(1),
                amount: Money(5),
            },
            AccountCommand::Withdraw {
                op: op(2),
                item_type: cloth(),
                quantity: 5,
            },
            AccountCommand::Deposit {
                op: op(3),
                item_type: cloth(),
                quantity: 2,
            },
        ];
        let state = commands.into_iter().try_fold(opened(), run).unwrap();
        let account = state.account.unwrap();
        assert_eq!(account.funds, Money(105));
        assert_eq!(account.quantity(&cloth()), 2);
    }

    #[test]
    fn apply_removes_empty_stacks() {
        let withdraw = AccountCommand::Withdraw {
            op: op(1),
            item_type: cloth(),
            quantity: 5,
        };
        let account = run(opened(), withdraw).unwrap().account.unwrap();
        assert!(account.items.is_empty());

        let deposit_none = AccountEvent::Deposited {
            op: op(2),
            item_type: cloth(),
            quantity: 0,
        };
        let account = Account::apply(Some(account), &deposit_none).unwrap();
        assert!(account.items.is_empty());
    }
}
