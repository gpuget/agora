use agora_domain::{ItemType, Money, PlayerRef};
use game_dto::{CreateListingRequest, HouseView};
use yew::prelude::*;

use crate::api::Action;
use crate::format::{input_value, select_value};

#[derive(Properties, PartialEq)]
pub struct SellFormProps {
    pub house: HouseView,
    pub seller: PlayerRef,
    /// Item type, display name, quantity held.
    pub items: Vec<(ItemType, String, u32)>,
    pub on_action: Callback<Action>,
}

#[component]
pub fn SellForm(props: &SellFormProps) -> Html {
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
