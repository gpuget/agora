use agora_domain::{Listing, Money, PlayerRef, Timestamp};
use yew::prelude::*;

use crate::api::Action;
use crate::format::{input_value, money, select_value, time_left, unit_price};

#[derive(Properties, PartialEq)]
pub struct ListingRowProps {
    pub listing: Listing,
    pub name: String,
    pub now: Timestamp,
    pub me: PlayerRef,
    pub on_action: Callback<Action>,
}

#[component]
pub fn ListingRow(props: &ListingRowProps) -> Html {
    let listing = &props.listing;
    let minimum = listing.min_next_bid().unwrap_or(Money(u64::MAX));
    let bid = use_state(|| minimum.0.to_string());
    let total_mode = use_state(|| false);
    let quantity = u64::from(listing.item.quantity);
    let id = listing.id;

    let on_bid_input = {
        let bid = bid.clone();
        Callback::from(move |e: InputEvent| bid.set(input_value(&e)))
    };
    let on_bid = {
        let (bid, total_mode, on_action) =
            (bid.clone(), total_mode.clone(), props.on_action.clone());
        Callback::from(move |_| {
            if let Some(unit_price) = unit_price(&bid, quantity, *total_mode) {
                on_action.emit(Action::Bid(id, Money(unit_price)));
            }
        })
    };
    let on_bid_mode = {
        let (bid, total_mode) = (bid.clone(), total_mode.clone());
        Callback::from(move |e: Event| {
            let total = select_value(&e) == "total";
            let shown = if total {
                minimum.checked_mul(quantity).unwrap_or(minimum)
            } else {
                minimum
            };
            bid.set(shown.0.to_string());
            total_mode.set(total);
        })
    };
    let on_buyout = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit(Action::Buyout(id)))
    };
    let on_cancel = {
        let on_action = props.on_action.clone();
        Callback::from(move |_| on_action.emit(Action::Cancel(id)))
    };

    let total = |unit: Option<Money>| {
        unit.and_then(|u| u.checked_mul(quantity))
            .map_or_else(|| "-".to_string(), money)
    };

    let mine = listing.seller == props.me;
    let leading = listing
        .high_bid
        .as_ref()
        .is_some_and(|b| b.bidder == props.me);

    html! {
        <tr class={classes!(mine.then_some("mine"), leading.then_some("leading"))}>
            <td>{ props.name.clone() }</td>
            <td>{ listing.item.quantity }</td>
            <td>{ listing.seller.to_string() }</td>
            <td>{ money(listing.current_unit_price()) }</td>
            <td>{ total(Some(listing.current_unit_price())) }</td>
            <td>{ listing.buyout_unit_price.map_or_else(|| "-".to_string(), money) }</td>
            <td>{ total(listing.buyout_unit_price) }</td>
            <td>{ listing.high_bid.as_ref().map_or_else(|| "-".to_string(), |b| b.bidder.to_string()) }</td>
            <td>{ time_left(props.now, listing.ends_at) }</td>
            <td class="actions">
                if mine {
                    if listing.high_bid.is_none() {
                        <button onclick={on_cancel}>{ "Annuler" }</button>
                    }
                } else {
                    if !leading {
                        <select onchange={on_bid_mode} title="Saisie du prix">
                            <option value="unit" selected={!*total_mode}>{ "/ u" }</option>
                            <option value="total" selected={*total_mode}>{ "lot" }</option>
                        </select>
                        <input type="number" min="1" value={(*bid).clone()} oninput={on_bid_input} title={if *total_mode { "Prix du lot en pc" } else { "Prix unitaire en pc" }} />
                        <button onclick={on_bid}>{ "Enchérir" }</button>
                    }
                    if listing.buyout_unit_price.is_some() {
                        <button onclick={on_buyout}>{ "Acheter" }</button>
                    }
                }
            </td>
        </tr>
    }
}
