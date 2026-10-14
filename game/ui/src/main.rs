use std::rc::Rc;

use agora_domain::{HouseId, ItemType, Listing, ListingId, Money, PlayerRef, Timestamp};
use game_dto::{
    BidRequest, BidResponse, BidResult, CatalogEntry, CreateListingRequest, ErrorBody, HouseView,
    ListingsResponse, PlayerRequest, PlayerView,
};
use gloo_net::http::{Request, Response};
use gloo_timers::callback::Interval;
use serde::Serialize;
use serde::de::DeserializeOwned;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

const REFRESH_MS: u32 = 2_000;

// ---- HTTP ----

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

async fn get<T: DeserializeOwned>(url: &str, query: Vec<(&str, String)>) -> Result<T, String> {
    let resp = send(Request::get(url).query(query).build()).await?;
    resp.json().await.map_err(|e| e.to_string())
}

async fn post<B: Serialize>(url: &str, body: &B) -> Result<Response, String> {
    send(Request::post(url).json(body)).await
}

// ---- formatting ----

/// Amounts are in copper: 1 gold = 100 silver = 10 000 copper.
fn money(amount: Money) -> String {
    let (gold, silver, copper) = (amount.0 / 10_000, amount.0 / 100 % 100, amount.0 % 100);
    let mut parts = Vec::new();
    if gold > 0 {
        parts.push(format!("{gold} po"));
    }
    if silver > 0 {
        parts.push(format!("{silver} pa"));
    }
    if copper > 0 || parts.is_empty() {
        parts.push(format!("{copper} pc"));
    }
    parts.join(" ")
}

