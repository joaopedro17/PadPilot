//! Reads finger contact from the Precision Touchpad.
//!
//! Ordinary mouse events cannot tell a resting finger from a lifted one, so
//! this reads the touchpad's own HID reports through Raw Input. They keep
//! arriving while another window, the game, has focus.

mod hid;
mod raw_input;

pub use raw_input::{Reader, listen};
