//! PadPilot main app.
//!
//! Build step 1: plug in the virtual controller and hold a fixed throttle,
//! to prove the game accepts it. Usage: `padpilot [seconds]` (default 30).

mod controller;

use std::process::ExitCode;
use std::time::Duration;

use padpilot_core::gamepad::{GamepadState, racing_state};
use padpilot_core::stick::Deflection;
use tracing::{error, info};

use controller::VirtualController;

const DEFAULT_HOLD: Duration = Duration::from_secs(30);

fn main() -> ExitCode {
    let logs = padpilot_logging::logs_dir();
    let _log_guard = match padpilot_logging::init(&logs, "padpilot", tracing::Level::INFO) {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("cannot write logs to {}: {err}", logs.display());
            return ExitCode::FAILURE;
        }
    };
    info!(target: "app", version = env!("CARGO_PKG_VERSION"), "startup");

    let hold = hold_duration(std::env::args().nth(1));
    let result = hold_half_throttle(hold);
    if let Err(err) = &result {
        error!(target: "controller", "{err}");
        eprintln!("virtual controller failed: {err}");
    }

    info!(target: "app", "shutdown");
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn hold_duration(arg: Option<String>) -> Duration {
    arg.and_then(|seconds| seconds.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_HOLD)
}

fn hold_half_throttle(hold: Duration) -> Result<(), controller::Error> {
    let mut pad = controller::connect()?;
    let half_throttle = racing_state(Deflection::new(0.0, 0.5));
    pad.send(half_throttle)?;
    info!(target: "controller", ?half_throttle, seconds = hold.as_secs(), "holding");
    println!("Holding 50% throttle for {} s...", hold.as_secs());
    std::thread::sleep(hold);
    pad.send(GamepadState::default())?;
    info!(target: "controller", "released to neutral");
    Ok(())
}