fn time_left(now: Timestamp, end: Timestamp) -> String {
    let secs = now.until(end).as_secs();
    if secs == 0 {
        "terminée".into()
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

fn item_name(catalog: &[CatalogEntry], item_type: &ItemType) -> String {
    catalog
        .iter()
        .find(|e| &e.item_type == item_type)
        .map_or_else(|| item_type.to_string(), |e| e.name.clone())
}

fn input_value(e: &InputEvent) -> String {
    e.target_unchecked_into::<HtmlInputElement>().value()
}

fn select_value(e: &Event) -> String {
    e.target_unchecked_into::<HtmlSelectElement>().value()
}

// ---- state ----

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    MySales,
    MyBids,
}

enum Action {
    Sell(CreateListingRequest),
    Bid(ListingId, Money),
    Buyout(ListingId),
    Cancel(ListingId),
}

/// Bumped to trigger a refresh.
#[derive(Default, PartialEq)]
struct Reload(u32);

impl Reducible for Reload {
    type Action = ();

    fn reduce(self: Rc<Self>, _: ()) -> Rc<Self> {
        Rc::new(Reload(self.0.wrapping_add(1)))
    }
}

async fn perform(action: Action, player: PlayerRef) -> Result<String, String> {
    match action {
        Action::Sell(req) => post("/api/listings", &req)
            .await
            .map(|_| "Objet mis en vente.".into()),
        Action::Bid(id, unit_price) => {
            let resp = post(
                &format!("/api/listings/{id}/bid"),
                &BidRequest {
                    bidder: player,
                    unit_price,
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
                BidResult::Bought => Ok("Prix d'achat immédiat atteint : objet acheté.".into()),
            }
        }
        Action::Buyout(id) => post(
            &format!("/api/listings/{id}/buyout"),
            &PlayerRequest { player },
        )
        .await
        .map(|_| "Objet acheté.".into()),
        Action::Cancel(id) => post(
            &format!("/api/listings/{id}/cancel"),
            &PlayerRequest { player },
        )
        .await
        .map(|_| "Vente annulée.".into()),
    }
}

// ---- app ----

#[component]
fn App() -> Html {
    let catalog = use_state(Vec::<CatalogEntry>::new);
    let houses = use_state(Vec::<HouseView>::new);
    let players = use_state(Vec::<PlayerView>::new);
    let sink = use_state(|| Money::ZERO);
    let player = use_state(|| None::<PlayerRef>);
    let house = use_state(|| None::<HouseId>);
    let filter = use_state(|| Filter::All);
    let item_filter = use_state(|| None::<ItemType>);
    let listings = use_state(|| None::<ListingsResponse>);
    let message = use_state(|| None::<(bool, String)>);
    let reload = use_reducer(Reload::default);

    {
        let (catalog, houses, house, player, message) = (
            catalog.clone(),
            houses.clone(),
            house.clone(),
            player.clone(),
            message.clone(),
        );
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let loaded = async {
                    catalog.set(get("/api/catalog", vec![]).await?);
                    let all_houses: Vec<HouseView> = get("/api/houses", vec![]).await?;
                    house.set(all_houses.first().map(|h| h.id));
                    houses.set(all_houses);
                    let all_players: Vec<PlayerView> = get("/api/players", vec![]).await?;
                    player.set(all_players.first().map(|p| p.player.clone()));
                    Ok::<_, String>(())
                };
                if let Err(e) = loaded.await {
                    message.set(Some((false, e)));
                }
            });
        });
    }

    {
        let reload = reload.dispatcher();
        use_effect_with((), move |_| {
            let interval = Interval::new(REFRESH_MS, move || reload.dispatch(()));
            move || drop(interval)
        });
    }

    {
        let (players, sink, listings, message) = (
            players.clone(),
            sink.clone(),
            listings.clone(),
            message.clone(),
        );
        let deps = (
            reload.0,
            *house,
            (*player).clone(),
            *filter,
            (*item_filter).clone(),
        );
        use_effect_with(deps, move |(_, house, player, filter, item_filter)| {
            let (house, player, filter, item_filter) =
                (*house, player.clone(), *filter, item_filter.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let loaded = async {
                    players.set(get("/api/players", vec![]).await?);
                    sink.set(get("/api/sink", vec![]).await?);
                    let Some(house) = house else { return Ok(()) };
                    let mut query = Vec::new();
                    if let Some(item_type) = item_filter {
                        query.push(("item_type", item_type.0));
                    }
                    match (filter, player) {
                        (Filter::MySales, Some(me)) => query.push(("seller", me.0)),
                        (Filter::MyBids, Some(me)) => query.push(("high_bidder", me.0)),
                        _ => {}
                    }
                    listings.set(Some(
                        get(&format!("/api/houses/{house}/listings"), query).await?,
                    ));
                    Ok::<_, String>(())
                };
                if let Err(e) = loaded.await {
                    message.set(Some((false, e)));
                }
            });
        });
    }

    let on_action = {
        let (player, message, reload) = (player.clone(), message.clone(), reload.dispatcher());
        Callback::from(move |action: Action| {
            let Some(me) = (*player).clone() else { return };
            let (message, reload) = (message.clone(), reload.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let result = perform(action, me).await;
                message.set(Some(match result {
                    Ok(text) => (true, text),
                    Err(text) => (false, text),
                }));
                reload.dispatch(());
            });
        })
    };

    let on_player = {
        let player = player.clone();
        Callback::from(move |e: Event| player.set(Some(PlayerRef(select_value(&e)))))
    };
    let on_house = {
        let (house, houses) = (house.clone(), houses.clone());
        Callback::from(move |e: Event| {
            let value = select_value(&e);
            house.set(
                houses
                    .iter()
                    .find(|h| h.id.to_string() == value)
                    .map(|h| h.id),
            );
        })
    };
    let on_item_filter = {
        let item_filter = item_filter.clone();
        Callback::from(move |e: Event| {
            let value = select_value(&e);
            item_filter.set((!value.is_empty()).then_some(ItemType(value)));
        })
    };
    let tab = |value: Filter, label: &str| {
        let filter = filter.clone();
        let class = if *filter == value {
            "tab active"
        } else {
            "tab"
        };
        html! { <button class={class} onclick={move |_| filter.set(value)}>{ label }</button> }
    };

    let me = (*player).clone();
    let my_view = me
        .as_ref()
        .and_then(|me| players.iter().find(|p| &p.player == me).cloned());
    let current_house = house.and_then(|id| houses.iter().find(|h| h.id == id).cloned());

    html! {
        <main>
            <header>
                <h1>{ "Agora" }</h1>
                <label>{ "Joueur " }
                    <select onchange={on_player}>
                        { for players.iter().map(|p| html! {
                            <option value={p.player.to_string()} selected={Some(&p.player) == me.as_ref()}>{ p.player.to_string() }</option>
                        }) }
                    </select>
                </label>
                <label>{ "Hôtel des ventes " }
                    <select onchange={on_house}>
                        { for houses.iter().map(|h| html! {
                            <option value={h.id.to_string()} selected={Some(h.id) == *house}>
                                { format!("{} (commission {} %)", h.name, f64::from(h.commission) / 100.0) }
                            </option>
                        }) }
                    </select>
                </label>
                <span class="sink">{ format!("Or retiré du jeu : {}", money(*sink)) }</span>
            </header>

            if let Some((ok, text)) = (*message).clone() {
                <p class={if ok { "message ok" } else { "message error" }}>{ text }</p>
            }

            <div class="layout">
                <aside>
                    if let Some(view) = my_view.clone() {
                        <h2>{ "Mon personnage" }</h2>
                        <p>{ format!("Or : {}", money(view.gold)) }</p>
                        <ul>
                            { for view.items.iter().map(|i| html! {
                                <li>{ format!("{} x{}", item_name(&catalog, &i.item_type), i.quantity) }</li>
                            }) }
                        </ul>
                        if let Some(current_house) = current_house.clone() {
                            <SellForm
                                key={format!("{}-{}", view.player, current_house.id)}
                                house={current_house.clone()}
                                seller={view.player.clone()}
                                items={view.items.iter().map(|i| (i.item_type.clone(), item_name(&catalog, &i.item_type), i.quantity)).collect::<Vec<_>>()}
                                on_action={on_action.clone()}
                            />
                        }
                    }
                </aside>

                <section>
                    <div class="filters">
                        { tab(Filter::All, "Toutes les ventes") }
                        { tab(Filter::MySales, "Mes ventes") }
                        { tab(Filter::MyBids, "Mes enchères") }
                        <select onchange={on_item_filter}>
                            <option value="" selected={item_filter.is_none()}>{ "Tous les objets" }</option>
                            { for catalog.iter().map(|e| html! {
                                <option value={e.item_type.to_string()} selected={item_filter.as_ref() == Some(&e.item_type)}>{ e.name.clone() }</option>
                            }) }
                        </select>
                    </div>
                    if let (Some(response), Some(me)) = ((*listings).clone(), me.clone()) {
                        <p class="count">{ format!("{} vente(s)", response.page.total) }</p>
                        <table>
                            <thead>
                                <tr>
                                    <th>{ "Objet" }</th><th>{ "Qté" }</th><th>{ "Vendeur" }</th>
                                    <th>{ "Enchère / u" }</th><th>{ "Achat immédiat / u" }</th>
                                    <th>{ "Meilleur enchérisseur" }</th><th>{ "Temps restant" }</th><th></th>
                                </tr>
                            </thead>
                            <tbody>
                                { for response.page.items.iter().map(|l| html! {
                                    <ListingRow
                                        key={l.id.to_string()}
                                        listing={l.clone()}
                                        name={item_name(&catalog, &l.item.item_type)}
                                        now={response.now}
                                        me={me.clone()}
                                        on_action={on_action.clone()}
                                    />
                                }) }
                            </tbody>
                        </table>
                    }
                </section>
            </div>
        </main>
    }
}

