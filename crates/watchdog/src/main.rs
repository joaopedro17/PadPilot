//! PadPilot watchdog. The main app starts it with its own process id and
//! sends it heartbeats through its standard input. If they stop, the
//! watchdog frees the cursor and ends the main app, which unplugs the
//! virtual controller. It has no window and installs nothing.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod watch;

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(windows)]
    return watch::run(std::env::args().nth(1));

    #[cfg(not(windows))]
    {
        eprintln!("PadPilot only runs on Windows for now.");
        ExitCode::FAILURE
    }
}
