//! Finds and reads `%APPDATA%\PadPilot\config.toml`. PadPilot always starts:
//! a missing file is created with the defaults, and a file with a mistake is
//! reported and replaced by the defaults for this run.
//!
//! The file is read before logging starts, since it sets the log level;
//! [`Loaded::report`] logs the outcome once logging runs.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use padpilot_core::config::{Config, LogLevel, TEMPLATE};
use tracing::{Level, error, info};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};
use windows::core::HSTRING;

/// `%APPDATA%\PadPilot`: the user's settings and the saved window state.
pub fn settings_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("PadPilot")
}

pub struct Loaded {
    pub config: Config,
    path: PathBuf,
    outcome: Outcome,
}

enum Outcome {
    Read,
    Created,
    Problem(String),
}

pub fn read() -> Loaded {
    let path = settings_dir().join("config.toml");
    let (config, outcome) = match std::fs::read_to_string(&path) {
        Ok(text) => match Config::parse(&text) {
            Ok(config) => (config, Outcome::Read),
            Err(err) => (Config::default(), Outcome::Problem(err.to_string())),
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => match create(&path) {
            Ok(()) => (Config::default(), Outcome::Created),
            Err(err) => (
                Config::default(),
                Outcome::Problem(format!("cannot create the file: {err}")),
            ),
        },
        Err(err) => (
            Config::default(),
            Outcome::Problem(format!("cannot read the file: {err}")),
        ),
    };
    Loaded {
        config,
        path,
        outcome,
    }
}

impl Loaded {
    pub fn log_level(&self) -> Level {
        match self.config.log_level {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    }

    /// Logs what was read, and tells the user about a mistake in the file.
    pub fn report(&self) {
        let (path, config) = (self.path.display(), &self.config);
        match &self.outcome {
            Outcome::Read => info!(target: "config", %path, ?config, "config loaded"),
            Outcome::Created => {
                info!(target: "config", %path, ?config, "created config file with the defaults");
            }
            Outcome::Problem(problem) => {
                error!(target: "config", %path, %problem, ?config, "config not usable, using defaults");
                warn_user(&format!(
                    "{path}\n\n{problem}\n\nPadPilot will use the default settings this time."
                ));
            }
        }
    }
}

/// Never overwrites: if the file appeared in the meantime, it is left alone.
fn create(path: &Path) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::File::create_new(path) {
        Ok(mut file) => file.write_all(TEMPLATE.replace('\n', "\r\n").as_bytes()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(err),
    }
}

fn warn_user(text: &str) {
    // SAFETY: plain message box with no owner window; both strings outlive the call.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from("PadPilot settings"),
            MB_OK | MB_ICONWARNING,
        );
    }
}
