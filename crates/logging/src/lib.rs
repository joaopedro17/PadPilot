//! Where PadPilot keeps its files, and the log setup shared by its programs.
//!
//! Lines look like `2026-11-15 12:00:00.123  INFO controller: connected`.
//! The component name is the event's `target`.

use std::io;
use std::path::{Path, PathBuf};

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::time::ChronoLocal;

/// Daily log files older than this many days are deleted.
const KEPT_LOG_FILES: usize = 14;

/// Keeps the background writer alive. Dropping it flushes the log.
pub type LogGuard = WorkerGuard;

/// `%LOCALAPPDATA%\PadPilot\logs`, or the temp folder when that is unavailable.
pub fn logs_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("PadPilot")
        .join("logs")
}

/// `%APPDATA%\PadPilot`: the user's settings and the saved window state.
pub fn settings_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("PadPilot")
}

/// The user's settings file.
pub fn config_path() -> PathBuf {
    settings_dir().join("config.toml")
}

/// Starts logging to `<dir>/<component>.<date>.log` through a background
/// writer, and makes panics leave a trace in it.
pub fn init(dir: &Path, component: &str, level: Level) -> io::Result<LogGuard> {
    std::fs::create_dir_all(dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(component)
        .filename_suffix("log")
        .max_log_files(KEPT_LOG_FILES)
        .build(dir)
        .map_err(io::Error::other)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_timer(ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f".to_owned()))
        .with_max_level(level)
        .try_init()
        .map_err(io::Error::other)?;
    log_panics(component.to_owned());
    Ok(guard)
}

fn log_panics(component: String) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(target: "panic", component, "{info}");
        default_hook(info);
    }));
}
