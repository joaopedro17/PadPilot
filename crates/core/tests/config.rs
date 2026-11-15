use padpilot_core::config::{Config, LogLevel, TEMPLATE};
use padpilot_core::stick::{LiftBehavior, StickSettings};

fn travel(mm: f32) -> StickSettings {
    StickSettings::new(LiftBehavior::ReturnToCenter, mm, 0.05).unwrap()
}

#[test]
fn an_empty_file_gives_the_defaults() {
    assert_eq!(Config::parse("").unwrap(), Config::default());
}

#[test]
fn the_file_written_for_new_users_holds_exactly_the_defaults() {
    assert_eq!(Config::parse(TEMPLATE).unwrap(), Config::default());
}

#[test]
fn sensitivity_is_the_finger_travel_for_full_deflection() {
    let config = Config::parse("[stick]\nfull_travel_mm = 7.5\n").unwrap();
    assert_eq!(config.stick, travel(7.5));
}

#[test]
fn sensitivity_may_be_written_as_a_whole_number() {
    let config = Config::parse("[stick]\nfull_travel_mm = 8\n").unwrap();
    assert_eq!(config.stick, travel(8.0));
}

#[test]
fn a_sensitivity_that_cannot_work_is_rejected_by_name() {
    for value in ["0", "-3", "nan", "inf"] {
        let err = Config::parse(&format!("[stick]\nfull_travel_mm = {value}\n")).unwrap_err();
        assert!(err.to_string().contains("full_travel_mm"), "{value}: {err}");
    }
}

#[test]
fn a_misspelled_setting_is_reported_rather_than_ignored() {
    let err = Config::parse("[stick]\nfull_travel = 8\n").unwrap_err();
    assert!(err.to_string().contains("full_travel"), "{err}");
}

#[test]
fn broken_toml_is_reported_with_its_line() {
    let err = Config::parse("[stick]\nfull_travel_mm = \n").unwrap_err();
    assert!(err.to_string().contains("line 2"), "{err}");
}

#[test]
fn dead_zone_and_lift_behavior_are_read() {
    let config = Config::parse(
        "[stick]\nfull_travel_mm = 12\ndead_zone = 0.1\nlift_behavior = \"hold_position\"\n",
    )
    .unwrap();
    let expected = StickSettings::new(LiftBehavior::HoldPosition, 12.0, 0.1).unwrap();
    assert_eq!(config.stick, expected);
    let config = Config::parse("[stick]\nlift_behavior = \"return_to_center\"\n").unwrap();
    assert_eq!(config.stick, StickSettings::default());
}

#[test]
fn an_impossible_dead_zone_or_lift_behavior_is_rejected_by_name() {
    for text in ["[stick]\ndead_zone = 1.0\n", "[stick]\ndead_zone = -0.1\n"] {
        let err = Config::parse(text).unwrap_err();
        assert!(err.to_string().contains("dead_zone"), "{text}: {err}");
    }
    let err = Config::parse("[stick]\nlift_behavior = \"float\"\n").unwrap_err();
    assert!(err.to_string().contains("float"), "{err}");
}

#[test]
fn the_log_level_is_read_and_defaults_to_info() {
    assert_eq!(Config::default().log_level, LogLevel::Info);
    for (text, level) in [
        ("error", LogLevel::Error),
        ("warn", LogLevel::Warn),
        ("info", LogLevel::Info),
        ("debug", LogLevel::Debug),
        ("trace", LogLevel::Trace),
    ] {
        let config = Config::parse(&format!("[log]\nlevel = \"{text}\"\n")).unwrap();
        assert_eq!(config.log_level, level);
    }
    assert!(Config::parse("[log]\nlevel = \"loud\"\n").is_err());
}
