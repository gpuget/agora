use agora_domain::{ItemType, Money, OpId, PlayerRef};
use event_sourcing::{EventStore, execute};
use tracing::{debug, info, warn};

use crate::{
    Account, AccountCommand, AccountDecider, AccountEvent, LedgerApi, LedgerError, TakenReason,
    TreasuryCommand, TreasuryDecider, TreasuryEvent, TreasuryId,
};

/// Applies the money and item movements requested by the auction house, as events.
///
/// Implements [`LedgerApi`]. Account events go to store `A`, one stream per player. Treasury
/// events go to store `T`, in the single [`TreasuryId`] stream. Each call runs
/// [`event_sourcing::execute`]: load the stream, decide, append, retry on a concurrent
/// append.
#[derive(Clone)]
pub struct LedgerService<A, T> {
    accounts: A,
    treasury: T,
}

impl<A, T> LedgerService<A, T>
where
    A: EventStore<PlayerRef, AccountEvent>,
    T: EventStore<TreasuryId, TreasuryEvent>,
{
    /// A ledger that records account events in `accounts` and treasury events in
    /// `treasury`. Accounts must be opened with [`LedgerApi::open_account`] before use.
    pub fn new(accounts: A, treasury: T) -> Self {
        Self { accounts, treasury }
    }

    /// Runs one movement on an account, then logs its result.
    async fn movement(
        &self,
        call: &'static str,
        op: OpId,
        player: &PlayerRef,
        command: AccountCommand,
        detail: String,
    ) -> Result<(), LedgerError> {
        let result = execute::<AccountDecider, _>(&self.accounts, player, &command).await;
        match &result {
            Ok(events) if events.is_empty() => {
                debug!(call, %op, %player, %detail, "ledger call already applied")
            }
            Ok(_) => debug!(call, %op, %player, %detail, "ledger call applied"),
            Err(error) => warn!(call, %op, %player, %detail, %error, "ledger call failed"),
        }
        result.map(|_| ())
    }
}

impl<A, T> LedgerApi for LedgerService<A, T>
where
    A: EventStore<PlayerRef, AccountEvent>,
    T: EventStore<TreasuryId, TreasuryEvent>,
{
    async fn open_account(&self, account: Account) -> Result<(), LedgerError> {
        let command = AccountCommand::Open {
            funds: account.funds,
            items: account.items,
        };
        execute::<AccountDecider, _>(&self.accounts, &account.id, &command).await?;
        info!(player = %account.id, "account opened");
        Ok(())
    }

    async fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> Result<(), LedgerError> {
        let command = AccountCommand::Withdraw {
            op,
            item_type: item_type.clone(),
            quantity,
        };
        let detail = format!("{item_type} x{quantity}");
        self.movement("withdraw_item", op, player, command, detail)
            .await
    }

    async fn deposit_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> Result<(), LedgerError> {
        let command = AccountCommand::Deposit {
            op,
            item_type: item_type.clone(),
            quantity,
        };
        let detail = format!("{item_type} x{quantity}");
        self.movement("deposit_item", op, player, command, detail)
            .await
    }

    async fn debit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), LedgerError> {
        let command = AccountCommand::Debit { op, amount };
        self.movement("debit", op, player, command, amount.0.to_string())
            .await
    }

    async fn credit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), LedgerError> {
        let command = AccountCommand::Credit { op, amount };
        self.movement("credit", op, player, command, amount.0.to_string())
            .await
    }

    async fn take_money(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> Result<(), LedgerError> {
        let command = TreasuryCommand::TakeMoney { op, amount, reason };
        let events = execute::<TreasuryDecider, _>(&self.treasury, &TreasuryId, &command).await?;
        if events.is_empty() {
            debug!(%op, amount = amount.0, ?reason, "money already taken");
        } else {
            info!(%op, amount = amount.0, ?reason, "money taken by the auction house");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use event_sourcing::memory::InMemoryEventStore;
    use event_sourcing::{Recorded, StoreError};

    use super::*;

    type AccountEvents = InMemoryEventStore<PlayerRef, AccountEvent>;
    type TreasuryEvents = InMemoryEventStore<TreasuryId, TreasuryEvent>;

    fn alice() -> PlayerRef {
        "alice".into()
    }

    fn op(n: u128) -> OpId {
        OpId(uuid::Uuid::from_u128(n))
    }

    /// The ledger with Alice opened (100 funds).
    async fn ledger() -> LedgerService<AccountEvents, TreasuryEvents> {
        let ledger = LedgerService::new(AccountEvents::default(), TreasuryEvents::default());
        ledger
            .open_account(Account::new(alice(), Money(100)))
            .await
            .unwrap();
        ledger
    }

    #[tokio::test]
    async fn failed_call_is_reported() {
        let ledger = ledger().await;
        let err = ledger.debit(op(1), &alice(), Money(101)).await;
        assert_eq!(err, Err(LedgerError::InsufficientFunds));

        let err = ledger.debit(op(2), &"nobody".into(), Money(1)).await;
        assert_eq!(err, Err(LedgerError::UnknownAccount("nobody".into())));
    }

    #[tokio::test]
    async fn opening_an_account_twice_fails() {
        let ledger = ledger().await;
        let err = ledger.open_account(Account::new(alice(), Money(0))).await;
        assert_eq!(err, Err(LedgerError::AccountExists(alice())));
    }

    /// Fails the first append with a conflict, as if another write happened in between.
    struct ConflictOnce<S> {
        inner: S,
        conflicted: AtomicBool,
    }

    impl<S: EventStore<PlayerRef, AccountEvent>> EventStore<PlayerRef, AccountEvent>
        for ConflictOnce<S>
    {
        async fn load(&self, stream: &PlayerRef) -> Result<Vec<AccountEvent>, StoreError> {
            self.inner.load(stream).await
        }

        async fn append(
            &self,
            stream: &PlayerRef,
            expected: u64,
            events: Vec<AccountEvent>,
        ) -> Result<u64, StoreError> {
            if !self.conflicted.swap(true, Ordering::SeqCst) {
                return Err(StoreError::Conflict);
            }
            self.inner.append(stream, expected, events).await
        }

        async fn read_all(
            &self,
            after: u64,
            limit: usize,
        ) -> Result<Vec<Recorded<PlayerRef, AccountEvent>>, StoreError> {
            self.inner.read_all(after, limit).await
        }
    }

    #[tokio::test]
    async fn call_retries_on_conflict() {
        let account_events = AccountEvents::default();
        let store = ConflictOnce {
            inner: account_events.clone(),
            conflicted: AtomicBool::new(false),
        };
        let ledger = LedgerService::new(store, TreasuryEvents::default());
        ledger
            .open_account(Account::new(alice(), Money(100)))
            .await
            .unwrap();
        assert_eq!(account_events.load(&alice()).await.unwrap().len(), 1);
    }
}
