//! Command line of the main app.
//!
//! Without arguments PadPilot shows the on-screen joystick. The other
//! commands test one piece at a time, for checking a new machine.

use std::ops::ControlFlow;
use std::process::ExitCode;
use std::time::Duration;

use padpilot_core::gamepad::{GamepadState, racing_state};
use padpilot_core::stick::{Contact, Deflection, Stick, StickSettings};
use tracing::{Level, debug, error, info, trace};

use crate::controller::{self, VirtualController};
use crate::error::Error;
use crate::{config, overlay, touchpad};

const USAGE: &str = "\
Usage:
  padpilot [overlay [seconds]]  show the on-screen joystick; joystick mode
                                switches itself off after the seconds (default:
                                never, or 60 when the watchdog is not running)
  padpilot telemetry [seconds]  the same, recording input timing to a CSV file
  padpilot freeze [seconds]     the same, but the window freezes after joystick
                                mode has been on for the seconds (default 10),
                                to check that the watchdog steps in
  padpilot throttle [seconds]   plug in the virtual controller and hold 50% throttle
  padpilot touchpad [seconds]   log finger touch-down, movement and lift
  padpilot drive [seconds]      drive the virtual controller from the touchpad

Seconds default to 30 for the test commands.";

const DEFAULT_SECONDS: u64 = 30;
const FREEZE_AFTER_SECONDS: u64 = 10;

#[derive(Debug, Clone, Copy)]
enum Command {
    Throttle(Duration),
    Touchpad(Duration),
    Drive(Duration),
    Overlay(overlay::Options),
}

impl Command {
    fn parse(mut args: impl Iterator<Item = String>) -> Option<Self> {
        let name = args.next().unwrap_or_else(|| "overlay".to_owned());
        let seconds = match args.next() {
            Some(seconds) => Some(seconds.parse().ok()?),
            None => None,
        };
        let duration = Duration::from_secs(seconds.unwrap_or(DEFAULT_SECONDS));
        match name.as_str() {
            "overlay" | "telemetry" => Some(Self::Overlay(overlay::Options {
                joystick_limit: seconds.map(Duration::from_secs),
                telemetry: name == "telemetry",
                freeze_after: None,
            })),
            "freeze" => Some(Self::Overlay(overlay::Options {
                freeze_after: Some(Duration::from_secs(seconds.unwrap_or(FREEZE_AFTER_SECONDS))),
                ..overlay::Options::default()
            })),
            "throttle" => Some(Self::Throttle(duration)),
            "touchpad" => Some(Self::Touchpad(duration)),
            "drive" => Some(Self::Drive(duration)),
            _ => None,
        }
    }

    /// The touchpad test logs every event, at trace; the rest follow the
    /// config file.
    fn log_level(self, configured: Level) -> Level {
        match self {
            Self::Throttle(_) | Self::Drive(_) | Self::Overlay(_) => configured,
            Self::Touchpad(_) => Level::TRACE,
        }
    }

    fn run(self, settings: StickSettings) -> Result<(), Error> {
        match self {
            Self::Throttle(hold) => hold_half_throttle(hold),
            Self::Touchpad(duration) => log_touches(duration),
            Self::Drive(duration) => drive(duration, settings),
            Self::Overlay(options) => overlay::run(options, settings),
        }
    }
}

pub fn run(args: impl Iterator<Item = String>) -> ExitCode {
    let Some(command) = Command::parse(args) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let loaded = config::read();
    let logs = padpilot_logging::logs_dir();
    let level = command.log_level(loaded.log_level());
    let _log_guard = match padpilot_logging::init(&logs, "padpilot", level) {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("cannot write logs to {}: {err}", logs.display());
            return ExitCode::FAILURE;
        }
    };
    info!(target: "app", version = env!("CARGO_PKG_VERSION"), ?command, "startup");
    println!("Logging to {}", logs.display());

    loaded.report();
    let result = command.run(loaded.config.stick);
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
fn drive(duration: Duration, settings: StickSettings) -> Result<(), Error> {
    let mut pad = controller::connect()?;
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
