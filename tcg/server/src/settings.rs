//! Server settings, read from a TOML file. Every top-level key is optional except `catalog`.
//! Inside the rarity table, a house or a catalog entry, every key is required.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use agora_domain::{Bps, DurationOption, HouseConfig, Money};
use serde::Deserialize;
use tcg_domain::{CardEntry, Rarity};

/// Settings file read when `TCG_CONFIG` is not set, relative to the working directory.
pub const DEFAULT_PATH: &str = "tcg.toml";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Listen address of the HTTP server. `BIND_ADDR` overrides it.
    pub bind_addr: String,
    /// Delay between two settlements of expired auctions.
    pub settle_every_secs: u64,
    /// Log level per target. The key `default` sets the level of every other target.
    /// `RUST_LOG` overrides the whole table.
    pub log: BTreeMap<String, String>,
    /// Value of one card in dust, per rarity.
    pub rarity: RaritySettings,
    /// Auction houses opened at startup, one `[[house]]` table each.
    #[serde(rename = "house")]
    pub houses: Vec<HouseSettings>,
    /// Cards the game knows, one inline table each in the `catalog` array. No default: the
    /// file must list them.
    pub catalog: Vec<CardSettings>,
}

/// Value of one card in dust, per rarity. Used as the base value of the deposit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RaritySettings {
    pub common: u64,
    pub rare: u64,
    pub epic: u64,
    pub legendary: u64,
}

/// Terms of one auction house. Rates are in basis points (100 = 1%).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseSettings {
    pub name: String,
    /// Taken from the final price of a sale.
    pub commission: u32,
    /// Absolute part of the minimum bid increment, in dust.
    pub min_increment: u64,
    /// Relative part of the minimum bid increment.
    pub increment: u32,
    /// Deposit rate, applied to the base value of the card.
    pub deposit: u32,
    /// Allowed listing durations.
    pub durations: Vec<DurationSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurationSettings {
    pub minutes: u64,
    pub deposit_multiplier: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RarityName {
    Common,
    Rare,
    Epic,
    Legendary,
}

/// One card of the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardSettings {
    pub card_id: String,
    pub name: String,
    pub rarity: RarityName,
}

impl RaritySettings {
    fn values(&self) -> [(&'static str, u64); 4] {
        [
            ("common", self.common),
            ("rare", self.rare),
            ("epic", self.epic),
            ("legendary", self.legendary),
        ]
    }
}

impl HouseSettings {
    pub fn config(&self) -> HouseConfig {
        HouseConfig {
            name: self.name.clone(),
            commission: Bps(self.commission),
            min_increment: Money(self.min_increment),
            increment: Bps(self.increment),
            deposit: Bps(self.deposit),
            durations: self
                .durations
                .iter()
                .map(|d| DurationOption {
                    duration: Duration::from_secs(d.minutes * 60),
                    deposit_multiplier: d.deposit_multiplier,
                })
                .collect(),
        }
    }
}

impl CardSettings {
    /// The catalog entry, valued by the rarity table.
    pub fn entry(&self, values: &RaritySettings) -> CardEntry {
        let (rarity, base_value) = match self.rarity {
            RarityName::Common => (Rarity::Common, values.common),
            RarityName::Rare => (Rarity::Rare, values.rare),
            RarityName::Epic => (Rarity::Epic, values.epic),
            RarityName::Legendary => (Rarity::Legendary, values.legendary),
        };
        CardEntry {
            card: self.card_id.as_str().into(),
            name: self.name.clone(),
            rarity,
            base_value: Money(base_value),
        }
    }
}

fn default_house(name: &str, commission: u32) -> HouseSettings {
    let duration = |minutes, deposit_multiplier| DurationSettings {
        minutes,
        deposit_multiplier,
    };
    HouseSettings {
        name: name.into(),
        commission,
        min_increment: 1,
        increment: 500,
        deposit: 1_500,
        durations: vec![duration(2, 1), duration(10, 2), duration(30, 4)],
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8081".into(),
            settle_every_secs: 1,
            log: [
                ("tcg_server", "debug"),
                ("tcg_core", "debug"),
                ("tower_http", "info"),
            ]
            .into_iter()
            .map(|(target, level)| (target.into(), level.into()))
            .collect(),
            rarity: RaritySettings {
                common: 40,
                rare: 100,
                epic: 400,
                legendary: 1_600,
            },
            houses: vec![default_house("Market", 500)],
            catalog: Vec::new(),
        }
    }
}

