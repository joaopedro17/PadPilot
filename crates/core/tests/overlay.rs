use padpilot_core::gamepad::GamepadState;
use padpilot_core::overlay::layout::{Button, Layout, Rect, Region};
use padpilot_core::overlay::model::{Command, HitKind, Mode, Overlay};
use padpilot_core::overlay::paint::{Canvas, paint};
use padpilot_core::stick::{Contact, Deflection, LiftBehavior, Point, StickSettings};

// ---- layout ----

#[test]
fn window_is_a_title_bar_above_a_square_joystick_area() {
    let layout = Layout::new(240.0, 1.0);
    assert_eq!((layout.width(), layout.height()), (240, 272));
    assert_eq!(layout.title_bar_height(), 32);
}

#[test]
fn every_size_follows_display_scaling() {
    let layout = Layout::new(240.0, 1.5);
    assert_eq!((layout.width(), layout.height()), (360, 408));
    assert_eq!(
        layout.button_rect(Button::Close),
        Rect {
            left: 312,
            top: 0,
            right: 360,
            bottom: 48
        }
    );
}

#[test]
fn joystick_area_never_gets_smaller_than_the_minimum() {
    assert_eq!(Layout::new(50.0, 1.0).width(), 200);
    assert_eq!(Layout::new(50.0, 2.0).width(), 400);
    assert_eq!(Layout::new(f32::NAN, 1.0).width(), 200);
}

#[test]
fn buttons_sit_at_the_right_of_the_title_bar_in_order() {
    let layout = Layout::new(240.0, 1.0);
    let lefts: Vec<i32> = Button::ALL
        .iter()
        .map(|&b| layout.button_rect(b).left)
        .collect();
    assert_eq!(lefts, [112, 144, 176, 208]);
    for button in Button::ALL {
        let rect = layout.button_rect(button);
        assert_eq!((rect.right - rect.left, rect.top, rect.bottom), (32, 0, 32));
        assert_eq!(layout.region_at(rect.left, 0), Region::Button(button));
        assert_eq!(layout.region_at(rect.right - 1, 31), Region::Button(button));
    }
}

#[test]
fn the_rest_of_the_title_bar_is_the_drag_area() {
    let layout = Layout::new(240.0, 1.0);
    assert_eq!(layout.region_at(0, 0), Region::DragArea);
    assert_eq!(layout.region_at(111, 31), Region::DragArea);
}

#[test]
fn the_joystick_is_a_circle_and_its_corners_are_outside() {
    let layout = Layout::new(240.0, 1.0);
    assert_eq!(layout.joystick_center(), (120.0, 152.0));
    assert_eq!(layout.base_radius(), 118.0);
    assert_eq!(layout.region_at(120, 152), Region::Joystick);
    assert_eq!(layout.region_at(120, 32 + 3), Region::Joystick);
    assert_eq!(layout.region_at(3, 152), Region::Joystick);
    assert_eq!(layout.region_at(0, 152), Region::Outside);
    assert_eq!(layout.region_at(4, 36), Region::Outside);
    assert_eq!(layout.region_at(235, 268), Region::Outside);
}

#[test]
fn points_off_the_window_are_outside() {
    let layout = Layout::new(240.0, 1.0);
    for (x, y) in [(-1, 5), (240, 5), (5, -1), (120, 272)] {
        assert_eq!(layout.region_at(x, y), Region::Outside, "({x}, {y})");
    }
}

#[test]
fn cursor_cage_is_centered_and_entirely_on_the_joystick() {
    for (side, scale) in [(200.0, 1.0), (240.0, 1.0), (333.0, 1.25), (500.0, 2.0)] {
        let layout = Layout::new(side, scale);
        let cage = layout.cursor_cage();
        for (x, y) in [
            (cage.left, cage.top),
            (cage.right - 1, cage.top),
            (cage.left, cage.bottom - 1),
            (cage.right - 1, cage.bottom - 1),
        ] {
            assert_eq!(
                layout.region_at(x, y),
                Region::Joystick,
                "({x}, {y}) of {cage:?}"
            );
        }
        let (cx, cy) = layout.joystick_center();
        assert!(((cage.left + cage.right) as f32 / 2.0 - cx).abs() <= 1.0);
        assert!(((cage.top + cage.bottom) as f32 / 2.0 - cy).abs() <= 1.0);
        // Large enough to be worth it: most of the inscribed square.
        assert!((cage.right - cage.left) as f32 > layout.base_radius() * 1.3);
        assert_eq!(cage.right - cage.left, cage.bottom - cage.top);
    }
}

// ---- model ----

fn overlay() -> Overlay {
    Overlay::new(StickSettings::new(LiftBehavior::HoldPosition, 20.0, 0.0).unwrap())
}

