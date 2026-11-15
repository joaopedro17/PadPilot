use padpilot_core::stick::{Contact, Point};
use padpilot_core::touchpad::{AxisRange, FirstContact, PadScale, RawContact};

fn finger(id: u32, tip: bool, x: f32, y: f32) -> RawContact {
    RawContact {
        id,
        tip,
        position: Point { x, y },
    }
}

fn touching(x: f32, y: f32) -> Option<Contact> {
    Some(Contact::Touching(Point { x, y }))
}

#[test]
fn nothing_happens_while_no_finger_is_down() {
    let mut first = FirstContact::default();
    assert_eq!(first.frame(&[]), None);
    assert_eq!(first.frame(&[finger(4, false, 1.0, 1.0)]), None);
}

#[test]
fn follows_the_finger_from_touch_down_to_lift() {
    let mut first = FirstContact::default();
    assert_eq!(
        first.frame(&[finger(7, true, 10.0, 20.0)]),
        touching(10.0, 20.0)
    );
    assert_eq!(
        first.frame(&[finger(7, true, 12.0, 19.0)]),
        touching(12.0, 19.0)
    );
    assert_eq!(
        first.frame(&[finger(7, false, 12.0, 19.0)]),
        Some(Contact::Lifted)
    );
    assert_eq!(first.frame(&[]), None);
}

#[test]
fn ignores_additional_fingers_wherever_they_are_listed() {
    let mut first = FirstContact::default();
    first.frame(&[finger(1, true, 10.0, 10.0)]);
    let both = [finger(2, true, 80.0, 80.0), finger(1, true, 11.0, 10.0)];
    assert_eq!(first.frame(&both), touching(11.0, 10.0));
}

#[test]
fn a_finger_that_vanishes_without_a_lift_report_counts_as_lifted() {
    let mut first = FirstContact::default();
    first.frame(&[finger(1, true, 10.0, 10.0)]);
    assert_eq!(first.frame(&[]), Some(Contact::Lifted));
    assert_eq!(first.frame(&[]), None);
}

#[test]
fn when_the_first_finger_lifts_a_remaining_finger_takes_over_on_the_next_frame() {
    let mut first = FirstContact::default();
    first.frame(&[finger(1, true, 10.0, 10.0)]);
    let lifting = [finger(1, false, 10.0, 10.0), finger(2, true, 80.0, 80.0)];
    assert_eq!(first.frame(&lifting), Some(Contact::Lifted));
    assert_eq!(
        first.frame(&[finger(2, true, 81.0, 80.0)]),
        touching(81.0, 80.0)
    );
}

fn axis(logical_max: i32, physical_max: i32, unit: u32, unit_exponent: i32) -> AxisRange {
    AxisRange {
        logical_min: 0,
        logical_max,
        physical_min: 0,
        physical_max,
        unit,
        unit_exponent,
    }
}

#[track_caller]
fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn reads_the_real_size_in_centimeters() {
    // 10.24 cm across 1024 counts
    assert_close(axis(1024, 1024, 0x11, -2).mm_per_count().unwrap(), 0.1);
    // the exponent may arrive as the raw nibble: 0xE means -2
    assert_close(axis(1024, 1024, 0x11, 0xE).mm_per_count().unwrap(), 0.1);
    assert_close(axis(100, 10, 0x11, 0).mm_per_count().unwrap(), 1.0);
}

#[test]
fn reads_the_real_size_in_inches() {
    // 4.00 in across 1016 counts
    assert_close(axis(1016, 400, 0x13, -2).mm_per_count().unwrap(), 0.1);
}

#[test]
fn offsets_in_the_ranges_do_not_change_the_scale() {
    let range = AxisRange {
        logical_min: 100,
        logical_max: 1124,
        physical_min: 50,
        physical_max: 1074,
        unit: 0x11,
        unit_exponent: -2,
    };
    assert_close(range.mm_per_count().unwrap(), 0.1);
}

#[test]
fn a_device_that_does_not_state_its_size_has_no_scale() {
    assert_eq!(axis(1024, 1024, 0, -2).mm_per_count(), None);
    assert_eq!(axis(1024, 0, 0x11, -2).mm_per_count(), None);
    assert_eq!(axis(0, 1024, 0x11, -2).mm_per_count(), None);
    assert_eq!(axis(-5, 1024, 0x11, -2).mm_per_count(), None);
}

#[test]
fn pad_scale_converts_counts_to_millimeters() {
    let scale = PadScale::new(axis(1000, 1000, 0x11, -2), axis(500, 250, 0x11, -1));
    assert!(scale.measured);
    let point = scale.point(500, 100);
    assert_close(point.x, 50.0);
    assert_close(point.y, 50.0);
}

#[test]
fn pad_scale_assumes_a_width_when_the_size_is_unknown() {
    for y in [axis(1000, 0, 0, 0), axis(1000, 500, 0x11, -2)] {
        let scale = PadScale::new(axis(2000, 0, 0, 0), y);
        assert!(!scale.measured);
        let point = scale.point(2000, 1000);
        assert_close(point.x, 100.0);
        assert_close(point.y, 50.0);
    }
}

#[test]
fn the_largest_positive_exponent_is_not_mistaken_for_a_negative_one() {
    assert_close(axis(10_000_000, 1, 0x11, 7).mm_per_count().unwrap(), 10.0);
}
