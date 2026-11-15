//! The settings file, `config.toml`. Missing settings take their defaults;
//! a misspelled or impossible one is an error, so it is never silently ignored.

use serde::Deserialize;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::stick::{
    DEFAULT_DEAD_ZONE, DEFAULT_FULL_TRAVEL, LiftBehavior, SettingsError, StickSettings,
};

/// Sent by the joystick to the settings window, which waits hidden in the
/// background so it appears at once.
pub const SHOW_SETTINGS: u8 = b's';

/// Written for new users. Must hold exactly the defaults.
pub const TEMPLATE: &str = "\
# PadPilot settings. Changes take effect the next time PadPilot starts.

[stick]
# Sensitivity: how far your finger moves on the touchpad, in millimetres,
# to push the stick from center to 100%. Smaller is sharper.
full_travel_mm = 10
# Dead zone: the part around the center that does nothing, as a fraction
# of the way to the edge, from 0 up to (not including) 1. 0.05 is 5%.
dead_zone = 0.05
# What happens when you lift your finger:
#   \"return_to_center\"  the stick springs back to the middle
#   \"hold_position\"     the stick stays where it is, like cruise control
lift_behavior = \"return_to_center\"

[log]
# How much goes into the log: \"error\", \"warn\", \"info\", \"debug\" or \"trace\".
# \"trace\" records every touchpad event and makes big files.
level = \"info\"
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    pub stick: StickSettings,
    pub log_level: LogLevel,
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
    /// `text` with these settings written into it, keeping its comments and
    /// anything else in it. A file that cannot hold them is replaced by the
    /// explained template.
    pub fn write_into(&self, text: &str) -> String {
        let written = text
            .parse::<DocumentMut>()
            .map(|document| self.written(document))
            .ok()
            .filter(|written| Config::parse(written).is_ok_and(|read| read == *self));
        written.unwrap_or_else(|| {
            let template = TEMPLATE.parse().unwrap_or_default();
            self.written(template)
        })
    }

    fn written(&self, mut document: DocumentMut) -> String {
        // Rounded, so the file shows 0.05 rather than 0.0500000007.
        let rounded = |number: f32, places: i32| {
            let factor = 10f64.powi(places);
            (f64::from(number) * factor).round() / factor
        };
        for section in ["stick", "log"] {
            if !document.contains_table(section) {
                document.insert(section, Item::Table(Table::new()));
            }
        }
        let stick = &self.stick;
        document["stick"]["full_travel_mm"] = value(rounded(stick.full_travel(), 1));
        document["stick"]["dead_zone"] = value(rounded(stick.dead_zone(), 2));
        document["stick"]["lift_behavior"] = value(match stick.lift_behavior() {
            LiftBehavior::ReturnToCenter => "return_to_center",
            LiftBehavior::HoldPosition => "hold_position",
        });
        document["log"]["level"] = value(match self.log_level {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        });
        document.to_string()
    }

    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let file: File = toml::from_str(text).map_err(|err| ConfigError(err.to_string()))?;
        let lift_behavior = match file.stick.lift_behavior {
            Lift::ReturnToCenter => LiftBehavior::ReturnToCenter,
            Lift::HoldPosition => LiftBehavior::HoldPosition,
        };
        let stick = StickSettings::new(
            lift_behavior,
            file.stick.full_travel_mm,
            file.stick.dead_zone,
        )
        .map_err(|err| {
            ConfigError(
                match err {
                    SettingsError::FullTravel => "[stick] full_travel_mm must be a number above 0",
                    SettingsError::DeadZone => {
                        "[stick] dead_zone must be from 0 up to (not including) 1"
                    }
                }
                .to_owned(),
            )
        })?;
        Ok(Self {
            stick,
            log_level: file.log.level,
        })
    }
}

/// The file as written, before its values are checked.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct File {
    stick: StickSection,
    log: LogSection,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StickSection {
    full_travel_mm: f32,
    dead_zone: f32,
    lift_behavior: Lift,
}

impl Default for StickSection {
    fn default() -> Self {
        Self {
            full_travel_mm: DEFAULT_FULL_TRAVEL,
            dead_zone: DEFAULT_DEAD_ZONE,
            lift_behavior: Lift::ReturnToCenter,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Lift {
    ReturnToCenter,
    HoldPosition,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct LogSection {
    level: LogLevel,
}
