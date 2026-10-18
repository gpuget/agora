use agora_domain::{ListingId, Money, PlayerRef};
use gloo_net::http::{Request, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tcg_dto::{
    BidRequest, BidResponse, BidResult, CollectorRequest, CreateListingRequest, ErrorBody,
};

async fn error_of(resp: &Response) -> String {
    match resp.json::<ErrorBody>().await {
        Ok(body) => body.error,
        Err(_) => format!("HTTP {}", resp.status()),
    }
}

async fn send(request: Result<Request, gloo_net::Error>) -> Result<Response, String> {
    let resp = request
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(resp)
    } else {
        Err(error_of(&resp).await)
    }
}

pub async fn get<T: DeserializeOwned>(url: &str, query: Vec<(&str, String)>) -> Result<T, String> {
    let resp = send(Request::get(url).query(query).build()).await?;
    resp.json().await.map_err(|e| e.to_string())
}

async fn post<B: Serialize>(url: &str, body: &B) -> Result<Response, String> {
    send(Request::post(url).json(body)).await
}

pub enum Action {
    Sell(CreateListingRequest),
    Bid(ListingId, Money),
    Buyout(ListingId),
    Cancel(ListingId),
}

pub async fn perform(action: Action, collector: PlayerRef) -> Result<String, String> {
    match action {
        Action::Sell(req) => post("/api/listings", &req)
            .await
            .map(|_| "Carte mise en vente.".into()),
        Action::Bid(id, price) => {
            let resp = post(
                &format!("/api/listings/{id}/bid"),
                &BidRequest {
                    bidder: collector,
                    price,
                },
            )
            .await?;
            match resp
                .json::<BidResponse>()
                .await
                .map_err(|e| e.to_string())?
                .result
            {
                BidResult::HighBidder => Ok("Enchère placée.".into()),
                BidResult::Bought => Ok("Prix d'achat immédiat atteint : carte achetée.".into()),
            }
        }
        Action::Buyout(id) => post(
            &format!("/api/listings/{id}/buyout"),
            &CollectorRequest { collector },
        )
        .await
        .map(|_| "Carte achetée.".into()),
        Action::Cancel(id) => post(
            &format!("/api/listings/{id}/cancel"),
            &CollectorRequest { collector },
        )
        .await
        .map(|_| "Vente annulée.".into()),
    }
}
