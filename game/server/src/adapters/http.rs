//! HTTP adapter: exposes the game use cases to the ui. Maps domain types to `game-dto`.

use std::sync::Arc;
use std::time::Duration;

use agora_domain::{AuctionError, HouseId, ListingId, Money, PortError, SearchQuery};
use agora_spi::Clock;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use game_api::{BidOutcome, GameApi, SellCommand};
use game_domain::GameError;
use game_dto::{
    BidRequest, BidResponse, BidResult, CatalogEntry, CreateListingRequest, CreateListingResponse,
    DurationView, ErrorBody, HouseView, ItemCount, ListingsParams, ListingsResponse, PlayerRequest,
    PlayerView,
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, error, warn};
use uuid::Uuid;

use crate::Game;
use crate::adapters::clock::SystemClock;

const DEFAULT_PAGE: u32 = 50;

pub fn router(game: Arc<Game>) -> Router {
    Router::new()
        .route("/api/catalog", get(catalog))
        .route("/api/houses", get(houses))
        .route("/api/players", get(players))
        .route("/api/sink", get(sink))
        .route("/api/houses/{house}/listings", get(listings))
        .route("/api/listings", post(create_listing))
        .route("/api/listings/{id}/bid", post(bid))
        .route("/api/listings/{id}/buyout", post(buyout))
        .route("/api/listings/{id}/cancel", post(cancel))
        .with_state(game)
        // Spans at INFO so core logs carry the method and uri. Responses at DEBUG:
        // the ui polls every 2 seconds.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::DEBUG)),
        )
}

pub struct ApiError(StatusCode, String);

impl From<GameError> for ApiError {
    fn from(error: GameError) -> Self {
        let status = match &error {
            GameError::UnknownPlayer(_)
            | GameError::Auction(AuctionError::HouseNotFound | AuctionError::ListingNotFound) => {
                StatusCode::NOT_FOUND
            }
            GameError::Conflict | GameError::Auction(AuctionError::Conflict) => {
                StatusCode::CONFLICT
            }
            GameError::Storage(_)
            | GameError::Auction(
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

async fn catalog(State(game): State<Arc<Game>>) -> ApiResult<Vec<CatalogEntry>> {
    let entries = game.catalog().await?;
    Ok(Json(
        entries
            .into_iter()
            .map(|e| CatalogEntry {
                item_type: e.item_type,
                name: e.name,
                base_value: e.base_value,
            })
            .collect(),
    ))
}

async fn houses(State(game): State<Arc<Game>>) -> ApiResult<Vec<HouseView>> {
    let houses = game.auction_houses().await?;
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

async fn players(State(game): State<Arc<Game>>) -> ApiResult<Vec<PlayerView>> {
    let players = game.players().await?;
    Ok(Json(
        players
            .into_iter()
            .map(|p| PlayerView {
                player: p.id,
                gold: p.gold,
                items: p
                    .items
                    .into_iter()
                    .map(|(item_type, quantity)| ItemCount {
                        item_type,
                        quantity,
                    })
                    .collect(),
            })
            .collect(),
    ))
}

/// Money taken out of the game: commissions and forfeited deposits.
async fn sink(State(game): State<Arc<Game>>) -> ApiResult<Money> {
    Ok(Json(game.money_taken().await?))
}

async fn listings(
    State(game): State<Arc<Game>>,
    Path(house): Path<Uuid>,
    Query(params): Query<ListingsParams>,
) -> ApiResult<ListingsResponse> {
    let query = SearchQuery {
        house: HouseId(house),
        item_type: params.item_type,
        seller: params.seller,
        high_bidder: params.high_bidder,
        limit: params.limit.unwrap_or(DEFAULT_PAGE),
        offset: params.offset.unwrap_or(0),
    };
    let page = game.search_market(query).await?;
    Ok(Json(ListingsResponse {
        now: SystemClock.now(),
        page,
    }))
}

async fn create_listing(
    State(game): State<Arc<Game>>,
    Json(req): Json<CreateListingRequest>,
) -> ApiResult<CreateListingResponse> {
    let cmd = SellCommand {
        player: req.seller,
        house: req.house,
        item_type: req.item_type,
        quantity: req.quantity,
        start_unit_price: req.start_unit_price,
        buyout_unit_price: req.buyout_unit_price,
        duration: Duration::from_secs(req.duration_secs),
    };
    let id = game.sell(cmd).await?;
    Ok(Json(CreateListingResponse { id }))
}

async fn bid(
    State(game): State<Arc<Game>>,
    Path(id): Path<Uuid>,
    Json(req): Json<BidRequest>,
) -> ApiResult<BidResponse> {
    let result = match game.bid(req.bidder, ListingId(id), req.unit_price).await? {
        BidOutcome::HighBidder => BidResult::HighBidder,
        BidOutcome::Bought => BidResult::Bought,
    };
    Ok(Json(BidResponse { result }))
}

async fn buyout(
    State(game): State<Arc<Game>>,
    Path(id): Path<Uuid>,
    Json(req): Json<PlayerRequest>,
) -> Result<StatusCode, ApiError> {
    game.buyout(req.player, ListingId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn cancel(
    State(game): State<Arc<Game>>,
    Path(id): Path<Uuid>,
    Json(req): Json<PlayerRequest>,
) -> Result<StatusCode, ApiError> {
    game.cancel(req.player, ListingId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
