use padpilot_core::overlay::layout::DEFAULT_JOYSTICK_SIDE;
use padpilot_core::window_state::{Opacity, WindowState};

#[test]
fn a_first_start_has_no_position_the_default_size_and_full_opacity() {
    let state = WindowState::default();
    assert_eq!(state.position, None);
    assert_eq!(state.joystick_side, DEFAULT_JOYSTICK_SIDE);
    assert_eq!(state.opacity, Opacity::FULL);
}

#[test]
fn what_is_saved_comes_back() {
    let state = WindowState {
        position: Some((-1200, 340)),
        joystick_side: 312.5,
        opacity: Opacity::new(0.6),
    };
    assert_eq!(WindowState::parse(&state.to_toml()), state);
}

#[test]
fn a_damaged_file_never_gives_an_unusable_window() {
    assert_eq!(
        WindowState::parse("not toml at all ["),
        WindowState::default()
    );
    let state = WindowState::parse("x = 10\njoystick_side = 50\nopacity = 0.01\n");
    assert_eq!(state.position, None);
    assert_eq!(state.joystick_side, 200.0);
    assert_eq!(state.opacity, Opacity::new(0.2));
    let state = WindowState::parse("joystick_side = nan\nopacity = nan\n");
    assert_eq!(state.joystick_side, DEFAULT_JOYSTICK_SIDE);
    assert_eq!(state.opacity, Opacity::FULL);
}

#[test]
fn opacity_never_goes_below_a_fifth() {
    assert_eq!(Opacity::new(0.0), Opacity::new(0.2));
    assert_eq!(Opacity::new(2.0), Opacity::FULL);
    assert_eq!(Opacity::new(0.2).alpha(), 51);
    assert_eq!(Opacity::FULL.alpha(), 255);
}

#[test]
fn the_opacity_button_steps_down_by_a_fifth_and_wraps_to_full() {
    let mut opacity = Opacity::FULL;
    let mut seen = Vec::new();
    for _ in 0..5 {
        opacity = opacity.next();
        seen.push(opacity.alpha());
    }
    assert_eq!(seen, [204, 153, 102, 51, 255]);
    assert_eq!(Opacity::new(0.75).next(), Opacity::new(0.6));
}
