//! The virtual Xbox controller the game sees.
//!
//! Everything else talks to [`VirtualController`], so the driver underneath
//! (ViGEmBus today) can be replaced without touching the rest of the app.

use padpilot_core::gamepad::GamepadState;

#[cfg(windows)]
mod vigem;

/// A plugged-in virtual controller. Dropping it unplugs the controller.
pub trait VirtualController {
    fn send(&mut self, state: GamepadState) -> Result<(), Error>;
}

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Plugs in a virtual controller and waits until Windows has it ready.
#[cfg(windows)]
pub fn connect() -> Result<impl VirtualController, Error> {
    vigem::Vigem::connect()
}

#[cfg(not(windows))]
pub fn connect() -> Result<impl VirtualController, Error> {
    struct Unsupported;
    impl VirtualController for Unsupported {
        fn send(&mut self, _: GamepadState) -> Result<(), Error> {
            unreachable!("never constructed")
        }
    }
    Err::<Unsupported, _>(Error("PadPilot only runs on Windows for now".to_owned()))
}
