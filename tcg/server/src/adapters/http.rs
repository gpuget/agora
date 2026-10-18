//! HTTP adapter: exposes the game use cases to the ui. Maps domain types to `tcg-dto`.

use std::sync::Arc;
use std::time::Duration;

use agora_domain::{AuctionError, HouseId, ListingId, Money, PortError, SearchQuery};
use agora_spi::Clock;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tcg_api::{BidOutcome, SellCommand, TcgApi};
use tcg_domain::{Rarity, TcgError};
use tcg_dto::{
    BidRequest, BidResponse, BidResult, CardCount, CardEntry, CollectorRequest, CollectorView,
    CreateListingRequest, CreateListingResponse, DurationView, ErrorBody, HouseView,
    ListingsParams, ListingsResponse,
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, error, warn};
use uuid::Uuid;

use crate::Tcg;
use crate::adapters::clock::SystemClock;

const DEFAULT_PAGE: u32 = 50;

pub fn router(tcg: Arc<Tcg>) -> Router {
    Router::new()
        .route("/api/catalog", get(catalog))
        .route("/api/houses", get(houses))
        .route("/api/collectors", get(collectors))
        .route("/api/sink", get(sink))
        .route("/api/houses/{house}/listings", get(listings))
        .route("/api/listings", post(create_listing))
        .route("/api/listings/{id}/bid", post(bid))
        .route("/api/listings/{id}/buyout", post(buyout))
        .route("/api/listings/{id}/cancel", post(cancel))
        .with_state(tcg)
        // Spans at INFO so core logs carry the method and uri. Responses at DEBUG:
        // the ui polls every second.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::DEBUG)),
        )
}

pub struct ApiError(StatusCode, String);

impl From<TcgError> for ApiError {
    fn from(error: TcgError) -> Self {
        let status = match &error {
            TcgError::UnknownCollector(_)
            | TcgError::Auction(AuctionError::HouseNotFound | AuctionError::ListingNotFound) => {
                StatusCode::NOT_FOUND
            }
            TcgError::Conflict | TcgError::Auction(AuctionError::Conflict) => StatusCode::CONFLICT,
            TcgError::Storage(_)
            | TcgError::Auction(
                AuctionError::Repository(_) | AuctionError::Port(PortError::Other(_)),
            ) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        ApiError(status, error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.0.is_server_error() {
            error!(status = self.0.as_u16(), error = %self.1, "request failed");
        } else {
            warn!(status = self.0.as_u16(), error = %self.1, "request rejected");
        }
        (self.0, Json(ErrorBody { error: self.1 })).into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

fn rarity(rarity: Rarity) -> tcg_dto::Rarity {
    match rarity {
        Rarity::Common => tcg_dto::Rarity::Common,
        Rarity::Rare => tcg_dto::Rarity::Rare,
        Rarity::Epic => tcg_dto::Rarity::Epic,
        Rarity::Legendary => tcg_dto::Rarity::Legendary,
    }
}

async fn catalog(State(tcg): State<Arc<Tcg>>) -> ApiResult<Vec<CardEntry>> {
    let entries = tcg.catalog().await?;
    Ok(Json(
        entries
            .into_iter()
            .map(|e| CardEntry {
                card: e.card,
                name: e.name,
                rarity: rarity(e.rarity),
                base_value: e.base_value,
            })
            .collect(),
    ))
}

async fn houses(State(tcg): State<Arc<Tcg>>) -> ApiResult<Vec<HouseView>> {
    let houses = tcg.auction_houses().await?;
    Ok(Json(
        houses
            .into_iter()
            .map(|h| HouseView {
                id: h.id,
                name: h.config.name,
                commission: h.config.commission.0,
                deposit: h.config.deposit.0,
                durations: h
                    .config
                    .durations
                    .iter()
                    .map(|o| DurationView {
                        secs: o.duration.as_secs(),
                        deposit_multiplier: o.deposit_multiplier,
                    })
                    .collect(),
            })
            .collect(),
    ))
}

async fn collectors(State(tcg): State<Arc<Tcg>>) -> ApiResult<Vec<CollectorView>> {
    let collectors = tcg.collectors().await?;
    Ok(Json(
        collectors
            .into_iter()
            .map(|c| CollectorView {
                collector: c.id,
                dust: c.dust,
                cards: c
                    .cards
                    .into_iter()
                    .map(|(card, copies)| CardCount { card, copies })
                    .collect(),
            })
            .collect(),
    ))
}

/// Dust taken out of the game: commissions and forfeited deposits.
async fn sink(State(tcg): State<Arc<Tcg>>) -> ApiResult<Money> {
    Ok(Json(tcg.dust_taken().await?))
}

async fn listings(
    State(tcg): State<Arc<Tcg>>,
    Path(house): Path<Uuid>,
    Query(params): Query<ListingsParams>,
) -> ApiResult<ListingsResponse> {
    let query = SearchQuery {
        house: HouseId(house),
        item_type: params.card,
        seller: params.seller,
        high_bidder: params.high_bidder,
        limit: params.limit.unwrap_or(DEFAULT_PAGE),
        offset: params.offset.unwrap_or(0),
    };
    let page = tcg.search_market(query).await?;
    Ok(Json(ListingsResponse {
        now: SystemClock.now(),
        page,
    }))
}

async fn create_listing(
    State(tcg): State<Arc<Tcg>>,
    Json(req): Json<CreateListingRequest>,
) -> ApiResult<CreateListingResponse> {
    let cmd = SellCommand {
        collector: req.seller,
        house: req.house,
        card: req.card,
        start_price: req.start_price,
        buyout_price: req.buyout_price,
        duration: Duration::from_secs(req.duration_secs),
    };
    let id = tcg.sell(cmd).await?;
    Ok(Json(CreateListingResponse { id }))
}

async fn bid(
    State(tcg): State<Arc<Tcg>>,
    Path(id): Path<Uuid>,
    Json(req): Json<BidRequest>,
) -> ApiResult<BidResponse> {
    let result = match tcg.bid(req.bidder, ListingId(id), req.price).await? {
        BidOutcome::HighBidder => BidResult::HighBidder,
        BidOutcome::Bought => BidResult::Bought,
    };
    Ok(Json(BidResponse { result }))
}

async fn buyout(
    State(tcg): State<Arc<Tcg>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CollectorRequest>,
) -> Result<StatusCode, ApiError> {
    tcg.buyout(req.collector, ListingId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn cancel(
    State(tcg): State<Arc<Tcg>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CollectorRequest>,
) -> Result<StatusCode, ApiError> {
    tcg.cancel(req.collector, ListingId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
