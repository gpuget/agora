use agora_domain::{ItemType, Money, OpId, PlayerRef};
use game_api::LedgerApi;
use game_domain::{GameError, TakenReason};
use game_spi::{PlayerRepository, Treasury};
use tracing::{debug, info, warn};

/// Applies the gold and item movements requested by the auction house.
#[derive(Clone)]
pub struct LedgerService<P, T> {
    players: P,
    treasury: T,
}

impl<P: PlayerRepository, T: Treasury> LedgerService<P, T> {
    pub fn new(players: P, treasury: T) -> Self {
        Self { players, treasury }
    }
}

/// Logs one movement, then returns its result.
fn traced(
    result: Result<(), GameError>,
    call: &'static str,
    op: OpId,
    player: &PlayerRef,
    detail: String,
) -> Result<(), GameError> {
    match &result {
        Ok(()) => debug!(call, %op, %player, %detail, "ledger call applied"),
        Err(error) => warn!(call, %op, %player, %detail, %error, "ledger call failed"),
    }
    result
}

impl<P: PlayerRepository, T: Treasury> LedgerApi for LedgerService<P, T> {
    async fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> Result<(), GameError> {
        let result = self
            .players
            .update(player, |p| p.withdraw(item_type, quantity))
            .await;
        traced(
            result,
            "withdraw_item",
            op,
            player,
            format!("{item_type} x{quantity}"),
        )
    }

    async fn deposit_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> Result<(), GameError> {
        let result = self
            .players
            .update(player, |p| p.deposit(item_type, quantity))
            .await;
        traced(
            result,
            "deposit_item",
            op,
            player,
            format!("{item_type} x{quantity}"),
        )
    }

    async fn debit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), GameError> {
        let result = self.players.update(player, |p| p.debit(amount)).await;
        traced(result, "debit", op, player, amount.0.to_string())
    }

    async fn credit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), GameError> {
        let result = self.players.update(player, |p| p.credit(amount)).await;
        traced(result, "credit", op, player, amount.0.to_string())
    }

    async fn take_money(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> Result<(), GameError> {
        self.treasury.add(amount).await?;
        info!(%op, amount = amount.0, ?reason, "money taken by the auction house");
        Ok(())
    }
}
