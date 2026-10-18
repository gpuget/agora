//! Demo game server: wires the game core, the Agora core and their adapters.

mod adapters;
mod settings;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use agora_core::AuctionService;
use agora_core::memory::InMemoryAuctionRepository;
use agora_domain::{Money, PlayerRef};
use event_sourcing::memory::InMemoryEventStore;
use game_api::{GameApi, LedgerApi};
use game_core::memory::{
    InMemoryHouseRegistry, InMemoryPlayerProjection, InMemoryTreasuryProjection, StaticCatalog,
};
use game_core::{CaughtUpView, GameService, LedgerService};
use game_domain::{Player, PlayerEvent, TreasuryEvent, TreasuryId};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::adapters::agora_bridge::AgoraBridge;
use crate::adapters::clock::SystemClock;
use crate::settings::{CatalogSettings, Settings};

type PlayerEvents = InMemoryEventStore<PlayerRef, PlayerEvent>;
type TreasuryEvents = InMemoryEventStore<TreasuryId, TreasuryEvent>;
type Ledger = LedgerService<PlayerEvents, TreasuryEvents>;
type Bridge = AgoraBridge<Ledger>;
type Agora = AuctionService<InMemoryAuctionRepository, Bridge, Bridge, Bridge, SystemClock>;
type Players = CaughtUpView<PlayerEvents, InMemoryPlayerProjection, PlayerRef, PlayerEvent>;
type Treasury = CaughtUpView<TreasuryEvents, InMemoryTreasuryProjection, TreasuryId, TreasuryEvent>;
pub type Game = GameService<Players, StaticCatalog, InMemoryHouseRegistry, Treasury, Agora>;

/// 1 gold = 100 silver = 10 000 copper. Amounts are in copper.
const GOLD: u64 = 10_000;

const PLAYER_IDS: [&str; 3] = ["alice", "bob", "carol"];

/// Players with 50 to 200 gold and 1 to 3 random item types of the catalog. Each stack is worth
/// 0.2 to 2 gold, at least one item.
fn starting_players(catalog: &[CatalogSettings]) -> Vec<Player> {
    PLAYER_IDS
        .into_iter()
        .map(|id| {
            let gold = Money(rand::random_range(50..=200) * GOLD);
            let mut player = Player::new(id.into(), gold);
            let kinds = rand::random_range(1..=catalog.len().min(3));
            let mut chosen = BTreeSet::new();
            while chosen.len() < kinds {
                chosen.insert(rand::random_range(0..catalog.len()));
            }
            for index in chosen {
                let item = catalog[index].entry();
                let worth = rand::random_range(GOLD / 5..=2 * GOLD);
                let quantity = (worth / item.base_value.0.max(1)).clamp(1, u32::MAX as u64);
                player = player.with_item(item.item_type, quantity as u32);
            }
            player
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let path = PathBuf::from(
        std::env::var("GAME_CONFIG").unwrap_or_else(|_| settings::DEFAULT_PATH.into()),
    );
    let read = Settings::read(&path).unwrap_or_else(|e| panic!("{e}"));
    let found = read.is_some();
    let settings = read.unwrap_or_default();
    assert!(
        !settings.catalog.is_empty(),
        "no catalog: {} must list the items of the game",
        path.display()
    );

    // RUST_LOG overrides the log table of the settings.
    let filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => EnvFilter::try_new(settings.log_filter())
            .unwrap_or_else(|e| panic!("invalid log table in {}: {e}", path.display())),
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();
    if found {
        info!(path = %path.display(), "settings read");
    } else {
        info!(path = %path.display(), "no settings file, using defaults");
    }

    let player_events = PlayerEvents::default();
    let treasury_events = TreasuryEvents::default();
    let ledger = LedgerService::new(player_events.clone(), treasury_events.clone());
    for player in starting_players(&settings.catalog) {
        info!(id = %player.id, gold = player.gold.0, items = ?player.items, "player created");
        ledger.open_player(player).await.expect("new player");
    }

    // Read models of the ledger, caught up with its events on every read.
    let players = CaughtUpView::new(player_events, InMemoryPlayerProjection::default());
    let treasury = CaughtUpView::new(treasury_events, InMemoryTreasuryProjection::default());

    // Agora drives the game ledger through its outbound ports.
    let bridge = AgoraBridge(ledger);
    let agora = AuctionService::new(
        InMemoryAuctionRepository::default(),
        bridge.clone(),
        bridge.clone(),
        bridge,
        SystemClock,
    );

    let game = Arc::new(GameService::new(
        players,
        StaticCatalog::new(settings.catalog.iter().map(|c| c.entry()).collect()),
        InMemoryHouseRegistry::default(),
        treasury,
        agora,
    ));

    for house in &settings.houses {
        game.open_auction_house(house.config())
            .await
            .unwrap_or_else(|e| panic!("invalid house {}: {e:?}", house.name));
    }

    let settler = game.clone();
    let settle_every = settings.settle_every();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(settle_every);
        loop {
            tick.tick().await;
            if let Err(error) = settler.settle_auctions().await {
                error!(%error, "settlement failed");
            }
        }
    });

    let app = adapters::http::router(game);
    let addr = std::env::var("BIND_ADDR").unwrap_or(settings.bind_addr);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("serve");
}
