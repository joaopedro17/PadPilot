use padpilot_core::config::{Config, TEMPLATE};
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
