//! Where the window was, how big and how see-through: saved by PadPilot in
//! `window.toml`, apart from the user's settings. Reading it never fails; a
//! damaged or edited file gives a usable window.

use serde::Deserialize;

use crate::overlay::layout::{DEFAULT_JOYSTICK_SIDE, MIN_JOYSTICK_SIDE};

/// How opaque the whole window is. Never below [`Opacity::MIN`], so the
/// window cannot become invisible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opacity(f32);

impl Opacity {
    pub const MIN: f32 = 0.2;
    pub const FULL: Self = Self(1.0);
    /// What the title bar button steps through, after full.
    const STEPS: [f32; 4] = [0.8, 0.6, 0.4, 0.2];

    pub fn new(value: f32) -> Self {
        if value.is_nan() {
            Self::FULL
        } else {
            Self(value.clamp(Self::MIN, 1.0))
        }
    }

    /// The next step down, or full after the last.
    pub fn next(self) -> Self {
        Self::STEPS
            .into_iter()
            // Compared as the window system sees them, 0 to 255, so values a
            // rounding error apart count as the same step.
            .find(|&step| Self(step).alpha() < self.alpha())
            .map_or(Self::FULL, Self)
    }

    pub fn value(self) -> f32 {
        self.0
    }

    /// For the window system: 0 to 255.
    pub fn alpha(self) -> u8 {
        (self.0 * 255.0).round() as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowState {
    /// Top-left corner on screen; `None`: the default corner.
    pub position: Option<(i32, i32)>,
    /// At 100% display scaling.
    pub joystick_side: f32,
    pub opacity: Opacity,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            position: None,
            joystick_side: DEFAULT_JOYSTICK_SIDE,
            opacity: Opacity::FULL,
        }
    }
}

impl WindowState {
    pub fn parse(text: &str) -> Self {
        let Ok(file) = toml::from_str::<File>(text) else {
            return Self::default();
        };
        let joystick_side = match file.joystick_side {
            Some(side) if !side.is_nan() => side.max(MIN_JOYSTICK_SIDE),
            _ => DEFAULT_JOYSTICK_SIDE,
        };
        Self {
            position: file.x.zip(file.y),
            joystick_side,
            opacity: file.opacity.map_or(Opacity::FULL, Opacity::new),
        }
    }

    pub fn to_toml(&self) -> String {
        let position = self
            .position
            .map(|(x, y)| format!("x = {x}\ny = {y}\n"))
            .unwrap_or_default();
        format!(
            "# Saved by PadPilot: where the window is, its size and opacity.\n\
             {position}joystick_side = {}\nopacity = {}\n",
            self.joystick_side,
            self.opacity.value()
        )
    }
}

/// The file as written. Anything missing or unknown is ignored.
#[derive(Deserialize, Default)]
#[serde(default)]
struct File {
    x: Option<i32>,
    y: Option<i32>,
    joystick_side: Option<f32>,
    opacity: Option<f32>,
}
