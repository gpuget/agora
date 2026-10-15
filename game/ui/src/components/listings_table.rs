use agora_domain::PlayerRef;
use game_dto::{CatalogEntry, ListingsResponse};
use yew::prelude::*;

use super::ListingRow;
use crate::api::Action;
use crate::format::item_name;

#[derive(Properties, PartialEq)]
pub struct ListingsTableProps {
    pub response: ListingsResponse,
    pub catalog: Vec<CatalogEntry>,
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
                        <th>{ "Objet" }</th><th>{ "Qté" }</th><th>{ "Vendeur" }</th>
                        <th>{ "Enchère / u" }</th><th>{ "Achat immédiat / u" }</th>
                        <th>{ "Meilleur enchérisseur" }</th><th>{ "Temps restant" }</th><th></th>
                    </tr>
                </thead>
                <tbody>
                    { for response.page.items.iter().map(|l| html! {
                        <ListingRow
                            key={l.id.to_string()}
                            listing={l.clone()}
                            name={item_name(&props.catalog, &l.item.item_type)}
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
