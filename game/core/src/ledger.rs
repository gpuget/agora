use agora_domain::{ItemType, Money, OpId, PlayerRef};
use game_api::LedgerApi;
use game_domain::{
    GameError, Player, PlayerCommand, PlayerDecider, PlayerEvent, TakenReason, TreasuryCommand,
    TreasuryDecider, TreasuryEvent, TreasuryId,
};
use game_spi::EventStore;
use tracing::{debug, info, warn};

use crate::event_sourcing::execute;

/// Applies the gold and item movements requested by the auction house, as events.
#[derive(Clone)]
pub struct LedgerService<P, T> {
    players: P,
    treasury: T,
}

impl<P, T> LedgerService<P, T>
where
    P: EventStore<PlayerRef, PlayerEvent>,
    T: EventStore<TreasuryId, TreasuryEvent>,
{
    pub fn new(players: P, treasury: T) -> Self {
        Self { players, treasury }
    }

    /// Runs one movement on a player, then logs its result.
    async fn movement(
        &self,
        call: &'static str,
        op: OpId,
        player: &PlayerRef,
        command: PlayerCommand,
        detail: String,
    ) -> Result<(), GameError> {
        let result = execute::<PlayerDecider, _>(&self.players, player, &command).await;
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

impl<P, T> LedgerApi for LedgerService<P, T>
where
    P: EventStore<PlayerRef, PlayerEvent>,
    T: EventStore<TreasuryId, TreasuryEvent>,
{
    async fn open_player(&self, player: Player) -> Result<(), GameError> {
        let command = PlayerCommand::Open {
            gold: player.gold,
            items: player.items,
        };
        execute::<PlayerDecider, _>(&self.players, &player.id, &command).await?;
        info!(player = %player.id, "player opened");
        Ok(())
    }

    async fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> Result<(), GameError> {
        let command = PlayerCommand::Withdraw {
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
    ) -> Result<(), GameError> {
        let command = PlayerCommand::Deposit {
            op,
            item_type: item_type.clone(),
            quantity,
        };
        let detail = format!("{item_type} x{quantity}");
        self.movement("deposit_item", op, player, command, detail)
            .await
    }

    async fn debit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), GameError> {
        let command = PlayerCommand::Debit { op, amount };
        self.movement("debit", op, player, command, amount.0.to_string())
            .await
    }

    async fn credit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), GameError> {
        let command = PlayerCommand::Credit { op, amount };
        self.movement("credit", op, player, command, amount.0.to_string())
            .await
    }

    async fn take_money(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> Result<(), GameError> {
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
