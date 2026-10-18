use std::future::Future;

use agora_domain::{ItemType, Money, OpId, PlayerRef};

use crate::{Account, LedgerError, TakenReason};

/// Money and items of the players, driven by the auction house through an adapter.
///
/// Every movement carries the [`OpId`] of the Agora call: a call replayed with the same
/// `OpId` succeeds without effect.
pub trait LedgerApi: Send + Sync {
    /// Opens a new account with its starting money and items. Fails with
    /// [`LedgerError::AccountExists`] if the player already has one.
    fn open_account(
        &self,
        account: Account,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    fn deposit_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    fn debit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    fn credit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    /// Money taken by the auction house (commission, forfeited deposit).
    fn take_money(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;
}