fn touch(overlay: &mut Overlay, x: f32, y: f32) -> Option<GamepadState> {
    overlay.contact(Contact::Touching(Point { x, y }))
}

const EVERYWHERE: [Region; 7] = [
    Region::DragArea,
    Region::Button(Button::Opacity),
    Region::Button(Button::Settings),
    Region::Button(Button::Minimize),
    Region::Button(Button::Close),
    Region::Joystick,
    Region::Outside,
];

#[test]
fn starts_in_normal_mode() {
    assert_eq!(overlay().mode(), Mode::Normal);
}

#[test]
fn right_clicking_the_joystick_enters_and_leaves_joystick_mode() {
    let mut o = overlay();
    assert_eq!(o.right_click(Region::Joystick), Some(Mode::Joystick));
    assert_eq!(o.mode(), Mode::Joystick);
    assert_eq!(o.right_click(Region::Joystick), Some(Mode::Normal));
    assert_eq!(o.mode(), Mode::Normal);
}

#[test]
fn right_clicking_anything_else_changes_nothing() {
    let mut o = overlay();
    for region in EVERYWHERE.into_iter().filter(|&r| r != Region::Joystick) {
        assert_eq!(o.right_click(region), None, "{region:?}");
        assert_eq!(o.mode(), Mode::Normal);
    }
}

#[test]
fn in_normal_mode_the_touchpad_does_not_drive_the_stick() {
    let mut o = overlay();
    assert_eq!(touch(&mut o, 0.0, 0.0), None);
    assert_eq!(touch(&mut o, 20.0, 0.0), None);
    assert_eq!(o.knob(), Deflection::NEUTRAL);
}

#[test]
fn in_joystick_mode_the_finger_drives_the_controller() {
    let mut o = overlay();
    o.right_click(Region::Joystick);
    assert_eq!(touch(&mut o, 0.0, 0.0), Some(GamepadState::default()));
    let state = touch(&mut o, 10.0, -20.0).unwrap();
    assert_eq!(
        (state.left_stick_x > 10_000, state.right_trigger > 200),
        (true, true)
    );
    assert!(o.knob().x() > 0.4 && o.knob().y() > 0.8);
}

#[test]
fn the_stick_is_centered_on_the_way_in_and_on_the_way_out() {
    let mut o = overlay();
    o.right_click(Region::Joystick);
    touch(&mut o, 0.0, 0.0);
    touch(&mut o, 20.0, 0.0);
    o.contact(Contact::Lifted); // hold_position keeps it deflected
    assert!(o.knob().x() > 0.9);

    o.right_click(Region::Joystick);
    assert_eq!(o.knob(), Deflection::NEUTRAL);

    o.right_click(Region::Joystick);
    assert_eq!(o.knob(), Deflection::NEUTRAL);
    // a finger resting on the pad while entering does not make it jump
    assert_eq!(touch(&mut o, 55.0, 5.0), Some(GamepadState::default()));
}

#[test]
fn leaving_joystick_mode_can_be_forced() {
    let mut o = overlay();
    assert!(!o.leave_joystick_mode());
    o.right_click(Region::Joystick);
    touch(&mut o, 0.0, 0.0);
    touch(&mut o, 20.0, 0.0);
    assert!(o.leave_joystick_mode());
    assert_eq!(o.mode(), Mode::Normal);
    assert_eq!(o.knob(), Deflection::NEUTRAL);
    assert!(!o.leave_joystick_mode());
}

#[test]
fn normal_mode_drags_by_the_title_bar_and_clicks_the_rest() {
    let o = overlay();
    assert_eq!(o.hit(Region::DragArea), HitKind::Caption);
    assert_eq!(o.hit(Region::Button(Button::Close)), HitKind::Client);
    assert_eq!(o.hit(Region::Joystick), HitKind::Client);
    assert_eq!(o.hit(Region::Outside), HitKind::PassThrough);
}

#[test]
fn joystick_mode_cannot_drag_the_window() {
    let mut o = overlay();
    o.right_click(Region::Joystick);
    assert_eq!(o.hit(Region::DragArea), HitKind::Client);
    assert_eq!(o.hit(Region::Joystick), HitKind::Client);
    assert_eq!(o.hit(Region::Outside), HitKind::PassThrough);
}

#[test]
fn normal_mode_buttons_minimize_and_close() {
    let o = overlay();
    assert_eq!(
        o.left_click(Region::Button(Button::Minimize)),
        Some(Command::Minimize)
    );
    assert_eq!(
        o.left_click(Region::Button(Button::Close)),
        Some(Command::Close)
    );
    assert_eq!(o.left_click(Region::Joystick), None);
    assert_eq!(o.left_click(Region::DragArea), None);
}

