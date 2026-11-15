//! PadPilot settings window. Opened by the joystick's Settings button; every
//! change is written to `config.toml` at once, and the joystick picks it up
//! while it runs.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod window;

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(windows)]
    return window::run();

    #[cfg(not(windows))]
    {
        eprintln!("PadPilot only runs on Windows for now.");
        ExitCode::FAILURE
    }
}
