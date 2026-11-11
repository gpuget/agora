use tcg_dto::{CardEntry, CollectorView};
use yew::prelude::*;

use crate::format::{card_entry, card_name, dust, rarity_class, rarity_label};

#[derive(Properties, PartialEq)]
pub struct CollectionPanelProps {
    pub view: CollectorView,
    pub catalog: Vec<CardEntry>,
}

#[component]
pub fn CollectionPanel(props: &CollectionPanelProps) -> Html {
    html! {
        <>
            <h2>{ "My collection" }</h2>
            <p>{ format!("Dust: {}", dust(props.view.dust)) }</p>
            <ul>
                { for props.view.cards.iter().map(|c| {
                    let rarity = card_entry(&props.catalog, &c.card).map(|e| e.rarity);
                    html! {
                        <li class={rarity.map(rarity_class)}>
                            { format!("{} x{}", card_name(&props.catalog, &c.card), c.copies) }
                            if let Some(rarity) = rarity { { format!(" ({})", rarity_label(rarity)) } }
                        </li>
                    }
                }) }
            </ul>
        </>
    }
}
