//! Platform-independent PadPilot logic. Nothing here may depend on an
//! operating system API, so everything is testable without hardware.

pub mod gamepad;
pub mod stick;
pub mod touchpad;