/// The first value that appears twice, if any.
fn first_duplicate<'a, T: Ord + 'a>(values: impl Iterator<Item = &'a T>) -> Option<&'a T> {
    let mut seen = std::collections::BTreeSet::new();
    values.into_iter().find(|v| !seen.insert(*v))
}

impl Settings {
    /// Reads the settings file. `Ok(None)` if it does not exist.
    pub fn read(path: &Path) -> Result<Option<Settings>, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str::<Settings>(&text)
                .map_err(|e| e.to_string())
                .and_then(Settings::validated)
                .map(Some)
                .map_err(|e| format!("invalid {}: {e}", path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        }
    }

    fn validated(self) -> Result<Settings, String> {
        if self.settle_every_secs == 0 {
            return Err("settle_every_secs must be greater than zero".into());
        }
        if let Some((name, _)) = self.rarity.values().into_iter().find(|(_, v)| *v == 0) {
            return Err(format!("rarity {name}: value must be greater than zero"));
        }
        if self.houses.is_empty() {
            return Err("at least one [[house]] is required".into());
        }
        if let Some(name) = first_duplicate(self.houses.iter().map(|h| &h.name)) {
            return Err(format!("duplicate [[house]] name {name:?}"));
        }
        for house in &self.houses {
            let name = &house.name;
            if let Some(minutes) = first_duplicate(house.durations.iter().map(|d| &d.minutes)) {
                return Err(format!("house {name:?} lists {minutes} minutes twice"));
            }
            if house.durations.iter().any(|d| d.deposit_multiplier == 0) {
                return Err(format!(
                    "house {name:?}: deposit_multiplier must be greater than zero"
                ));
            }
        }
        if self.catalog.is_empty() {
            return Err("at least one `catalog` card is required".into());
        }
        if let Some(card_id) = first_duplicate(self.catalog.iter().map(|c| &c.card_id)) {
            return Err(format!("duplicate `catalog` card_id {card_id:?}"));
        }
        Ok(self)
    }

    /// The catalog entries, valued by the rarity table.
    pub fn catalog(&self) -> Vec<CardEntry> {
        self.catalog.iter().map(|c| c.entry(&self.rarity)).collect()
    }

    pub fn settle_every(&self) -> Duration {
        Duration::from_secs(self.settle_every_secs)
    }

    /// The `log` table as an `EnvFilter` directive, such as `info,tcg_core=debug`.
    pub fn log_filter(&self) -> String {
        self.log
            .iter()
            .map(|(target, level)| match target.as_str() {
                "default" => level.clone(),
                _ => format!("{target}={level}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_take_defaults() {
        let settings: Settings = toml::from_str("settle_every_secs = 5").unwrap();
        assert_eq!(settings.settle_every(), Duration::from_secs(5));
        assert_eq!(settings.bind_addr, Settings::default().bind_addr);
        assert_eq!(settings.rarity, Settings::default().rarity);
    }

    #[test]
    fn log_table_becomes_a_filter() {
        let settings: Settings = toml::from_str(
            r#"
            [log]
            default = "warn"
            tcg_core = "debug"
            "#,
        )
        .unwrap();
        assert_eq!(settings.log_filter(), "warn,tcg_core=debug");
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(toml::from_str::<Settings>("bind_adr = \"0.0.0.0:80\"").is_err());
        assert!(
            toml::from_str::<Settings>(
                "catalog = [{ card_id = \"a\", name = \"A\", rarity = \"mythic\" }]"
            )
            .is_err()
        );
    }

    #[test]
    fn houses_and_catalog_are_read() {
        let settings: Settings = toml::from_str(
            r#"
            rarity = { common = 1, rare = 2, epic = 3, legendary = 4 }
            catalog = [{ card_id = "drake", name = "Drake", rarity = "epic" }]

            [[house]]
            name = "Test"
            commission = 100
            min_increment = 1
            increment = 500
            deposit = 1000
            durations = [{ minutes = 5, deposit_multiplier = 1 }]
            "#,
        )
        .unwrap();
        let config = settings.houses[0].config();
        assert_eq!(config.commission, Bps(100));
        assert_eq!(config.durations[0].duration, Duration::from_secs(300));
        let entry = &settings.catalog()[0];
        assert_eq!(entry.rarity, Rarity::Epic);
        assert_eq!(entry.base_value, Money(3));
    }

    fn rejected(toml: &str) -> bool {
        toml::from_str::<Settings>(toml)
            .unwrap()
            .validated()
            .is_err()
    }

    const CATALOG: &str = r#"catalog = [{ card_id = "a", name = "A", rarity = "common" }]"#;

    const HOUSE: &str = r#"
        [[house]]
        name = "A"
        commission = 0
        min_increment = 1
        increment = 0
        deposit = 0
        durations = [{ minutes = 1, deposit_multiplier = 1 }]
    "#;

    #[test]
    fn duplicate_house_name_is_rejected() {
        assert!(rejected(&format!("{CATALOG}\n{HOUSE}{HOUSE}")));
    }

    #[test]
    fn bad_durations_are_rejected() {
        let zero = HOUSE.replace("deposit_multiplier = 1", "deposit_multiplier = 0");
        assert!(rejected(&format!("{CATALOG}\n{zero}")));
        let twice = HOUSE.replace(
            "[{ minutes = 1, deposit_multiplier = 1 }]",
            "[{ minutes = 1, deposit_multiplier = 1 }, { minutes = 1, deposit_multiplier = 2 }]",
        );
        assert!(rejected(&format!("{CATALOG}\n{twice}")));
    }

    #[test]
    fn bad_catalog_is_rejected() {
        let card = |id: &str| format!("{{ card_id = \"{id}\", name = \"x\", rarity = \"rare\" }}");
        assert!(rejected("catalog = []"));
        assert!(rejected(&format!(
            "catalog = [{}, {}]",
            card("a"),
            card("a")
        )));
    }

    #[test]
    fn zero_rarity_value_is_rejected() {
        assert!(rejected(&format!(
            "rarity = {{ common = 0, rare = 2, epic = 3, legendary = 4 }}\n{CATALOG}"
        )));
    }

    #[test]
    fn no_house_is_rejected() {
        assert!(rejected(&format!("house = []\n{CATALOG}")));
    }

    #[test]
    fn zero_settlement_delay_is_rejected() {
        assert!(rejected(&format!("settle_every_secs = 0\n{CATALOG}")));
    }

    #[test]
    fn shipped_file_shows_the_defaults() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tcg.toml");
        let shipped = Settings::read(&path).unwrap().unwrap();
        assert!(!shipped.catalog.is_empty());
        let expected = Settings {
            catalog: shipped.catalog.clone(),
            ..Settings::default()
        };
        assert_eq!(shipped, expected);
    }

    #[test]
    fn missing_file_is_not_an_error() {
        assert_eq!(Settings::read(Path::new("no/such/tcg.toml")), Ok(None));
    }
}