#[test]
fn joystick_mode_ignores_every_left_click() {
    let mut o = overlay();
    o.right_click(Region::Joystick);
    for region in EVERYWHERE {
        assert_eq!(o.left_click(region), None, "{region:?}");
    }
    assert_eq!(o.mode(), Mode::Joystick);
}

// ---- paint ----

struct Image {
    layout: Layout,
    pixels: Vec<u32>,
}

impl Image {
    fn at(&self, x: i32, y: i32) -> [u32; 4] {
        let p = self.pixels[(y * self.layout.width() + x) as usize];
        [p >> 24, (p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF]
    }

    fn alpha(&self, x: i32, y: i32) -> u32 {
        self.at(x, y)[0]
    }
}

fn render(mode: Mode, knob: Deflection) -> Image {
    let layout = Layout::new(240.0, 1.0);
    let (width, height) = (layout.width() as usize, layout.height() as usize);
    let mut pixels = vec![0xDEADBEEF; width * height];
    paint(
        &mut Canvas {
            width,
            height,
            pixels: &mut pixels,
        },
        &layout,
        mode,
        knob,
    );
    Image { layout, pixels }
}

#[test]
fn the_area_around_the_circle_is_fully_transparent() {
    let image = render(Mode::Normal, Deflection::NEUTRAL);
    for (x, y) in [(0, 32), (239, 32), (0, 271), (239, 271), (8, 44)] {
        assert_eq!(image.at(x, y), [0, 0, 0, 0], "({x}, {y})");
    }
}

#[test]
fn the_joystick_and_the_title_bar_are_solid() {
    let image = render(Mode::Normal, Deflection::NEUTRAL);
    assert_eq!(image.alpha(120, 152), 255);
    assert_eq!(image.alpha(20, 152), 255);
    assert!(image.alpha(60, 16) > 200);
}

#[test]
fn colors_never_exceed_their_alpha() {
    for mode in [Mode::Normal, Mode::Joystick] {
        let image = render(mode, Deflection::new(0.3, -0.6));
        for (i, p) in image.pixels.iter().enumerate() {
            let a = p >> 24;
            assert!(
                (p >> 16) & 0xFF <= a && (p >> 8) & 0xFF <= a && p & 0xFF <= a,
                "pixel {i} is {p:08X}"
            );
        }
    }
}

#[test]
fn the_knob_follows_the_stick() {
    let brightness = |image: &Image, x: i32, y: i32| image.at(x, y)[1..].iter().sum::<u32>();
    let centered = render(Mode::Normal, Deflection::NEUTRAL);
    let right = render(Mode::Normal, Deflection::new(1.0, 0.0));
    let up = render(Mode::Normal, Deflection::new(0.0, 1.0));
    // The knob is lighter than the well it sits in.
    let (well_left, well_right, well_top) = ((40, 152), (200, 152), (120, 72));
    let knob_at = |image: &Image, (x, y): (i32, i32)| {
        brightness(image, x, y) > brightness(&centered, well_left.0, well_left.1) + 20
    };
    assert!(!knob_at(&centered, well_right) && !knob_at(&centered, well_top));
    assert!(
        knob_at(&right, well_right) && !knob_at(&right, well_left) && !knob_at(&right, well_top)
    );
    assert!(knob_at(&up, well_top) && !knob_at(&up, well_right));
}

#[test]
fn joystick_mode_highlights_the_ring_and_fades_the_title_bar() {
    let normal = render(Mode::Normal, Deflection::NEUTRAL);
    let active = render(Mode::Joystick, Deflection::NEUTRAL);
    let ring = (120 + 113, 152);
    let [_, r, _, b] = active.at(ring.0, ring.1);
    assert!(b > r + 60, "active ring should be blue, got r={r} b={b}");
    let [_, r, _, b] = normal.at(ring.0, ring.1);
    assert!(b < r + 30, "normal ring should be gray, got r={r} b={b}");
    assert!(active.alpha(60, 16) + 80 < normal.alpha(60, 16));
}

#[test]
fn title_bar_shows_an_icon_on_every_button() {
    let image = render(Mode::Normal, Deflection::NEUTRAL);
    let background = image.at(60, 16);
    for button in Button::ALL {
        let rect = image.layout.button_rect(button);
        let drawn = (rect.left..rect.right)
            .flat_map(|x| (rect.top..rect.bottom).map(move |y| (x, y)))
            .filter(|&(x, y)| image.at(x, y) != background)
            .count();
        assert!(drawn > 20, "{button:?} has {drawn} icon pixels");
    }
}
