//! Server settings, read from a TOML file. Every top-level key is optional except `catalog`.
//! Inside a house or a catalog entry, every key is required except `commodity`.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use agora_domain::{Bps, DurationOption, HouseConfig, Money};
use game_domain::CatalogEntry;
use serde::Deserialize;

/// Settings file read when `GAME_CONFIG` is not set, relative to the working directory.
pub const DEFAULT_PATH: &str = "game.toml";

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
    /// Auction houses opened at startup, one `[[house]]` table each.
    #[serde(rename = "house")]
    pub houses: Vec<HouseSettings>,
    /// Item types the game knows, one inline table each in the `catalog` array. No default: the file must list them.
    pub catalog: Vec<CatalogSettings>,
}

/// Terms of one auction house. Rates are in basis points (100 = 1%).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseSettings {
    pub name: String,
    /// Taken from the final price of a sale.
    pub commission: u32,
    /// Absolute part of the minimum bid increment, per unit, in copper.
    pub min_increment: u64,
    /// Relative part of the minimum bid increment.
    pub increment: u32,
    /// Deposit rate, applied to the total base value of the stack.
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

/// One item type of the catalog. `base_value` is in copper.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogSettings {
    pub item_type: String,
    pub name: String,
    pub base_value: u64,
    #[serde(default)]
    pub commodity: bool,
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

impl CatalogSettings {
    pub fn entry(&self) -> CatalogEntry {
        CatalogEntry {
            item_type: self.item_type.as_str().into(),
            name: self.name.clone(),
            base_value: Money(self.base_value),
            commodity: self.commodity,
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
            bind_addr: "127.0.0.1:8080".into(),
            settle_every_secs: 1,
            log: [
                ("game_server", "debug"),
                ("game_core", "debug"),
                ("tower_http", "info"),
            ]
            .into_iter()
            .map(|(target, level)| (target.into(), level.into()))
            .collect(),
            houses: vec![
                default_house("Alliance", 500),
                default_house("Neutre", 1_500),
            ],
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
            return Err("at least one `catalog` item is required".into());
        }
        if let Some(item_type) = first_duplicate(self.catalog.iter().map(|c| &c.item_type)) {
            return Err(format!("duplicate `catalog` item_type {item_type:?}"));
        }
        if let Some(item) = self.catalog.iter().find(|c| c.base_value == 0) {
            return Err(format!(
                "catalog item {:?}: base_value must be greater than zero",
                item.item_type
            ));
        }
        Ok(self)
    }

    pub fn settle_every(&self) -> Duration {
        Duration::from_secs(self.settle_every_secs)
    }

    /// The `log` table as an `EnvFilter` directive, such as `info,game_core=debug`.
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
    }

    #[test]
    fn log_table_becomes_a_filter() {
        let settings: Settings = toml::from_str(
            r#"
            [log]
            default = "warn"
            game_core = "debug"
            "#,
        )
        .unwrap();
        assert_eq!(settings.log_filter(), "warn,game_core=debug");
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(toml::from_str::<Settings>("bind_adr = \"0.0.0.0:80\"").is_err());
    }

    #[test]
    fn houses_and_catalog_are_read() {
        let settings: Settings = toml::from_str(
            r#"
            catalog = [{ item_type = "stick", name = "Stick", base_value = 2 }]

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
        assert_eq!(settings.catalog[0].entry().base_value, Money(2));
    }

    fn rejected(toml: &str) -> bool {
        toml::from_str::<Settings>(toml)
            .unwrap()
            .validated()
            .is_err()
    }

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
        assert!(rejected(&format!("{HOUSE}{HOUSE}")));
    }

    #[test]
    fn bad_durations_are_rejected() {
        let zero = HOUSE.replace("deposit_multiplier = 1", "deposit_multiplier = 0");
        assert!(rejected(&zero));
        let twice = HOUSE.replace(
            "[{ minutes = 1, deposit_multiplier = 1 }]",
            "[{ minutes = 1, deposit_multiplier = 1 }, { minutes = 1, deposit_multiplier = 2 }]",
        );
        assert!(rejected(&twice));
    }

    #[test]
    fn bad_catalog_is_rejected() {
        let item = |name: &str, value: u64| {
            format!("{{ item_type = \"{name}\", name = \"x\", base_value = {value} }}")
        };
        assert!(rejected("catalog = []"));
        assert!(rejected(&format!("catalog = [{}, {}]", item("a", 1), item("a", 2))));
        assert!(rejected(&format!("catalog = [{}]", item("a", 0))));
    }

    #[test]
    fn no_house_is_rejected() {
        let settings: Settings = toml::from_str("house = []").unwrap();
        assert!(settings.validated().is_err());
    }

    #[test]
    fn zero_settlement_delay_is_rejected() {
        let settings: Settings = toml::from_str("settle_every_secs = 0").unwrap();
        assert!(settings.validated().is_err());
    }

    #[test]
    fn shipped_file_shows_the_defaults() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../game.toml");
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
        assert_eq!(Settings::read(Path::new("no/such/game.toml")), Ok(None));
    }
}
