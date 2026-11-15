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
///
/// The round stick is stretched onto a square, so the ring's edge is full
/// input: throttle (or brake) stays full while steering until the diagonal,
/// and steering is at full lock from the diagonal on. In every direction,
/// the stronger of the two inputs is how far the stick is pushed.
pub fn racing_state(stick: Deflection) -> GamepadState {
    let (x, y) = square(stick);
    GamepadState {
        left_stick_x: (x * f32::from(i16::MAX)).round() as i16,
        right_trigger: trigger(y),
        left_trigger: trigger(-y),
    }
}

fn square(stick: Deflection) -> (f32, f32) {
    let (x, y) = (stick.x(), stick.y());
    let stronger = x.abs().max(y.abs());
    if stronger == 0.0 {
        return (0.0, 0.0);
    }
    let stretch = x.hypot(y) / stronger;
    (
        (x * stretch).clamp(-1.0, 1.0),
        (y * stretch).clamp(-1.0, 1.0),
    )
}

fn trigger(amount: f32) -> u8 {
    (amount.max(0.0) * f32::from(u8::MAX)).round() as u8
}
