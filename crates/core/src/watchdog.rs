//! What the watchdog does about what it hears from the main app.
//!
//! The main app writes one byte per message to the watchdog's standard
//! input: a heartbeat every [`HEARTBEAT_EVERY`], and a goodbye when it closes
//! normally. The pipe closes when the main app ends, however it ends.

use std::time::Duration;

use crate::overlay::model::Mode;

pub const HEARTBEAT_EVERY: Duration = Duration::from_millis(100);
/// No message for this long means the main app is stuck.
pub const SILENCE_LIMIT: Duration = Duration::from_millis(1000);

const BEAT_NORMAL: u8 = b'n';
const BEAT_JOYSTICK: u8 = b'j';
const GOODBYE: u8 = b'q';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// The main app is alive, in this mode.
    Beat(Mode),
    /// The main app is closing normally.
    Goodbye,
}

impl Message {
    pub fn byte(self) -> u8 {
        match self {
            Self::Beat(Mode::Normal) => BEAT_NORMAL,
            Self::Beat(Mode::Joystick) => BEAT_JOYSTICK,
            Self::Goodbye => GOODBYE,
        }
    }

    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            BEAT_NORMAL => Some(Self::Beat(Mode::Normal)),
            BEAT_JOYSTICK => Some(Self::Beat(Mode::Joystick)),
            GOODBYE => Some(Self::Goodbye),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    Message(Message),
    /// The pipe closed: the main app has ended.
    Closed,
    /// Nothing for [`SILENCE_LIMIT`].
    Silence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    KeepWatching,
    /// The main app closed normally; the watchdog can go.
    Done,
    /// The main app ended without a goodbye: free the cursor, tell the user.
    Crashed(Mode),
    /// The main app is stuck: free the cursor, end it, tell the user.
    Hung(Mode),
    /// Stuck while closing: free the cursor and end it, quietly.
    HungClosing,
}

#[derive(Debug, Default)]
pub struct Watch {
    mode: Option<Mode>,
    closing: bool,
}

impl Watch {
    pub fn hear(&mut self, heard: Heard) -> Action {
        let mode = self.mode.unwrap_or(Mode::Normal);
        match heard {
            Heard::Message(Message::Beat(now)) => {
                self.mode = Some(now);
                Action::KeepWatching
            }
            Heard::Message(Message::Goodbye) => {
                self.closing = true;
                Action::KeepWatching
            }
            Heard::Closed if self.closing => Action::Done,
            Heard::Closed => Action::Crashed(mode),
            Heard::Silence if self.closing => Action::HungClosing,
            Heard::Silence => Action::Hung(mode),
        }
    }
}
