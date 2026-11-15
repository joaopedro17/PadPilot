//! Platform-independent PadPilot logic. Nothing here may depend on an
//! operating system API, so everything is testable without hardware.

pub mod config;
pub mod gamepad;
pub mod overlay;
pub mod stick;
pub mod touchpad;
pub mod watchdog;
