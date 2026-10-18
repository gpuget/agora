//! Server settings, read from a TOML file. Every key is optional.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

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
        }
    }
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
    fn zero_settlement_delay_is_rejected() {
        let settings: Settings = toml::from_str("settle_every_secs = 0").unwrap();
        assert!(settings.validated().is_err());
    }

    #[test]
    fn shipped_file_shows_the_defaults() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../game.toml");
        assert_eq!(Settings::read(&path), Ok(Some(Settings::default())));
    }

    #[test]
    fn missing_file_is_not_an_error() {
        assert_eq!(Settings::read(Path::new("no/such/game.toml")), Ok(None));
    }
}
