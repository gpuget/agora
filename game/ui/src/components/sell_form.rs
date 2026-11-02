use agora_domain::{Bps, ItemType, Money, PlayerRef};
use game_dto::{CreateListingRequest, HouseView};
use yew::prelude::*;

use crate::api::Action;
use crate::format::{input_value, money, select_value, unit_price};

#[derive(Clone, PartialEq)]
pub struct SellItem {
    pub item_type: ItemType,
    pub name: String,
    pub held: u32,
    pub base_value: Money,
    /// Sold at a fixed price only, and bought in part.
    pub commodity: bool,
}

#[derive(Properties, PartialEq)]
pub struct SellFormProps {
    pub house: HouseView,
    pub seller: PlayerRef,
    pub items: Vec<SellItem>,
    pub on_action: Callback<Action>,
}

#[component]
pub fn SellForm(props: &SellFormProps) -> Html {
    let item = use_state(|| props.items.first().map(|i| i.item_type.clone()));
    let quantity = use_state(|| "1".to_string());
    let start = use_state(String::new);
    let buyout = use_state(String::new);
    let total_mode = use_state(|| false);
    let duration = use_state(|| props.house.durations.first().map_or(0, |d| d.secs));

    let text_input = |state: &UseStateHandle<String>| {
        let state = state.clone();
        Callback::from(move |e: InputEvent| state.set(input_value(&e)))
    };
    let on_item = {
        let item = item.clone();
        Callback::from(move |e: Event| item.set(Some(ItemType(select_value(&e)))))
    };
    let on_price_mode = {
        let total_mode = total_mode.clone();
        Callback::from(move |e: Event| total_mode.set(select_value(&e) == "total"))
    };
    let on_duration = {
        let duration = duration.clone();
        Callback::from(move |e: Event| duration.set(select_value(&e).parse().unwrap_or(0)))
    };
    let commodity = props
        .items
        .iter()
        .find(|i| Some(&i.item_type) == item.as_ref())
        .is_some_and(|i| i.commodity);
    let on_submit = {
        let (item, quantity, start, buyout, duration, total_mode) = (
            item.clone(),
            quantity.clone(),
            start.clone(),
            buyout.clone(),
            duration.clone(),
            total_mode.clone(),
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
            let Ok(quantity) = quantity.parse::<u32>() else {
                return;
            };
            let total = *total_mode;
            let buyout = unit_price(&buyout, u64::from(quantity), total).map(Money);
            // A commodity has a fixed price: the start price only has to stay below it.
            let start = if commodity {
                let Some(price) = buyout else {
                    return;
                };
                price.0
            } else {
                let Some(start) = unit_price(&start, u64::from(quantity), total) else {
                    return;
                };
                start
            };
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

    let preview = |raw: &str| {
        let unit = unit_price(raw, quantity.parse().ok()?, *total_mode)?;
        Some(format!("= {} / u", money(Money(unit))))
    };
    let (start_preview, buyout_preview) = if *total_mode {
        (preview(&start), preview(&buyout))
    } else {
        (None, None)
    };

    // Same formula as the auction core: base value x quantity x rate x duration multiplier.
    let deposit = (|| {
        let base_value = props
            .items
            .iter()
            .find(|i| Some(&i.item_type) == item.as_ref())?
            .base_value;
        let quantity = quantity.parse::<u64>().ok()?;
        let multiplier = props
            .house
            .durations
            .iter()
            .find(|d| d.secs == *duration)?
            .deposit_multiplier;
        base_value
            .checked_mul(quantity)?
            .apply_bps(Bps(props.house.deposit))
            .checked_mul(u64::from(multiplier))
    })();

    html! {
        <form class="sell" onsubmit={on_submit}>
            <h2>{ "Vendre" }</h2>
            <label>{ "Objet" }
                <select onchange={on_item}>
                    { for props.items.iter().map(|i| html! {
                        <option value={i.item_type.to_string()} selected={item.as_ref() == Some(&i.item_type)}>{ format!("{} ({})", i.name, i.held) }</option>
                    }) }
                </select>
            </label>
            <label>{ "Quantité" }
                <input type="number" min="1" value={(*quantity).clone()} oninput={text_input(&quantity)} />
            </label>
            <label>{ "Saisie des prix" }
                <select onchange={on_price_mode}>
                    <option value="unit" selected={!*total_mode}>{ "Prix unitaire" }</option>
                    <option value="total" selected={*total_mode}>{ "Prix du lot" }</option>
                </select>
            </label>
            if !commodity {
                <label>{ if *total_mode { "Mise à prix du lot (pc)" } else { "Mise à prix / u (pc)" } }
                    <input type="number" min="1" value={(*start).clone()} oninput={text_input(&start)} />
                </label>
                if let Some(text) = start_preview { <p class="deposit">{ text }</p> }
            }
            <label>{ match (commodity, *total_mode) {
                (true, true) => "Prix du lot (pc)",
                (true, false) => "Prix / u (pc)",
                (false, true) => "Achat immédiat du lot (pc, optionnel)",
                (false, false) => "Achat immédiat / u (pc, optionnel)",
            } }
                <input type="number" min="1" value={(*buyout).clone()} oninput={text_input(&buyout)} />
            </label>
            if let Some(text) = buyout_preview { <p class="deposit">{ text }</p> }
            <label>{ "Durée" }
                <select onchange={on_duration}>
                    { for props.house.durations.iter().map(|d| html! {
                        <option value={d.secs.to_string()} selected={*duration == d.secs}>
                            { format!("{} min (dépôt x{})", d.secs / 60, d.deposit_multiplier) }
                        </option>
                    }) }
                </select>
            </label>
            <p class="deposit">
                { format!("Dépôt : {}", deposit.map_or_else(|| "-".to_string(), money)) }
            </p>
            <button type="submit">{ "Mettre en vente" }</button>
        </form>
    }
}
