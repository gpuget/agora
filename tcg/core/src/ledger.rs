use agora_domain::{Money, OpId, PlayerRef};
use event_sourcing::{EventStore, execute};
use tcg_api::LedgerApi;
use tcg_domain::{
    CardId, Collector, CollectorCommand, CollectorDecider, CollectorEvent, TakenReason, TcgError,
    TreasuryCommand, TreasuryDecider, TreasuryEvent, TreasuryId,
};
use tracing::{debug, info, warn};

/// Applies the dust and card movements requested by the auction house, as events.
#[derive(Clone)]
pub struct LedgerService<C, T> {
    collectors: C,
    treasury: T,
}

impl<C, T> LedgerService<C, T>
where
    C: EventStore<PlayerRef, CollectorEvent>,
    T: EventStore<TreasuryId, TreasuryEvent>,
{
    pub fn new(collectors: C, treasury: T) -> Self {
        Self {
            collectors,
            treasury,
        }
    }

    /// Runs one movement on a collector, then logs its result.
    async fn movement(
        &self,
        call: &'static str,
        op: OpId,
        collector: &PlayerRef,
        command: CollectorCommand,
        detail: String,
    ) -> Result<(), TcgError> {
        let result = execute::<CollectorDecider, _>(&self.collectors, collector, &command).await;
        match &result {
            Ok(events) if events.is_empty() => {
                debug!(call, %op, %collector, %detail, "ledger call already applied")
            }
            Ok(_) => debug!(call, %op, %collector, %detail, "ledger call applied"),
            Err(error) => warn!(call, %op, %collector, %detail, %error, "ledger call failed"),
        }
        result.map(|_| ())
    }
}

impl<C, T> LedgerApi for LedgerService<C, T>
where
    C: EventStore<PlayerRef, CollectorEvent>,
    T: EventStore<TreasuryId, TreasuryEvent>,
{
    async fn open_collector(&self, collector: Collector) -> Result<(), TcgError> {
        let command = CollectorCommand::Open {
            dust: collector.dust,
            cards: collector.cards,
        };
        execute::<CollectorDecider, _>(&self.collectors, &collector.id, &command).await?;
        info!(collector = %collector.id, "collector opened");
        Ok(())
    }

    async fn withdraw_card(
        &self,
        op: OpId,
        collector: &PlayerRef,
        card: &CardId,
        copies: u32,
    ) -> Result<(), TcgError> {
        let command = CollectorCommand::Withdraw {
            op,
            card: card.clone(),
            copies,
        };
        let detail = format!("{card} x{copies}");
        self.movement("withdraw_card", op, collector, command, detail)
            .await
    }

    async fn deposit_card(
        &self,
        op: OpId,
        collector: &PlayerRef,
        card: &CardId,
        copies: u32,
    ) -> Result<(), TcgError> {
        let command = CollectorCommand::Deposit {
            op,
            card: card.clone(),
            copies,
        };
        let detail = format!("{card} x{copies}");
        self.movement("deposit_card", op, collector, command, detail)
            .await
    }

    async fn debit(&self, op: OpId, collector: &PlayerRef, amount: Money) -> Result<(), TcgError> {
        let command = CollectorCommand::Debit { op, amount };
        self.movement("debit", op, collector, command, amount.0.to_string())
            .await
    }

    async fn credit(&self, op: OpId, collector: &PlayerRef, amount: Money) -> Result<(), TcgError> {
        let command = CollectorCommand::Credit { op, amount };
        self.movement("credit", op, collector, command, amount.0.to_string())
            .await
    }

    async fn take_dust(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> Result<(), TcgError> {
        let command = TreasuryCommand::TakeDust { op, amount, reason };
        let events = execute::<TreasuryDecider, _>(&self.treasury, &TreasuryId, &command).await?;
        if events.is_empty() {
            debug!(%op, amount = amount.0, ?reason, "dust already taken");
        } else {
            info!(%op, amount = amount.0, ?reason, "dust taken by the auction house");
        }
        Ok(())
    }
}
