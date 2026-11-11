use agora_domain::{Listing, Money, PlayerRef, Timestamp};
use tcg_dto::Rarity;
use yew::prelude::*;

use crate::api::Action;
use crate::format::{dust, input_value, rarity_class, rarity_label, time_left};

#[derive(Properties, PartialEq)]
pub struct ListingRowProps {
    pub listing: Listing,
    pub name: String,
    pub rarity: Option<Rarity>,
    pub now: Timestamp,
    pub me: PlayerRef,
    pub on_action: Callback<Action>,
}

/// A listing holds a single card: its unit prices are the prices of the card.
#[component]
pub fn ListingRow(props: &ListingRowProps) -> Html {
    let listing = &props.listing;
    let minimum = listing.min_next_bid().unwrap_or(Money(u64::MAX));
    let bid = use_state(|| minimum.0.to_string());
    let id = listing.id;

    let on_bid_input = {
        let bid = bid.clone();
        Callback::from(move |e: InputEvent| bid.set(input_value(&e)))
    };
    let on_bid = {
        let (bid, on_action) = (bid.clone(), props.on_action.clone());
        Callback::from(move |_| {
            if let Ok(price) = bid.parse::<u64>() {
                on_action.emit(Action::Bid(id, Money(price)));
            }
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

    let mine = listing.seller == props.me;
    let leading = listing
        .high_bid
        .as_ref()
        .is_some_and(|b| b.bidder == props.me);

    html! {
        <tr class={classes!(mine.then_some("mine"), leading.then_some("leading"))}>
            <td class={props.rarity.map(rarity_class)}>{ props.name.clone() }</td>
            <td>{ props.rarity.map_or("-", rarity_label) }</td>
            <td>{ listing.seller.to_string() }</td>
            <td>{ dust(listing.current_unit_price()) }</td>
            <td>{ listing.buyout_unit_price.map_or_else(|| "-".to_string(), dust) }</td>
            <td>{ listing.high_bid.as_ref().map_or_else(|| "-".to_string(), |b| b.bidder.to_string()) }</td>
            <td>{ time_left(props.now, listing.ends_at) }</td>
            <td class="actions">
                if mine {
                    if listing.high_bid.is_none() {
                        <button onclick={on_cancel}>{ "Cancel" }</button>
                    }
                } else {
                    if !leading {
                        <input type="number" min="1" value={(*bid).clone()} oninput={on_bid_input} title="Bid in dust" />
                        <button onclick={on_bid}>{ "Bid" }</button>
                    }
                    if listing.buyout_unit_price.is_some() {
                        <button onclick={on_buyout}>{ "Buy" }</button>
                    }
                }
            </td>
        </tr>
    }
}
