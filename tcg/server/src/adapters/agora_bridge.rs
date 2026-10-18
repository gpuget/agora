//! Implements the outbound ports of Agora by driving the game ledger.

use agora_domain::{HouseId, ItemStack, Money, OpId, PlayerRef, PortError};
use agora_spi::{Inventory, MoneySink, SinkReason, Wallet};
use tcg_api::LedgerApi;
use tcg_domain::{TakenReason, TcgError};

#[derive(Clone)]
pub struct AgoraBridge<L>(pub L);

fn port_error(error: TcgError) -> PortError {
    match error {
        TcgError::CardUnavailable => PortError::ItemUnavailable,
        TcgError::InsufficientDust => PortError::InsufficientFunds,
        other => PortError::Other(other.to_string()),
    }
}

impl<L: LedgerApi> Inventory for AgoraBridge<L> {
    async fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> Result<(), PortError> {
        self.0
            .withdraw_card(op, player, &item.item_type, item.quantity)
            .await
            .map_err(port_error)
    }

    async fn deposit_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> Result<(), PortError> {
        self.0
            .deposit_card(op, player, &item.item_type, item.quantity)
            .await
            .map_err(port_error)
    }
}

impl<L: LedgerApi> Wallet for AgoraBridge<L> {
    async fn debit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), PortError> {
        self.0.debit(op, player, amount).await.map_err(port_error)
    }

    async fn credit(&self, op: OpId, player: &PlayerRef, amount: Money) -> Result<(), PortError> {
        self.0.credit(op, player, amount).await.map_err(port_error)
    }
}

impl<L: LedgerApi> MoneySink for AgoraBridge<L> {
    async fn absorb(
        &self,
        op: OpId,
        _house: HouseId,
        amount: Money,
        reason: SinkReason,
    ) -> Result<(), PortError> {
        let reason = match reason {
            SinkReason::Commission { listing } => TakenReason::Commission { listing },
            SinkReason::ForfeitedDeposit { listing } => TakenReason::ForfeitedDeposit { listing },
        };
        self.0
            .take_dust(op, amount, reason)
            .await
            .map_err(port_error)
    }
}
