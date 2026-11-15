//! The settings file, `config.toml`. Missing settings take their defaults;
//! a misspelled or impossible one is an error, so it is never silently ignored.

use serde::Deserialize;

use crate::stick::{DEFAULT_DEAD_ZONE, DEFAULT_FULL_TRAVEL, LiftBehavior, StickSettings};

/// Written for new users. Must hold exactly the defaults.
pub const TEMPLATE: &str = "\
# PadPilot settings. Changes take effect the next time PadPilot starts.

[stick]
# Sensitivity: how far your finger moves on the touchpad, in millimetres,
# to push the stick from center to 100%. Smaller is sharper.
full_travel_mm = 10
";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    pub stick: StickSettings,
}

#[derive(Debug)]
pub struct ConfigError(String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let file: File = toml::from_str(text).map_err(|err| ConfigError(err.to_string()))?;
        let stick = StickSettings::new(
            LiftBehavior::default(),
            file.stick.full_travel_mm,
            DEFAULT_DEAD_ZONE,
        )
        .map_err(|_| ConfigError("[stick] full_travel_mm must be a number above 0".to_owned()))?;
        Ok(Self { stick })
    }
}

/// The file as written, before its values are checked.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct File {
    stick: StickSection,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StickSection {
    full_travel_mm: f32,
}

impl Default for StickSection {
    fn default() -> Self {
        Self {
            full_travel_mm: DEFAULT_FULL_TRAVEL,
        }
    }
}
