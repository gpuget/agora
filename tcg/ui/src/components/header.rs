use agora_domain::{HouseId, Money, PlayerRef};
use tcg_dto::{CollectorView, HouseView};
use yew::prelude::*;

use crate::format::{dust, select_value};

#[derive(Properties, PartialEq)]
pub struct HeaderProps {
    pub collectors: Vec<CollectorView>,
    pub houses: Vec<HouseView>,
    pub collector: Option<PlayerRef>,
    pub house: Option<HouseId>,
    pub sink: Money,
    pub on_collector: Callback<PlayerRef>,
    pub on_house: Callback<Option<HouseId>>,
}

#[component]
pub fn Header(props: &HeaderProps) -> Html {
    let on_collector = {
        let on_collector = props.on_collector.clone();
        Callback::from(move |e: Event| on_collector.emit(PlayerRef(select_value(&e))))
    };
    let on_house = {
        let (houses, on_house) = (props.houses.clone(), props.on_house.clone());
        Callback::from(move |e: Event| {
            let value = select_value(&e);
            on_house.emit(
                houses
                    .iter()
                    .find(|h| h.id.to_string() == value)
                    .map(|h| h.id),
            );
        })
    };

    html! {
        <header>
            <h1>{ "Agora TCG" }</h1>
            <label>{ "Collector " }
                <select onchange={on_collector}>
                    { for props.collectors.iter().map(|c| html! {
                        <option value={c.collector.to_string()} selected={Some(&c.collector) == props.collector.as_ref()}>{ c.collector.to_string() }</option>
                    }) }
                </select>
            </label>
            <label>{ "Auction house " }
                <select onchange={on_house}>
                    { for props.houses.iter().map(|h| html! {
                        <option value={h.id.to_string()} selected={Some(h.id) == props.house}>
                            { format!("{} (commission {}%)", h.name, f64::from(h.commission) / 100.0) }
                        </option>
                    }) }
                </select>
            </label>
            <span class="sink">{ format!("Dust taken out of the game: {}", dust(props.sink)) }</span>
        </header>
    }
}
