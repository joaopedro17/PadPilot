//! PadPilot settings window. Every change is written to `config.toml` at
//! once, and the joystick picks it up while it runs.
//!
//! The joystick starts it with `--hidden` when it starts, and shows it when
//! its Settings button is clicked, so it appears at once. Started without
//! arguments, it simply opens.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod window;

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(windows)]
    return window::run(std::env::args().any(|arg| arg == "--hidden"));

    #[cfg(not(windows))]
    {
        eprintln!("PadPilot only runs on Windows for now.");
        ExitCode::FAILURE
    }
}
