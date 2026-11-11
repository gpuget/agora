use agora_domain::{Bps, ItemType, Money, PlayerRef};
use tcg_dto::{CreateListingRequest, HouseView};
use yew::prelude::*;

use crate::api::Action;
use crate::format::{dust, input_value, select_value};

#[derive(Clone, PartialEq)]
pub struct SellCard {
    pub card: ItemType,
    pub name: String,
    pub copies: u32,
    pub base_value: Money,
}

#[derive(Properties, PartialEq)]
pub struct SellFormProps {
    pub house: HouseView,
    pub seller: PlayerRef,
    pub cards: Vec<SellCard>,
    pub on_action: Callback<Action>,
}

/// Puts one copy of a card up for sale.
#[component]
pub fn SellForm(props: &SellFormProps) -> Html {
    let card = use_state(|| props.cards.first().map(|c| c.card.clone()));
    let start = use_state(String::new);
    let buyout = use_state(String::new);
    let duration = use_state(|| props.house.durations.first().map_or(0, |d| d.secs));

    let text_input = |state: &UseStateHandle<String>| {
        let state = state.clone();
        Callback::from(move |e: InputEvent| state.set(input_value(&e)))
    };
    let on_card = {
        let card = card.clone();
        Callback::from(move |e: Event| card.set(Some(ItemType(select_value(&e)))))
    };
    let on_duration = {
        let duration = duration.clone();
        Callback::from(move |e: Event| duration.set(select_value(&e).parse().unwrap_or(0)))
    };
    let on_submit = {
        let (card, start, buyout, duration) = (
            card.clone(),
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
            let Some(card) = (*card).clone() else {
                return;
            };
            let Ok(start) = start.parse::<u64>() else {
                return;
            };
            let buyout = buyout.parse::<u64>().ok().map(Money);
            on_action.emit(Action::Sell(CreateListingRequest {
                house,
                seller: seller.clone(),
                card,
                start_price: Money(start),
                buyout_price: buyout,
                duration_secs: *duration,
            }));
        })
    };

    // Same formula as the auction core for a single card: base value x rate x duration
    // multiplier.
    let deposit = (|| {
        let base_value = props
            .cards
            .iter()
            .find(|c| Some(&c.card) == card.as_ref())?
            .base_value;
        let multiplier = props
            .house
            .durations
            .iter()
            .find(|d| d.secs == *duration)?
            .deposit_multiplier;
        base_value
            .apply_bps(Bps(props.house.deposit))
            .checked_mul(u64::from(multiplier))
    })();

    html! {
        <form class="sell" onsubmit={on_submit}>
            <h2>{ "Sell a card" }</h2>
            <label>{ "Card" }
                <select onchange={on_card}>
                    { for props.cards.iter().map(|c| html! {
                        <option value={c.card.to_string()} selected={card.as_ref() == Some(&c.card)}>{ format!("{} (x{})", c.name, c.copies) }</option>
                    }) }
                </select>
            </label>
            <label>{ "Start price (dust)" }
                <input type="number" min="1" value={(*start).clone()} oninput={text_input(&start)} />
            </label>
            <label>{ "Buyout price (dust, optional)" }
                <input type="number" min="1" value={(*buyout).clone()} oninput={text_input(&buyout)} />
            </label>
            <label>{ "Duration" }
                <select onchange={on_duration}>
                    { for props.house.durations.iter().map(|d| html! {
                        <option value={d.secs.to_string()} selected={*duration == d.secs}>
                            { format!("{} min (deposit x{})", d.secs / 60, d.deposit_multiplier) }
                        </option>
                    }) }
                </select>
            </label>
            <p class="deposit">
                { format!("Deposit: {}", deposit.map_or_else(|| "-".to_string(), dust)) }
            </p>
            <button type="submit">{ "List for sale" }</button>
        </form>
    }
}
