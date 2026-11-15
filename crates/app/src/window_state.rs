//! Loads and saves `%APPDATA%\PadPilot\window.toml`. Only PadPilot writes it.

use std::path::PathBuf;

use padpilot_core::window_state::WindowState;
use tracing::{info, warn};

use padpilot_logging::settings_dir;

fn path() -> PathBuf {
    settings_dir().join("window.toml")
}

/// The saved state, or the defaults when there is none.
pub fn load() -> WindowState {
    let state = std::fs::read_to_string(path())
        .map(|text| WindowState::parse(&text))
        .unwrap_or_default();
    info!(target: "overlay", ?state, "window state loaded");
    state
}

pub fn save(state: &WindowState) {
    let path = path();
    match padpilot_logging::write_text_file(&path, &state.to_toml()) {
        Ok(()) => info!(target: "overlay", ?state, "window state saved"),
        Err(err) => {
            warn!(target: "overlay", path = %path.display(), "cannot save the window state: {err}")
        }
    }
}
