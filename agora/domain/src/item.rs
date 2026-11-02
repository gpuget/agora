use serde::{Deserialize, Serialize};

use crate::{ItemType, Money};

/// A stack of identical items. A commodity can be bought in part and never takes bids,
/// any other item is auctioned and bought as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item_type: ItemType,
    pub quantity: u32,
    /// Provided by the game.
    #[serde(default)]
    pub commodity: bool,
    /// Value of one item, provided by the game. Used to compute the deposit.
    pub base_value: Money,
    /// Opaque to the core: stored and returned as is.
    #[serde(default)]
    pub attributes: serde_json::Value,
}
