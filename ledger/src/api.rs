use std::future::Future;

use agora_domain::{ItemType, Money, OpId, PlayerRef};

use crate::{Account, LedgerError, TakenReason};

/// Money and items of the players, driven by the auction house through an adapter.
///
/// Every movement carries the [`OpId`] of the Agora call: a call replayed with the same
/// `OpId` succeeds without effect.
///
/// A movement on a player without an account fails with [`LedgerError::UnknownAccount`].
/// Every method can fail with [`LedgerError::Store`]. A failed call records nothing.
pub trait LedgerApi: Send + Sync {
    /// Opens a new account with its starting money and items. Fails with
    /// [`LedgerError::AccountExists`] if the player already has one.
    ///
    /// Not idempotent: it carries no `OpId`, so a second call fails.
    fn open_account(
        &self,
        account: Account,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    /// Removes `quantity` items of `item_type` from the player. Fails with
    /// [`LedgerError::ItemUnavailable`] if the player holds fewer.
    fn withdraw_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    /// Adds `quantity` items of `item_type` to the player. Fails with
    /// [`LedgerError::Overflow`] if the held quantity would exceed `u32::MAX`.
    fn deposit_item(
        &self,
        op: OpId,
        player: &PlayerRef,
        item_type: &ItemType,
        quantity: u32,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    /// Takes `amount` from the funds of the player. Fails with
    /// [`LedgerError::InsufficientFunds`] if the funds are lower.
    fn debit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    /// Adds `amount` to the funds of the player. Fails with [`LedgerError::Overflow`] if the
    /// funds would exceed `u64::MAX`.
    fn credit(
        &self,
        op: OpId,
        player: &PlayerRef,
        amount: Money,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;

    /// Money taken by the auction house (commission, forfeited deposit).
    ///
    /// Adds `amount` to the treasury; no account is touched. Fails with
    /// [`LedgerError::Overflow`] if the treasury total would exceed `u64::MAX`.
    fn take_money(
        &self,
        op: OpId,
        amount: Money,
        reason: TakenReason,
    ) -> impl Future<Output = Result<(), LedgerError>> + Send;
}
