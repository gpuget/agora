//! Trading card game server: wires the game core, the Agora core and their adapters.

mod adapters;
mod settings;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use agora_core::AuctionService;
use agora_core::memory::InMemoryAuctionRepository;
use agora_domain::{Money, PlayerRef};
use event_sourcing::memory::InMemoryEventStore;
use tcg_api::{LedgerApi, TcgApi};
use tcg_core::memory::{
    InMemoryCollectorProjection, InMemoryHouseRegistry, InMemoryTreasuryProjection, StaticCatalog,
};
use tcg_core::{CaughtUpView, LedgerService, TcgService};
use tcg_domain::{CardEntry, Collector, CollectorEvent, TreasuryEvent, TreasuryId};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::adapters::agora_bridge::AgoraBridge;
use crate::adapters::clock::SystemClock;
use crate::settings::Settings;

type CollectorEvents = InMemoryEventStore<PlayerRef, CollectorEvent>;
type TreasuryEvents = InMemoryEventStore<TreasuryId, TreasuryEvent>;
type Ledger = LedgerService<CollectorEvents, TreasuryEvents>;
type Bridge = AgoraBridge<Ledger>;
type Agora = AuctionService<InMemoryAuctionRepository, Bridge, Bridge, Bridge, SystemClock>;
type Collectors =
    CaughtUpView<CollectorEvents, InMemoryCollectorProjection, PlayerRef, CollectorEvent>;
type Treasury = CaughtUpView<TreasuryEvents, InMemoryTreasuryProjection, TreasuryId, TreasuryEvent>;
pub type Tcg = TcgService<Collectors, StaticCatalog, InMemoryHouseRegistry, Treasury, Agora>;

const COLLECTOR_IDS: [&str; 3] = ["alice", "bob", "carol"];

/// Collectors with 1000 to 5000 dust and 3 to 6 random cards of the catalog, 1 to 3 copies
/// each.
fn starting_collectors(catalog: &[CardEntry]) -> Vec<Collector> {
    COLLECTOR_IDS
        .into_iter()
        .map(|id| {
            let dust = Money(rand::random_range(1_000..=5_000));
            let mut collector = Collector::new(id.into(), dust);
            let kinds = rand::random_range(3.min(catalog.len())..=catalog.len().min(6));
            let mut chosen = BTreeSet::new();
            while chosen.len() < kinds {
                chosen.insert(rand::random_range(0..catalog.len()));
            }
            for index in chosen {
                let copies = rand::random_range(1..=3);
                collector = collector.with_card(catalog[index].card.clone(), copies);
            }
            collector
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let path = PathBuf::from(
        std::env::var("TCG_CONFIG").unwrap_or_else(|_| settings::DEFAULT_PATH.into()),
    );
    let read = Settings::read(&path).unwrap_or_else(|e| panic!("{e}"));
    let found = read.is_some();
    let settings = read.unwrap_or_default();
    assert!(
        !settings.catalog.is_empty(),
        "no catalog: {} must list the cards of the game",
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

    let catalog = settings.catalog();
    let collector_events = CollectorEvents::default();
    let treasury_events = TreasuryEvents::default();
    let ledger = LedgerService::new(collector_events.clone(), treasury_events.clone());
    for collector in starting_collectors(&catalog) {
        info!(id = %collector.id, dust = collector.dust.0, cards = ?collector.cards, "collector created");
        ledger
            .open_collector(collector)
            .await
            .expect("new collector");
    }

    // Read models of the ledger, caught up with its events on every read.
    let collectors = CaughtUpView::new(collector_events, InMemoryCollectorProjection::default());
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

    let tcg = Arc::new(TcgService::new(
        collectors,
        StaticCatalog::new(catalog),
        InMemoryHouseRegistry::default(),
        treasury,
        agora,
    ));

    for house in &settings.houses {
        tcg.open_auction_house(house.config())
            .await
            .unwrap_or_else(|e| panic!("invalid house {}: {e:?}", house.name));
    }

    let settler = tcg.clone();
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

    let app = adapters::http::router(tcg);
    let addr = std::env::var("BIND_ADDR").unwrap_or(settings.bind_addr);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    info!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("serve");
}
