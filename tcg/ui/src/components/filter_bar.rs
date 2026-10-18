use agora_domain::ItemType;
use tcg_dto::CardEntry;
use yew::prelude::*;

use crate::format::select_value;

#[derive(Clone, Copy, PartialEq)]
pub enum Filter {
    All,
    MySales,
    MyBids,
}

#[derive(Properties, PartialEq)]
pub struct FilterBarProps {
    pub filter: Filter,
    pub item_filter: Option<ItemType>,
    pub catalog: Vec<CardEntry>,
    pub on_filter: Callback<Filter>,
    pub on_item_filter: Callback<Option<ItemType>>,
}

#[component]
pub fn FilterBar(props: &FilterBarProps) -> Html {
    let tab = |value: Filter, label: &str| {
        let on_filter = props.on_filter.clone();
        let class = if props.filter == value {
            "tab active"
        } else {
            "tab"
        };
        html! { <button class={class} onclick={move |_| on_filter.emit(value)}>{ label }</button> }
    };
    let on_item_filter = {
        let on_item_filter = props.on_item_filter.clone();
        Callback::from(move |e: Event| {
            let value = select_value(&e);
            on_item_filter.emit((!value.is_empty()).then_some(ItemType(value)));
        })
    };

    html! {
        <div class="filters">
            { tab(Filter::All, "Toutes les ventes") }
            { tab(Filter::MySales, "Mes ventes") }
            { tab(Filter::MyBids, "Mes enchères") }
            <select onchange={on_item_filter}>
                <option value="" selected={props.item_filter.is_none()}>{ "Toutes les cartes" }</option>
                { for props.catalog.iter().map(|e| html! {
                    <option value={e.card.to_string()} selected={props.item_filter.as_ref() == Some(&e.card)}>{ e.name.clone() }</option>
                }) }
            </select>
        </div>
    }
}