// ---- listing row ----

#[derive(Properties, PartialEq)]
struct ListingRowProps {
    listing: Listing,
    name: String,
    now: Timestamp,
    me: PlayerRef,
    on_action: Callback<Action>,
}

#[component]
fn ListingRow(props: &ListingRowProps) -> Html {
    let listing = &props.listing;
    let minimum = listing.min_next_bid().unwrap_or(Money(u64::MAX));
    let bid = use_state(|| minimum.0.to_string());
    let id = listing.id;

    let on_bid_input = {
        let bid = bid.clone();
        Callback::from(move |e: InputEvent| bid.set(input_value(&e)))
    };
    let on_bid = {
        let (bid, on_action) = (bid.clone(), props.on_action.clone());
        Callback::from(move |_| {
            if let Ok(unit_price) = bid.parse::<u64>() {
                on_action.emit(Action::Bid(id, Money(unit_price)));
            }
        })
    };
    let on_buyout = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit(Action::Buyout(id)))
    };
    let on_cancel = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit(Action::Cancel(id)))
    };

    let mine = listing.seller == props.me;
    let leading = listing
        .high_bid
        .as_ref()
        .is_some_and(|b| b.bidder == props.me);

    html! {
        <tr class={classes!(mine.then_some("mine"), leading.then_some("leading"))}>
            <td>{ props.name.clone() }</td>
            <td>{ listing.item.quantity }</td>
            <td>{ listing.seller.to_string() }</td>
            <td>{ money(listing.current_unit_price()) }</td>
            <td>{ listing.buyout_unit_price.map_or_else(|| "-".to_string(), money) }</td>
            <td>{ listing.high_bid.as_ref().map_or_else(|| "-".to_string(), |b| b.bidder.to_string()) }</td>
            <td>{ time_left(props.now, listing.ends_at) }</td>
            <td class="actions">
                if mine {
                    if listing.high_bid.is_none() {
                        <button onclick={on_cancel}>{ "Annuler" }</button>
                    }
                } else {
                    if !leading {
                        <input type="number" min={minimum.0.to_string()} value={(*bid).clone()} oninput={on_bid_input} title="Prix unitaire en pc" />
                        <button onclick={on_bid}>{ "Enchérir" }</button>
                    }
                    if listing.buyout_unit_price.is_some() {
                        <button onclick={on_buyout}>{ "Acheter" }</button>
                    }
                }
            </td>
        </tr>
    }
}

