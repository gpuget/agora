use std::rc::Rc;

use agora_domain::{HouseId, ItemType, Money, PlayerRef};
use game_dto::{CatalogEntry, HouseView, ListingsResponse, PlayerView};
use gloo_timers::callback::Interval;
use yew::prelude::*;

use crate::api::{Action, get, perform};
use crate::components::{
    CharacterPanel, Filter, FilterBar, Header, ListingsTable, MessageBanner, SellForm, SellItem,
};
use crate::format::{base_value, is_commodity, item_name};

const REFRESH_MS: u32 = 1_000;

/// Bumped to trigger a refresh.
#[derive(Default, PartialEq)]
struct Reload(u32);

impl Reducible for Reload {
    type Action = ();

    fn reduce(self: Rc<Self>, _: ()) -> Rc<Self> {
        Rc::new(Reload(self.0.wrapping_add(1)))
    }
}

#[component]
pub fn App() -> Html {
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
        Callback::from(move |p: PlayerRef| player.set(Some(p)))
    };
    let on_house = {
        let house = house.clone();
        Callback::from(move |id: Option<HouseId>| house.set(id))
    };
    let on_filter = {
        let filter = filter.clone();
        Callback::from(move |value: Filter| filter.set(value))
    };
    let on_item_filter = {
        let item_filter = item_filter.clone();
        Callback::from(move |value: Option<ItemType>| item_filter.set(value))
    };

    let me = (*player).clone();
    let my_view = me
        .as_ref()
        .and_then(|me| players.iter().find(|p| &p.player == me).cloned());
    let current_house = house.and_then(|id| houses.iter().find(|h| h.id == id).cloned());

    html! {
        <main>
            <Header
                players={(*players).clone()}
                houses={(*houses).clone()}
                player={me.clone()}
                house={*house}
                sink={*sink}
                {on_player}
                {on_house}
            />

            <MessageBanner message={(*message).clone()} />

            <div class="layout">
                <aside>
                    if let Some(view) = my_view.clone() {
                        <CharacterPanel view={view.clone()} catalog={(*catalog).clone()} />
                        if let Some(current_house) = current_house.clone() {
                            <SellForm
                                key={format!("{}-{}", view.player, current_house.id)}
                                house={current_house.clone()}
                                seller={view.player.clone()}
                                items={view.items.iter().map(|i| SellItem {
                                    item_type: i.item_type.clone(),
                                    name: item_name(&catalog, &i.item_type),
                                    held: i.quantity,
                                    base_value: base_value(&catalog, &i.item_type),
                                    commodity: is_commodity(&catalog, &i.item_type),
                                }).collect::<Vec<_>>()}
                                on_action={on_action.clone()}
                            />
                        }
                    }
                </aside>

                <section>
                    <FilterBar
                        filter={*filter}
                        item_filter={(*item_filter).clone()}
                        catalog={(*catalog).clone()}
                        {on_filter}
                        {on_item_filter}
                    />
                    if let (Some(response), Some(me)) = ((*listings).clone(), me.clone()) {
                        <ListingsTable
                            {response}
                            catalog={(*catalog).clone()}
                            {me}
                            on_action={on_action.clone()}
                        />
                    }
                </section>
            </div>
        </main>
    }
}
