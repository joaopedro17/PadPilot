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

/// A point on the ring, `degrees` clockwise from straight up.
fn on_ring(degrees: f32) -> GamepadState {
    let (sin, cos) = degrees.to_radians().sin_cos();
    racing_state(Deflection::new(sin, cos))
}

#[test]
fn on_the_ring_throttle_stays_full_while_steering_until_the_diagonal() {
    for degrees in [-44.0, -30.0, -10.0, 10.0, 30.0, 44.0] {
        assert_eq!(on_ring(degrees).right_trigger, 255, "{degrees} degrees");
    }
    let slightly_left = on_ring(-15.0);
    let expected = -(15f32.to_radians().tan() * 32767.0).round() as i16;
    assert_eq!(slightly_left.left_stick_x, expected);
}

#[test]
fn on_the_ring_steering_is_at_full_lock_from_the_diagonal_on() {
    for degrees in [45.0, 60.0, 90.0, 120.0, 135.0] {
        assert_eq!(on_ring(degrees).left_stick_x, 32767, "{degrees} degrees");
        assert_eq!(on_ring(-degrees).left_stick_x, -32767, "{degrees} degrees");
    }
    // Past the diagonal, the throttle eases off as the stick turns sideways.
    let state = on_ring(60.0);
    assert_eq!(
        state.right_trigger,
        (30f32.to_radians().tan() * 255.0).round() as u8
    );
}

#[test]
fn on_the_ring_brake_stays_full_while_steering_until_the_diagonal() {
    for degrees in [140.0, 170.0, 190.0, 220.0] {
        let state = on_ring(degrees);
        assert_eq!(
            (state.left_trigger, state.right_trigger),
            (255, 0),
            "{degrees} degrees"
        );
    }
}

#[test]
fn inside_the_ring_the_stronger_input_follows_how_far_the_stick_is_pushed() {
    let d = std::f32::consts::FRAC_1_SQRT_2 * 0.6;
    let diagonal = racing_state(Deflection::new(d, d));
    assert_eq!(
        (diagonal.left_stick_x, diagonal.right_trigger),
        (19660, 153)
    );
    let mostly_up = racing_state(Deflection::new(-0.36, 0.48));
    assert_eq!(
        (mostly_up.left_stick_x, mostly_up.right_trigger),
        (-14745, 153)
    );
}
