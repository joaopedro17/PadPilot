//! PadPilot main app.

#[cfg(windows)]
mod commands;
#[cfg(windows)]
mod config;
#[cfg(windows)]
mod controller;
#[cfg(windows)]
mod error;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod touchpad;

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(windows)]
    return commands::run(std::env::args().skip(1));

    #[cfg(not(windows))]
    {
        eprintln!("PadPilot only runs on Windows for now.");
        ExitCode::FAILURE
    }
}
