use padpilot_core::overlay::model::Mode;
use padpilot_core::watchdog::{Action, Heard, Message, Watch};

fn beat(mode: Mode) -> Heard {
    Heard::Message(Message::Beat(mode))
}

#[test]
fn heartbeats_keep_it_watching() {
    let mut watch = Watch::default();
    assert_eq!(watch.hear(beat(Mode::Normal)), Action::KeepWatching);
    assert_eq!(watch.hear(beat(Mode::Joystick)), Action::KeepWatching);
}

#[test]
fn a_goodbye_then_a_closed_pipe_is_a_normal_exit() {
    let mut watch = Watch::default();
    watch.hear(beat(Mode::Normal));
    assert_eq!(
        watch.hear(Heard::Message(Message::Goodbye)),
        Action::KeepWatching
    );
    assert_eq!(watch.hear(Heard::Closed), Action::Done);
}

#[test]
fn a_closed_pipe_without_goodbye_is_a_crash_in_the_last_mode_heard() {
    let mut watch = Watch::default();
    watch.hear(beat(Mode::Joystick));
    assert_eq!(watch.hear(Heard::Closed), Action::Crashed(Mode::Joystick));
}

#[test]
fn silence_means_the_app_hung_in_the_last_mode_heard() {
    let mut watch = Watch::default();
    watch.hear(beat(Mode::Joystick));
    watch.hear(beat(Mode::Normal));
    assert_eq!(watch.hear(Heard::Silence), Action::Hung(Mode::Normal));
}

#[test]
fn silence_while_closing_is_ended_without_alarming_the_user() {
    let mut watch = Watch::default();
    watch.hear(Heard::Message(Message::Goodbye));
    assert_eq!(watch.hear(Heard::Silence), Action::HungClosing);
}

#[test]
fn before_any_heartbeat_the_app_counts_as_in_normal_mode() {
    assert_eq!(
        Watch::default().hear(Heard::Closed),
        Action::Crashed(Mode::Normal)
    );
}

#[test]
fn messages_survive_the_trip_through_one_byte() {
    for message in [
        Message::Beat(Mode::Normal),
        Message::Beat(Mode::Joystick),
        Message::Goodbye,
    ] {
        assert_eq!(Message::from_byte(message.byte()), Some(message));
    }
    let bytes = [
        Message::Beat(Mode::Normal),
        Message::Beat(Mode::Joystick),
        Message::Goodbye,
    ]
    .map(Message::byte);
    assert!(bytes[0] != bytes[1] && bytes[1] != bytes[2] && bytes[0] != bytes[2]);
    assert_eq!(Message::from_byte(b'?'), None);
}
