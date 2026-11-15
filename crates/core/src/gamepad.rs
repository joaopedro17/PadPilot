//! Maps the stick onto a virtual Xbox controller for racing games.

use crate::stick::Deflection;

/// The controller inputs PadPilot drives. `Default` is everything released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GamepadState {
    pub left_stick_x: i16,
    pub right_trigger: u8,
    pub left_trigger: u8,
}

/// Left/right steers, up is throttle, down is brake.
pub fn racing_state(stick: Deflection) -> GamepadState {
    GamepadState {
        left_stick_x: (stick.x() * f32::from(i16::MAX)).round() as i16,
        right_trigger: trigger(stick.y()),
        left_trigger: trigger(-stick.y()),
    }
}

fn trigger(amount: f32) -> u8 {
    (amount.max(0.0) * f32::from(u8::MAX)).round() as u8
}
