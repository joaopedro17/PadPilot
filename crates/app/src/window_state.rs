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

/// Written to a temporary file first, so a crash mid-write never leaves a
/// half-written file behind.
pub fn save(state: &WindowState) {
    let path = path();
    let temporary = path.with_extension("toml.new");
    let written = std::fs::create_dir_all(settings_dir())
        .and_then(|()| std::fs::write(&temporary, state.to_toml().replace('\n', "\r\n")))
        .and_then(|()| std::fs::rename(&temporary, &path));
    match written {
        Ok(()) => info!(target: "overlay", ?state, "window state saved"),
        Err(err) => {
            warn!(target: "overlay", path = %path.display(), "cannot save the window state: {err}")
        }
    }
}
