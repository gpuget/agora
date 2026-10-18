//! Demo game server: wires the game core, the Agora core and their adapters.

mod adapters;

use std::sync::Arc;
use std::time::Duration;

use agora_core::AuctionService;
use agora_core::memory::InMemoryAuctionRepository;
use agora_domain::{Bps, DurationOption, HouseConfig, Money, PlayerRef};
use game_api::{GameApi, LedgerApi};
use game_core::memory::{
    InMemoryEventStore, InMemoryHouseRegistry, InMemoryPlayerProjection,
    InMemoryTreasuryProjection, StaticCatalog,
};
use game_core::{CaughtUpView, GameService, LedgerService};
use game_domain::{CatalogEntry, Player, PlayerEvent, TreasuryEvent, TreasuryId};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::adapters::agora_bridge::AgoraBridge;
use crate::adapters::clock::SystemClock;

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
const SETTLE_EVERY: Duration = Duration::from_secs(1);
/// Log filter when `RUST_LOG` is not set. `RUST_LOG=game_server=debug,game_core=debug,tower_http=debug`
/// shows ledger calls and every HTTP response.
const DEFAULT_LOG: &str = "game_server=info,game_core=info,tower_http=info";

fn catalog() -> Vec<CatalogEntry> {
    [
        ("linen_cloth", "Étoffe de lin", 13),
        ("copper_ore", "Minerai de cuivre", 5),
        ("healing_potion", "Potion de soins", 250),
        ("iron_sword", "Épée en fer", 4_500),
    ]
    .into_iter()
    .map(|(item_type, name, base_value)| CatalogEntry {
        item_type: item_type.into(),
        name: name.into(),
        base_value: Money(base_value),
    })
    .collect()
}

fn house_config(name: &str, commission: Bps) -> HouseConfig {
    let minutes = |m: u64| Duration::from_secs(m * 60);
    HouseConfig {
        name: name.into(),
        commission,
        min_increment: Money(1),
        increment: Bps(500),
        deposit: Bps(1_500),
        durations: vec![
            DurationOption {
                duration: minutes(2),
                deposit_multiplier: 1,
            },
            DurationOption {
                duration: minutes(10),
                deposit_multiplier: 2,
            },
            DurationOption {
                duration: minutes(30),
                deposit_multiplier: 4,
            },
        ],
    }
}

fn starting_players() -> Vec<Player> {
    let gold = Money(100 * GOLD);
    vec![
        Player::new("alice".into(), gold)
            .with_item("linen_cloth".into(), 200)
            .with_item("healing_potion".into(), 20),
        Player::new("bob".into(), gold)
            .with_item("copper_ore".into(), 100)
            .with_item("iron_sword".into(), 2),
        Player::new("carol".into(), gold)
            .with_item("linen_cloth".into(), 50)
            .with_item("healing_potion".into(), 10),
    ]
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG)),
        )
        .init();

    let player_events = PlayerEvents::default();
    let treasury_events = TreasuryEvents::default();
    let ledger = LedgerService::new(player_events.clone(), treasury_events.clone());
    for player in starting_players() {
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
        StaticCatalog::new(catalog()),
        InMemoryHouseRegistry::default(),
        treasury,
        agora,
    ));

    for config in [
        house_config("Alliance", Bps(500)),
        house_config("Neutre", Bps(1_500)),
    ] {
        game.open_auction_house(config)
            .await
            .expect("valid house config");
    }

    let settler = game.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SETTLE_EVERY);
        loop {
            tick.tick().await;
            if let Err(error) = settler.settle_auctions().await {
                error!(%error, "settlement failed");
            }
        }
    });

    let app = adapters::http::router(game);
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("serve");
}