// ---- sell form ----

#[derive(Properties, PartialEq)]
struct SellFormProps {
    house: HouseView,
    seller: PlayerRef,
    /// Item type, display name, quantity held.
    items: Vec<(ItemType, String, u32)>,
    on_action: Callback<Action>,
}

#[component]
fn SellForm(props: &SellFormProps) -> Html {
    let item = use_state(|| props.items.first().map(|(t, _, _)| t.clone()));
    let quantity = use_state(|| "1".to_string());
    let start = use_state(String::new);
    let buyout = use_state(String::new);
    let duration = use_state(|| props.house.durations.first().map_or(0, |d| d.secs));

    let text_input = |state: &UseStateHandle<String>| {
        let state = state.clone();
        Callback::from(move |e: InputEvent| state.set(input_value(&e)))
    };
    let on_item = {
        let item = item.clone();
        Callback::from(move |e: Event| item.set(Some(ItemType(select_value(&e)))))
    };
    let on_duration = {
        let duration = duration.clone();
        Callback::from(move |e: Event| duration.set(select_value(&e).parse().unwrap_or(0)))
    };
    let on_submit = {
        let (item, quantity, start, buyout, duration) = (
            item.clone(),
            quantity.clone(),
            start.clone(),
            buyout.clone(),
            duration.clone(),
        );
        let (house, seller, on_action) = (
            props.house.id,
            props.seller.clone(),
            props.on_action.clone(),
        );
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let Some(item_type) = (*item).clone() else {
                return;
            };
            let (Ok(quantity), Ok(start)) = (quantity.parse::<u32>(), start.parse::<u64>()) else {
                return;
            };
            let buyout = buyout.parse::<u64>().ok().map(Money);
            on_action.emit(Action::Sell(CreateListingRequest {
                house,
                seller: seller.clone(),
                item_type,
                quantity,
                start_unit_price: Money(start),
                buyout_unit_price: buyout,
                duration_secs: *duration,
            }));
        })
    };

    html! {
        <form class="sell" onsubmit={on_submit}>
            <h2>{ "Vendre" }</h2>
            <label>{ "Objet" }
                <select onchange={on_item}>
                    { for props.items.iter().map(|(t, name, held)| html! {
                        <option value={t.to_string()} selected={item.as_ref() == Some(t)}>{ format!("{name} ({held})") }</option>
                    }) }
                </select>
            </label>
            <label>{ "Quantité" }
                <input type="number" min="1" value={(*quantity).clone()} oninput={text_input(&quantity)} />
            </label>
            <label>{ "Mise à prix / u (pc)" }
                <input type="number" min="1" value={(*start).clone()} oninput={text_input(&start)} />
            </label>
            <label>{ "Achat immédiat / u (pc, optionnel)" }
                <input type="number" min="1" value={(*buyout).clone()} oninput={text_input(&buyout)} />
            </label>
            <label>{ "Durée" }
                <select onchange={on_duration}>
                    { for props.house.durations.iter().map(|d| html! {
                        <option value={d.secs.to_string()} selected={*duration == d.secs}>
                            { format!("{} min (dépôt x{})", d.secs / 60, d.deposit_multiplier) }
                        </option>
                    }) }
                </select>
            </label>
            <button type="submit">{ "Mettre en vente" }</button>
        </form>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
