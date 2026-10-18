use agora_domain::PlayerRef;
use tcg_dto::{CardEntry, ListingsResponse};
use yew::prelude::*;

use super::ListingRow;
use crate::api::Action;
use crate::format::{card_entry, card_name};

#[derive(Properties, PartialEq)]
pub struct ListingsTableProps {
    pub response: ListingsResponse,
    pub catalog: Vec<CardEntry>,
    pub me: PlayerRef,
    pub on_action: Callback<Action>,
}

#[component]
pub fn ListingsTable(props: &ListingsTableProps) -> Html {
    let response = &props.response;
    html! {
        <>
            <p class="count">{ format!("{} vente(s)", response.page.total) }</p>
            <table>
                <thead>
                    <tr>
                        <th>{ "Carte" }</th><th>{ "Rareté" }</th><th>{ "Vendeur" }</th>
                        <th>{ "Enchère" }</th><th>{ "Achat immédiat" }</th>
                        <th>{ "Meilleur enchérisseur" }</th><th>{ "Temps restant" }</th><th></th>
                    </tr>
                </thead>
                <tbody>
                    { for response.page.items.iter().map(|l| html! {
                        <ListingRow
                            key={l.id.to_string()}
                            listing={l.clone()}
                            name={card_name(&props.catalog, &l.item.item_type)}
                            rarity={card_entry(&props.catalog, &l.item.item_type).map(|e| e.rarity)}
                            now={response.now}
                            me={props.me.clone()}
                            on_action={props.on_action.clone()}
                        />
                    }) }
                </tbody>
            </table>
        </>
    }
}
