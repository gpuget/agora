//! The game side: players, money and inventories. Implements the outbound ports of Agora.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use agora_domain::{HouseId, ItemStack, ItemType, Money, OpId, PlayerRef, PortError, Timestamp};
use agora_spi::{Clock, Inventory, MoneySink, SinkReason, Wallet};
use game_dto::{ItemCount, PlayerView};

#[derive(Default)]
struct Player {
    gold: Money,
    items: BTreeMap<ItemType, u32>,
}

#[derive(Default)]
struct State {
    players: BTreeMap<PlayerRef, Player>,
    /// Money taken by the auction houses. The demo just destroys it.
    destroyed: Money,
}

/// Shared handle on the game state. Cheap to clone.
#[derive(Clone, Default)]
pub struct World(Arc<Mutex<State>>);

impl World {
    pub fn add_player(&self, player: PlayerRef, gold: Money, items: &[(&str, u32)]) {
        let items = items
            .iter()
            .map(|(t, q)| (ItemType::from(*t), *q))
            .collect();
        self.0
            .lock()
            .unwrap()
            .players
            .insert(player, Player { gold, items });
    }

    pub fn players(&self) -> Vec<PlayerView> {
        let state = self.0.lock().unwrap();
        state
            .players
            .iter()
            .map(|(player, p)| PlayerView {
                player: player.clone(),
                gold: p.gold,
                items: p
                    .items
                    .iter()
                    .filter(|(_, q)| **q > 0)
                    .map(|(t, q)| ItemCount {
                        item_type: t.clone(),
                        quantity: *q,
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn destroyed(&self) -> Money {
        self.0.lock().unwrap().destroyed
    }

    fn with_player<T>(
        &self,
        player: &PlayerRef,
        f: impl FnOnce(&mut Player) -> Result<T, PortError>,
    ) -> Result<T, PortError> {
        let mut state = self.0.lock().unwrap();
        let player = state
            .players
            .get_mut(player)
            .ok_or_else(|| PortError::Other(format!("unknown player {player}")))?;
        f(player)
    }
}

impl Inventory for World {
    async fn withdraw_item(
        &self,
        _op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> Result<(), PortError> {
        self.with_player(player, |p| {
            let held = p
                .items
                .get_mut(&item.item_type)
                .filter(|held| **held >= item.quantity)
                .ok_or(PortError::ItemUnavailable)?;
            *held -= item.quantity;
            Ok(())
        })
    }

    async fn deposit_item(
        &self,
        _op: OpId,
        player: &PlayerRef,
        item: &ItemStack,
    ) -> Result<(), PortError> {
        self.with_player(player, |p| {
            *p.items.entry(item.item_type.clone()).or_default() += item.quantity;
            Ok(())
        })
    }
}

impl Wallet for World {
    async fn debit(&self, _op: OpId, player: &PlayerRef, amount: Money) -> Result<(), PortError> {
        self.with_player(player, |p| {
            p.gold = p
                .gold
                .checked_sub(amount)
                .ok_or(PortError::InsufficientFunds)?;
            Ok(())
        })
    }

    async fn credit(&self, _op: OpId, player: &PlayerRef, amount: Money) -> Result<(), PortError> {
        self.with_player(player, |p| {
            p.gold = p
                .gold
                .checked_add(amount)
                .ok_or_else(|| PortError::Other("gold overflow".into()))?;
            Ok(())
        })
    }
}

impl MoneySink for World {
    async fn absorb(
        &self,
        _op: OpId,
        _house: HouseId,
        amount: Money,
        _reason: SinkReason,
    ) -> Result<(), PortError> {
        let mut state = self.0.lock().unwrap();
        state.destroyed = Money(state.destroyed.0.saturating_add(amount.0));
        Ok(())
    }
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before 1970")
            .as_millis();
        Timestamp(u64::try_from(millis).expect("timestamp fits in u64"))
    }
}
