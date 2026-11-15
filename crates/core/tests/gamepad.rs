use padpilot_core::gamepad::{GamepadState, racing_state};
use padpilot_core::stick::Deflection;

#[test]
fn neutral_stick_releases_everything() {
    assert_eq!(racing_state(Deflection::NEUTRAL), GamepadState::default());
}

#[test]
fn left_and_right_steer() {
    assert_eq!(racing_state(Deflection::new(1.0, 0.0)).left_stick_x, 32767);
    assert_eq!(
        racing_state(Deflection::new(-1.0, 0.0)).left_stick_x,
        -32767
    );
    assert_eq!(racing_state(Deflection::new(0.5, 0.0)).left_stick_x, 16384);
}

#[test]
fn up_is_throttle_only() {
    let full = racing_state(Deflection::new(0.0, 1.0));
    assert_eq!((full.right_trigger, full.left_trigger), (255, 0));
    assert_eq!(racing_state(Deflection::new(0.0, 0.5)).right_trigger, 128);
}

#[test]
fn down_is_brake_only() {
    let full = racing_state(Deflection::new(0.0, -1.0));
    assert_eq!((full.right_trigger, full.left_trigger), (0, 255));
    assert_eq!(racing_state(Deflection::new(0.0, -0.5)).left_trigger, 128);
}

#[test]
fn steering_and_throttle_combine() {
    let state = racing_state(Deflection::new(-0.6, 0.8));
    assert_eq!(
        state,
        GamepadState {
            left_stick_x: -19660,
            right_trigger: 204,
            left_trigger: 0
        }
    );
}
