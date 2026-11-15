//! The virtual Xbox controller the game sees.
//!
//! Everything else talks to [`VirtualController`], so the driver underneath
//! (ViGEmBus today) can be replaced without touching the rest of the app.

use padpilot_core::gamepad::GamepadState;

use crate::error::Error;

mod vigem;

/// A plugged-in virtual controller. Dropping it unplugs the controller.
pub trait VirtualController {
    fn send(&mut self, state: GamepadState) -> Result<(), Error>;
}

/// Plugs in a virtual controller and waits until Windows has it ready.
pub fn connect() -> Result<impl VirtualController, Error> {
    vigem::Vigem::connect()
}
