use std::rc::Rc;

use agora_domain::{HouseId, ItemType, Money, PlayerRef};
use gloo_timers::callback::Interval;
use tcg_dto::{CardEntry, CollectorView, HouseView, ListingsResponse};
use yew::prelude::*;

use crate::api::{Action, get, perform};
use crate::components::{
    CollectionPanel, Filter, FilterBar, Header, ListingsTable, MessageBanner, SellCard, SellForm,
};
use crate::format::{card_entry, card_name};

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
    let catalog = use_state(Vec::<CardEntry>::new);
    let houses = use_state(Vec::<HouseView>::new);
    let collectors = use_state(Vec::<CollectorView>::new);
    let sink = use_state(|| Money::ZERO);
    let collector = use_state(|| None::<PlayerRef>);
    let house = use_state(|| None::<HouseId>);
    let filter = use_state(|| Filter::All);
    let card_filter = use_state(|| None::<ItemType>);
    let listings = use_state(|| None::<ListingsResponse>);
    let message = use_state(|| None::<(bool, String)>);
    let reload = use_reducer(Reload::default);

    {
        let (catalog, houses, house, collector, message) = (
            catalog.clone(),
            houses.clone(),
            house.clone(),
            collector.clone(),
            message.clone(),
        );
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let loaded = async {
                    catalog.set(get("/api/catalog", vec![]).await?);
                    let all_houses: Vec<HouseView> = get("/api/houses", vec![]).await?;
                    house.set(all_houses.first().map(|h| h.id));
                    houses.set(all_houses);
                    let all: Vec<CollectorView> = get("/api/collectors", vec![]).await?;
                    collector.set(all.first().map(|c| c.collector.clone()));
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
        let (collectors, sink, listings, message) = (
            collectors.clone(),
            sink.clone(),
            listings.clone(),
            message.clone(),
        );
        let deps = (
            reload.0,
            *house,
            (*collector).clone(),
            *filter,
            (*card_filter).clone(),
        );
        use_effect_with(deps, move |(_, house, collector, filter, card_filter)| {
            let (house, collector, filter, card_filter) =
                (*house, collector.clone(), *filter, card_filter.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let loaded = async {
                    collectors.set(get("/api/collectors", vec![]).await?);
                    sink.set(get("/api/sink", vec![]).await?);
                    let Some(house) = house else { return Ok(()) };
                    let mut query = Vec::new();
                    if let Some(card) = card_filter {
                        query.push(("card", card.0));
                    }
                    match (filter, collector) {
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
        let (collector, message, reload) =
            (collector.clone(), message.clone(), reload.dispatcher());
        Callback::from(move |action: Action| {
            let Some(me) = (*collector).clone() else {
                return;
            };
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

    let on_collector = {
        let collector = collector.clone();
        Callback::from(move |c: PlayerRef| collector.set(Some(c)))
    };
    let on_house = {
        let house = house.clone();
        Callback::from(move |id: Option<HouseId>| house.set(id))
    };
    let on_filter = {
        let filter = filter.clone();
        Callback::from(move |value: Filter| filter.set(value))
    };
    let on_card_filter = {
        let card_filter = card_filter.clone();
        Callback::from(move |value: Option<ItemType>| card_filter.set(value))
    };

    let me = (*collector).clone();
    let my_view = me
        .as_ref()
        .and_then(|me| collectors.iter().find(|c| &c.collector == me).cloned());
    let current_house = house.and_then(|id| houses.iter().find(|h| h.id == id).cloned());

    html! {
        <main>
            <Header
                collectors={(*collectors).clone()}
                houses={(*houses).clone()}
                collector={me.clone()}
                house={*house}
                sink={*sink}
                {on_collector}
                {on_house}
            />

            <MessageBanner message={(*message).clone()} />

            <div class="layout">
                <aside>
                    if let Some(view) = my_view.clone() {
                        <CollectionPanel view={view.clone()} catalog={(*catalog).clone()} />
                        if let Some(current_house) = current_house.clone() {
                            <SellForm
                                key={format!("{}-{}", view.collector, current_house.id)}
                                house={current_house.clone()}
                                seller={view.collector.clone()}
                                cards={view.cards.iter().map(|c| SellCard {
                                    card: c.card.clone(),
                                    name: card_name(&catalog, &c.card),
                                    copies: c.copies,
                                    base_value: card_entry(&catalog, &c.card).map_or(Money::ZERO, |e| e.base_value),
                                }).collect::<Vec<_>>()}
                                on_action={on_action.clone()}
                            />
                        }
                    }
                </aside>

                <section>
                    <FilterBar
                        filter={*filter}
                        item_filter={(*card_filter).clone()}
                        catalog={(*catalog).clone()}
                        {on_filter}
                        on_item_filter={on_card_filter}
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
