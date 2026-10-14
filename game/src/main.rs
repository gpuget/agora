//! Demo game server: hosts the Agora core in process and exposes it over HTTP.

mod http;
mod world;

use std::sync::Arc;
use std::time::Duration;

use agora_api::AuctionApi;
use agora_core::AuctionService;
use agora_core::memory::InMemoryAuctionRepository;
use agora_domain::{Bps, DurationOption, HouseConfig, Money};
use game_dto::{CatalogEntry, DurationView, HouseView};

use crate::http::AppState;
use crate::world::{SystemClock, World};

pub type Auction = AuctionService<InMemoryAuctionRepository, World, World, World, SystemClock>;

/// 1 gold = 100 silver = 10 000 copper. Amounts are in copper.
const GOLD: u64 = 10_000;
const SETTLE_EVERY: Duration = Duration::from_secs(1);

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

fn seed_players(world: &World) {
    world.add_player(
        "alice".into(),
        Money(100 * GOLD),
        &[("linen_cloth", 200), ("healing_potion", 20)],
    );
    world.add_player(
        "bob".into(),
        Money(100 * GOLD),
        &[("copper_ore", 100), ("iron_sword", 2)],
    );
    world.add_player(
        "carol".into(),
        Money(100 * GOLD),
        &[("linen_cloth", 50), ("healing_potion", 10)],
    );
}

#[tokio::main]
async fn main() {
    let world = World::default();
    seed_players(&world);

    let auction = Arc::new(AuctionService::new(
        InMemoryAuctionRepository::default(),
        world.clone(),
        world.clone(),
        world.clone(),
        SystemClock,
    ));

    let mut houses = Vec::new();
    for config in [
        house_config("Alliance", Bps(500)),
        house_config("Neutre", Bps(1_500)),
    ] {
        let view = HouseView {
            id: auction
                .create_auction_house(config.clone())
                .await
                .expect("valid house config"),
            name: config.name,
            commission: config.commission.0,
            durations: config
                .durations
                .iter()
                .map(|o| DurationView {
                    secs: o.duration.as_secs(),
                    deposit_multiplier: o.deposit_multiplier,
                })
                .collect(),
        };
        houses.push(view);
    }

    let settler = auction.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SETTLE_EVERY);
        loop {
            tick.tick().await;
            match settler.settle_expired().await {
                Ok(0) => {}
                Ok(n) => println!("settled {n} listing(s)"),
                Err(e) => eprintln!("settlement failed: {e}"),
            }
        }
    });

    let state = AppState {
        auction,
        world: world.clone(),
        catalog: Arc::new(catalog()),
        houses: Arc::new(houses),
    };
    let app = http::router(state);

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("serve");
}
