use game_dto::{CatalogEntry, PlayerView};
use yew::prelude::*;

use crate::format::{item_name, money};

#[derive(Properties, PartialEq)]
pub struct CharacterPanelProps {
    pub view: PlayerView,
    pub catalog: Vec<CatalogEntry>,
}

#[component]
pub fn CharacterPanel(props: &CharacterPanelProps) -> Html {
    html! {
        <>
            <h2>{ "My character" }</h2>
            <p>{ format!("Gold: {}", money(props.view.gold)) }</p>
            <ul>
                { for props.view.items.iter().map(|i| html! {
                    <li>{ format!("{} x{}", item_name(&props.catalog, &i.item_type), i.quantity) }</li>
                }) }
            </ul>
        </>
    }
}
