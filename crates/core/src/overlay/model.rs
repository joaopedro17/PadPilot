//! How the window reacts: normal mode (move it, use its buttons) and
//! joystick mode (the touchpad drives the stick, everything else is ignored).

use super::layout::{Button, Region};
use crate::gamepad::{GamepadState, racing_state};
use crate::stick::{Contact, Deflection, Stick, StickSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Joystick,
}

/// How the window system should treat a point of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitKind {
    /// Dragging here moves the window.
    Caption,
    /// Clicks here come to the window.
    Client,
    /// Clicks here go to whatever is behind the window.
    PassThrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Step the window's opacity down, wrapping to full.
    Opacity,
    Minimize,
    Close,
}

#[derive(Debug, Clone)]
pub struct Overlay {
    mode: Mode,
    stick: Stick,
}

impl Overlay {
    pub fn new(settings: StickSettings) -> Self {
        Self {
            mode: Mode::Normal,
            stick: Stick::new(settings),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Where to draw the knob.
    pub fn knob(&self) -> Deflection {
        self.stick.output()
    }

    pub fn hit(&self, region: Region) -> HitKind {
        match (region, self.mode) {
            (Region::Outside, _) | (Region::ResizeHandle, Mode::Joystick) => HitKind::PassThrough,
            (Region::DragArea, Mode::Normal) => HitKind::Caption,
            _ => HitKind::Client,
        }
    }

    /// Right-clicking the joystick switches mode. Returns the new mode. The
    /// stick is centered on the way in and on the way out.
    pub fn right_click(&mut self, region: Region) -> Option<Mode> {
        if region != Region::Joystick {
            return None;
        }
        self.mode = match self.mode {
            Mode::Normal => Mode::Joystick,
            Mode::Joystick => Mode::Normal,
        };
        self.stick.reset();
        Some(self.mode)
    }

    /// Leaves joystick mode whatever the reason. Returns whether it was on.
    pub fn leave_joystick_mode(&mut self) -> bool {
        let was_on = self.mode == Mode::Joystick;
        self.mode = Mode::Normal;
        self.stick.reset();
        was_on
    }

    /// Resizing, by the handle or by scrolling, works in normal mode only.
    pub fn can_resize(&self) -> bool {
        self.mode == Mode::Normal
    }

    /// Whether pressing the left button here starts resizing.
    pub fn starts_resize(&self, region: Region) -> bool {
        self.can_resize() && region == Region::ResizeHandle
    }

    pub fn left_click(&self, region: Region) -> Option<Command> {
        match (self.mode, region) {
            (Mode::Normal, Region::Button(Button::Opacity)) => Some(Command::Opacity),
            (Mode::Normal, Region::Button(Button::Minimize)) => Some(Command::Minimize),
            (Mode::Normal, Region::Button(Button::Close)) => Some(Command::Close),
            _ => None,
        }
    }

    /// Feeds the touchpad to the stick. Returns what the controller should
    /// report; `None` in normal mode, where the touchpad is just a mouse.
    pub fn contact(&mut self, contact: Contact) -> Option<GamepadState> {
        if self.mode == Mode::Normal {
            return None;
        }
        self.stick.apply(contact);
        Some(racing_state(self.stick.output()))
    }
}
