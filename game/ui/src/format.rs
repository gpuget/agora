use agora_domain::{ItemType, Money, Timestamp};
use game_dto::CatalogEntry;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

/// Amounts are in copper: 1 gold = 100 silver = 10 000 copper.
pub fn money(amount: Money) -> String {
    let (gold, silver, copper) = (amount.0 / 10_000, amount.0 / 100 % 100, amount.0 % 100);
    let mut parts = Vec::new();
    if gold > 0 {
        parts.push(format!("{gold} po"));
    }
    if silver > 0 {
        parts.push(format!("{silver} pa"));
    }
    if copper > 0 || parts.is_empty() {
        parts.push(format!("{copper} pc"));
    }
    parts.join(" ")
}

pub fn time_left(now: Timestamp, end: Timestamp) -> String {
    let secs = now.until(end).as_secs();
    if secs == 0 {
        "terminée".into()
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

pub fn item_name(catalog: &[CatalogEntry], item_type: &ItemType) -> String {
    catalog
        .iter()
        .find(|e| &e.item_type == item_type)
        .map_or_else(|| item_type.to_string(), |e| e.name.clone())
}

pub fn input_value(e: &InputEvent) -> String {
    e.target_unchecked_into::<HtmlInputElement>().value()
}

pub fn select_value(e: &Event) -> String {
    e.target_unchecked_into::<HtmlSelectElement>().value()
}
