//! HTTP adapter: lets the ui drive the auction core through the game server.

use std::sync::Arc;
use std::time::Duration;

use agora_api::{AuctionApi, BidOutcome, CreateListing};
use agora_domain::{AuctionError, HouseId, ItemStack, ListingId, Money, PortError, SearchQuery};
use agora_spi::Clock;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use game_dto::{
    BidRequest, BidResponse, BidResult, CatalogEntry, CreateListingRequest, CreateListingResponse,
    ErrorBody, HouseView, ListingsParams, ListingsResponse, PlayerRequest, PlayerView,
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, error, info, warn};
use uuid::Uuid;

use crate::Auction;
use crate::world::{SystemClock, World};

const DEFAULT_PAGE: u32 = 50;

#[derive(Clone)]
pub struct AppState {
    pub auction: Arc<Auction>,
    pub world: World,
    pub catalog: Arc<Vec<CatalogEntry>>,
    pub houses: Arc<Vec<HouseView>>,
}

pub fn router(state: AppState) -> Router {
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
        .with_state(state)
        // Spans at INFO so handler logs carry the method and uri. Responses at DEBUG:
        // the ui polls every 2 seconds.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::DEBUG)),
        )
}

pub struct ApiError(StatusCode, String);

impl From<AuctionError> for ApiError {
    fn from(error: AuctionError) -> Self {
        let status = match &error {
            AuctionError::HouseNotFound | AuctionError::ListingNotFound => StatusCode::NOT_FOUND,
            AuctionError::Conflict => StatusCode::CONFLICT,
            AuctionError::Repository(_) | AuctionError::Port(PortError::Other(_)) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
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

async fn catalog(State(state): State<AppState>) -> Json<Vec<CatalogEntry>> {
    Json(state.catalog.as_ref().clone())
}

async fn houses(State(state): State<AppState>) -> Json<Vec<HouseView>> {
    Json(state.houses.as_ref().clone())
}

async fn players(State(state): State<AppState>) -> Json<Vec<PlayerView>> {
    Json(state.world.players())
}

/// Money destroyed by the money sink: commissions and forfeited deposits.
async fn sink(State(state): State<AppState>) -> Json<Money> {
    Json(state.world.destroyed())
}

async fn listings(
    State(state): State<AppState>,
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
    let page = state.auction.search(query).await?;
    Ok(Json(ListingsResponse {
        now: SystemClock.now(),
        page,
    }))
}

async fn create_listing(
    State(state): State<AppState>,
    Json(req): Json<CreateListingRequest>,
) -> ApiResult<CreateListingResponse> {
    // The base value comes from the server catalog, never from the client.
    let entry = state
        .catalog
        .iter()
        .find(|e| e.item_type == req.item_type)
        .ok_or_else(|| {
            ApiError(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("unknown item type {}", req.item_type),
            )
        })?;
    let (house, seller, item_type, quantity) = (
        req.house,
        req.seller.clone(),
        req.item_type.clone(),
        req.quantity,
    );
    let cmd = CreateListing {
        house: req.house,
        seller: req.seller,
        item: ItemStack {
            item_type: req.item_type,
            quantity: req.quantity,
            base_value: entry.base_value,
            attributes: serde_json::Value::Null,
        },
        start_unit_price: req.start_unit_price,
        buyout_unit_price: req.buyout_unit_price,
        duration: Duration::from_secs(req.duration_secs),
    };
    let (start, buyout) = (cmd.start_unit_price, cmd.buyout_unit_price);
    let id = state.auction.create_listing(cmd).await?;
    info!(
        listing = %id,
        %house,
        %seller,
        %item_type,
        quantity,
        start_unit_price = start.0,
        buyout_unit_price = buyout.map(|b| b.0),
        "listing created"
    );
    Ok(Json(CreateListingResponse { id }))
}

async fn bid(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<BidRequest>,
) -> ApiResult<BidResponse> {
    let bidder = req.bidder.clone();
    let outcome = state
        .auction
        .place_bid(req.bidder, ListingId(id), req.unit_price)
        .await?;
    info!(listing = %id, %bidder, unit_price = req.unit_price.0, ?outcome, "bid placed");
    let result = match outcome {
        BidOutcome::HighBidder => BidResult::HighBidder,
        BidOutcome::Bought => BidResult::Bought,
    };
    Ok(Json(BidResponse { result }))
}

async fn buyout(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PlayerRequest>,
) -> Result<StatusCode, ApiError> {
    let buyer = req.player.clone();
    state.auction.buyout(req.player, ListingId(id)).await?;
    info!(listing = %id, %buyer, "listing bought out");
    Ok(StatusCode::NO_CONTENT)
}

async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PlayerRequest>,
) -> Result<StatusCode, ApiError> {
    let seller = req.player.clone();
    state
        .auction
        .cancel_listing(req.player, ListingId(id))
        .await?;
    info!(listing = %id, %seller, "listing cancelled");
    Ok(StatusCode::NO_CONTENT)
}
