//! Finds and reads `%APPDATA%\PadPilot\config.toml`. PadPilot always starts:
//! a missing file is created with the defaults, and a file with a mistake is
//! reported and replaced by the defaults for this run.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use padpilot_core::config::{Config, TEMPLATE};
use tracing::{error, info};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};
use windows::core::HSTRING;

pub fn load() -> Config {
    let path = path();
    match read(&path) {
        Ok(config) => {
            info!(target: "config", path = %path.display(), ?config, "config loaded");
            config
        }
        Err(problem) => {
            let config = Config::default();
            error!(target: "config", path = %path.display(), %problem, ?config, "config not usable, using defaults");
            warn_user(&format!(
                "{}\n\n{problem}\n\nPadPilot will use the default settings this time.",
                path.display()
            ));
            config
        }
    }
}

fn path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("PadPilot")
        .join("config.toml")
}

fn read(path: &Path) -> Result<Config, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Config::parse(&text).map_err(|err| err.to_string()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            create(path).map_err(|err| format!("cannot create the file: {err}"))?;
            info!(target: "config", path = %path.display(), "created config file with the defaults");
            Ok(Config::default())
        }
        Err(err) => Err(format!("cannot read the file: {err}")),
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
