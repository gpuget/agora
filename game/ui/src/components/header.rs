use agora_domain::{HouseId, Money, PlayerRef};
use game_dto::{HouseView, PlayerView};
use yew::prelude::*;

use crate::format::{money, select_value};

#[derive(Properties, PartialEq)]
pub struct HeaderProps {
    pub players: Vec<PlayerView>,
    pub houses: Vec<HouseView>,
    pub player: Option<PlayerRef>,
    pub house: Option<HouseId>,
    pub sink: Money,
    pub on_player: Callback<PlayerRef>,
    pub on_house: Callback<Option<HouseId>>,
}

#[component]
pub fn Header(props: &HeaderProps) -> Html {
    let on_player = {
        let on_player = props.on_player.clone();
        Callback::from(move |e: Event| on_player.emit(PlayerRef(select_value(&e))))
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
            <h1>{ "Agora" }</h1>
            <label>{ "Joueur " }
                <select onchange={on_player}>
                    { for props.players.iter().map(|p| html! {
                        <option value={p.player.to_string()} selected={Some(&p.player) == props.player.as_ref()}>{ p.player.to_string() }</option>
                    }) }
                </select>
            </label>
            <label>{ "Hôtel des ventes " }
                <select onchange={on_house}>
                    { for props.houses.iter().map(|h| html! {
                        <option value={h.id.to_string()} selected={Some(h.id) == props.house}>
                            { format!("{} (commission {} %)", h.name, f64::from(h.commission) / 100.0) }
                        </option>
                    }) }
                </select>
            </label>
            <span class="sink">{ format!("Or retiré du jeu : {}", money(props.sink)) }</span>
        </header>
    }
}
