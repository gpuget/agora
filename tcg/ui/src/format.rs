use agora_domain::{ItemType, Money, Timestamp};
use tcg_dto::{CardEntry, Rarity};
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

pub fn dust(amount: Money) -> String {
    format!("{} dust", amount.0)
}

pub fn time_left(now: Timestamp, end: Timestamp) -> String {
    let secs = now.until(end).as_secs();
    if secs == 0 {
        "terminée".into()
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

pub fn card_entry<'a>(catalog: &'a [CardEntry], card: &ItemType) -> Option<&'a CardEntry> {
    catalog.iter().find(|e| &e.card == card)
}

pub fn card_name(catalog: &[CardEntry], card: &ItemType) -> String {
    card_entry(catalog, card).map_or_else(|| card.to_string(), |e| e.name.clone())
}

pub fn rarity_label(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Common => "Commune",
        Rarity::Rare => "Rare",
        Rarity::Epic => "Épique",
        Rarity::Legendary => "Légendaire",
    }
}

/// CSS class of a rarity, defined in `index.html`.
pub fn rarity_class(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Common => "common",
        Rarity::Rare => "rare",
        Rarity::Epic => "epic",
        Rarity::Legendary => "legendary",
    }
}

pub fn input_value(e: &InputEvent) -> String {
    e.target_unchecked_into::<HtmlInputElement>().value()
}

pub fn select_value(e: &Event) -> String {
    e.target_unchecked_into::<HtmlSelectElement>().value()
}
