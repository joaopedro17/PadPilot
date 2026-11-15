//! Command line of the main app.
//!
//! Until the overlay exists, PadPilot runs as small diagnostic commands, one
//! per build step. They stay useful afterwards for checking a new machine.

use std::ops::ControlFlow;
use std::process::ExitCode;
use std::time::Duration;

use padpilot_core::gamepad::{GamepadState, racing_state};
use padpilot_core::stick::{Contact, Deflection, Stick, StickSettings};
use tracing::{Level, debug, error, info, trace};

use crate::controller::{self, VirtualController};
use crate::error::Error;
use crate::touchpad;

const USAGE: &str = "\
Usage:
  padpilot throttle [seconds]   plug in the virtual controller and hold 50% throttle
  padpilot touchpad [seconds]   log finger touch-down, movement and lift
  padpilot drive [seconds]      drive the virtual controller from the touchpad

Seconds default to 30.";

const DEFAULT_SECONDS: u64 = 30;

#[derive(Debug, Clone, Copy)]
enum Command {
    Throttle(Duration),
    Touchpad(Duration),
    Drive(Duration),
}

impl Command {
    fn parse(mut args: impl Iterator<Item = String>) -> Option<Self> {
        let name = args.next()?;
        let seconds = match args.next() {
            Some(seconds) => seconds.parse().ok()?,
            None => DEFAULT_SECONDS,
        };
        let duration = Duration::from_secs(seconds);
        match name.as_str() {
            "throttle" => Some(Self::Throttle(duration)),
            "touchpad" => Some(Self::Touchpad(duration)),
            "drive" => Some(Self::Drive(duration)),
            _ => None,
        }
    }

    /// Per-event touchpad data is only logged at trace.
    fn log_level(self) -> Level {
        match self {
            Self::Throttle(_) | Self::Drive(_) => Level::INFO,
            Self::Touchpad(_) => Level::TRACE,
        }
    }

    fn run(self) -> Result<(), Error> {
        match self {
            Self::Throttle(hold) => hold_half_throttle(hold),
            Self::Touchpad(duration) => log_touches(duration),
            Self::Drive(duration) => drive(duration),
        }
    }
}

pub fn run(args: impl Iterator<Item = String>) -> ExitCode {
    let Some(command) = Command::parse(args) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let logs = padpilot_logging::logs_dir();
    let _log_guard = match padpilot_logging::init(&logs, "padpilot", command.log_level()) {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("cannot write logs to {}: {err}", logs.display());
            return ExitCode::FAILURE;
        }
    };
    info!(target: "app", version = env!("CARGO_PKG_VERSION"), ?command, "startup");
    println!("Logging to {}", logs.display());

    let result = command.run();
    if let Err(err) = &result {
        error!(target: "app", "{err}");
        eprintln!("Failed: {err}");
    }

    info!(target: "app", "shutdown");
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn hold_half_throttle(hold: Duration) -> Result<(), Error> {
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

fn log_touches(duration: Duration) -> Result<(), Error> {
    println!(
        "Touch the touchpad. Reading for {} s...",
        duration.as_secs()
    );
    let mut touching = false;
    let mut moves = 0u32;
    touchpad::listen(Some(duration), |contact| {
        match contact {
            Contact::Touching(at) if !touching => {
                touching = true;
                moves = 0;
                debug!(target: "reader", x_mm = at.x, y_mm = at.y, "touch down");
                println!("touch down at {:6.1} mm, {:6.1} mm", at.x, at.y);
            }
            Contact::Touching(at) => {
                moves += 1;
                trace!(target: "reader", x_mm = at.x, y_mm = at.y, "move");
            }
            Contact::Lifted => {
                touching = false;
                debug!(target: "reader", moves, "lift");
                println!("lift after {moves} moves");
            }
        }
        ControlFlow::Continue(())
    })
}

/// Finger movement deflects the stick; the stick drives the controller.
fn drive(duration: Duration) -> Result<(), Error> {
    let mut pad = controller::connect()?;
    let settings = StickSettings::default();
    let mut stick = Stick::new(settings);
    info!(target: "stick", ?settings, seconds = duration.as_secs(), "driving from the touchpad");
    println!("Driving for {} s.", duration.as_secs());
    println!("Slide up for throttle, down for brake, sideways to steer. Lift to release.");

    let mut failure = None;
    touchpad::listen(Some(duration), |contact| {
        stick.apply(contact);
        let state = racing_state(stick.output());
        trace!(target: "stick", ?contact, ?state, "update");
        match pad.send(state) {
            Ok(()) => ControlFlow::Continue(()),
            Err(err) => {
                failure = Some(err);
                ControlFlow::Break(())
            }
        }
    })?;
    if let Some(err) = failure {
        return Err(err);
    }
    pad.send(GamepadState::default())?;
    info!(target: "controller", "released to neutral");
    Ok(())
}
