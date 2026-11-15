use padpilot_core::stick::{
    Contact, Deflection, LiftBehavior, Point, SettingsError, Stick, StickSettings,
};

const TRAVEL: f32 = 20.0;

fn stick(lift: LiftBehavior, dead_zone: f32) -> Stick {
    Stick::new(StickSettings::new(lift, TRAVEL, dead_zone).unwrap())
}

fn spring() -> Stick {
    stick(LiftBehavior::ReturnToCenter, 0.0)
}

fn touch(stick: &mut Stick, x: f32, y: f32) {
    stick.apply(Contact::Touching(Point { x, y }));
}

#[track_caller]
fn assert_at(stick: &Stick, x: f32, y: f32) {
    let out = stick.output();
    assert!(
        (out.x() - x).abs() < 1e-4 && (out.y() - y).abs() < 1e-4,
        "expected ({x}, {y}), got ({}, {})",
        out.x(),
        out.y()
    );
}

#[test]
fn starts_neutral() {
    assert_eq!(spring().output(), Deflection::NEUTRAL);
}

#[test]
fn touching_down_does_not_move_the_stick_wherever_the_finger_lands() {
    let mut s = spring();
    touch(&mut s, 73.0, 41.0);
    assert_at(&s, 0.0, 0.0);
}

#[test]
fn finger_travel_deflects_in_proportion() {
    let mut s = spring();
    touch(&mut s, 50.0, 50.0);
    touch(&mut s, 60.0, 50.0);
    assert_at(&s, 0.5, 0.0);
    touch(&mut s, 45.0, 50.0);
    assert_at(&s, -0.25, 0.0);
}

#[test]
fn moving_the_finger_up_the_pad_pushes_the_stick_up() {
    let mut s = spring();
    touch(&mut s, 50.0, 50.0);
    touch(&mut s, 50.0, 40.0);
    assert_at(&s, 0.0, 0.5);
    touch(&mut s, 50.0, 70.0);
    assert_at(&s, 0.0, -1.0);
}

#[test]
fn travel_past_the_edge_stays_at_maximum_and_comes_back_at_once() {
    let mut s = spring();
    touch(&mut s, 0.0, 0.0);
    touch(&mut s, 60.0, 0.0);
    assert_at(&s, 1.0, 0.0);
    touch(&mut s, 50.0, 0.0);
    assert_at(&s, 0.5, 0.0);
}

#[test]
fn diagonal_travel_is_limited_to_the_circle() {
    let mut s = spring();
    touch(&mut s, 0.0, 0.0);
    touch(&mut s, 100.0, -100.0);
    let d = std::f32::consts::FRAC_1_SQRT_2;
    assert_at(&s, d, d);
}

#[test]
fn lifting_returns_to_center_by_default() {
    let mut s = spring();
    touch(&mut s, 0.0, 0.0);
    touch(&mut s, 10.0, 0.0);
    s.apply(Contact::Lifted);
    assert_at(&s, 0.0, 0.0);
}

#[test]
fn hold_position_keeps_the_stick_and_the_next_touch_continues_without_a_jump() {
    let mut s = stick(LiftBehavior::HoldPosition, 0.0);
    touch(&mut s, 0.0, 0.0);
    touch(&mut s, 10.0, 0.0);
    s.apply(Contact::Lifted);
    assert_at(&s, 0.5, 0.0);
    touch(&mut s, 90.0, 90.0);
    assert_at(&s, 0.5, 0.0);
    touch(&mut s, 95.0, 90.0);
    assert_at(&s, 0.75, 0.0);
}

#[test]
fn reset_centers_even_when_holding_and_a_resting_finger_does_not_jump() {
    let mut s = stick(LiftBehavior::HoldPosition, 0.0);
    touch(&mut s, 0.0, 0.0);
    touch(&mut s, 10.0, 0.0);
    s.reset();
    assert_at(&s, 0.0, 0.0);
    touch(&mut s, 14.0, 0.0);
    assert_at(&s, 0.0, 0.0);
    touch(&mut s, 18.0, 0.0);
    assert_at(&s, 0.2, 0.0);
}

#[test]
fn dead_zone_ignores_small_deflection_and_rescales_the_rest() {
    let mut s = stick(LiftBehavior::ReturnToCenter, 0.2);
    touch(&mut s, 0.0, 0.0);
    touch(&mut s, 3.0, 0.0);
    assert_at(&s, 0.0, 0.0);
    touch(&mut s, 4.0, 0.0);
    assert_at(&s, 0.0, 0.0);
    touch(&mut s, 12.0, 0.0);
    assert_at(&s, 0.5, 0.0);
    touch(&mut s, 20.0, 0.0);
    assert_at(&s, 1.0, 0.0);
}

#[test]
fn dead_zone_is_round_so_it_rescales_every_direction_alike() {
    let mut s = stick(LiftBehavior::ReturnToCenter, 0.2);
    touch(&mut s, 0.0, 0.0);
    touch(&mut s, 0.0, -12.0);
    assert_at(&s, 0.0, 0.5);
    touch(&mut s, -7.2, -9.6);
    assert_at(&s, -0.3, 0.4);
}

#[test]
fn dead_zone_does_not_lose_travel_made_inside_it() {
    let mut s = stick(LiftBehavior::ReturnToCenter, 0.2);
    touch(&mut s, 0.0, 0.0);
    for x in 1..=12 {
        touch(&mut s, x as f32, 0.0);
    }
    assert_at(&s, 0.5, 0.0);
}

#[test]
fn deflection_is_always_inside_the_unit_circle() {
    let d = Deflection::new(3.0, -4.0);
    assert!((d.x() - 0.6).abs() < 1e-6 && (d.y() + 0.8).abs() < 1e-6);
    let inside = Deflection::new(0.3, -0.4);
    assert_eq!((inside.x(), inside.y()), (0.3, -0.4));
    assert_eq!(Deflection::new(f32::NAN, 1.0), Deflection::NEUTRAL);
}

#[test]
fn settings_reject_values_that_cannot_work() {
    let new = |travel, dead_zone| StickSettings::new(LiftBehavior::default(), travel, dead_zone);
    assert_eq!(new(0.0, 0.1), Err(SettingsError::FullTravel));
    assert_eq!(new(-1.0, 0.1), Err(SettingsError::FullTravel));
    assert_eq!(new(f32::NAN, 0.1), Err(SettingsError::FullTravel));
    assert_eq!(new(f32::INFINITY, 0.1), Err(SettingsError::FullTravel));
    assert_eq!(new(20.0, -0.1), Err(SettingsError::DeadZone));
    assert_eq!(new(20.0, 1.0), Err(SettingsError::DeadZone));
    assert_eq!(new(20.0, f32::NAN), Err(SettingsError::DeadZone));
    assert!(new(20.0, 0.0).is_ok());
    assert!(new(0.001, 0.99).is_ok());
}

#[test]
fn default_settings_spring_back_sharply_with_a_small_dead_zone() {
    let expected = StickSettings::new(LiftBehavior::ReturnToCenter, 10.0, 0.05).unwrap();
    assert_eq!(StickSettings::default(), expected);
}
