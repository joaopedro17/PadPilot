//! Where everything sits in the window: a slim title bar above a square
//! joystick area. All results are in physical pixels.

/// Sizes at 100% display scaling.
const TITLE_BAR_HEIGHT: f32 = 32.0;
const BUTTON_SIZE: f32 = 32.0;
pub const MIN_JOYSTICK_SIDE: f32 = 200.0;
pub const DEFAULT_JOYSTICK_SIDE: f32 = 240.0;
/// Gap between the joystick's base and the edge of the window.
const BASE_MARGIN: f32 = 2.0;
/// The resize handle: a square in the bottom-right corner, outside the circle.
const GRIP_SIZE: f32 = 20.0;
/// Each notch of scrolling changes the joystick's side by this factor.
const SCROLL_GROWTH: f32 = 1.1;

/// Title bar buttons, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Opacity,
    Settings,
    Minimize,
    Close,
}

impl Button {
    pub const ALL: [Button; 4] = [
        Button::Opacity,
        Button::Settings,
        Button::Minimize,
        Button::Close,
    ];
}

/// What lies under a point of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    DragArea,
    Button(Button),
    Joystick,
    ResizeHandle,
    /// Transparent: around the circle, or off the window.
    Outside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// The joystick's side at 100% scaling.
    joystick_side: f32,
    side: i32,
    bar: i32,
    button: i32,
    scale: f32,
}

impl Layout {
    /// `joystick_side` is the side of the joystick area at 100% scaling;
    /// `scale` is the display scaling (1.5 for 150%).
    pub fn new(joystick_side: f32, scale: f32) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        // `max` also replaces a NaN side with the minimum.
        let side = joystick_side.max(MIN_JOYSTICK_SIDE);
        let pixels = |size: f32| (size * scale).round() as i32;
        Self {
            joystick_side: side,
            side: pixels(side),
            bar: pixels(TITLE_BAR_HEIGHT),
            button: pixels(BUTTON_SIZE),
            scale,
        }
    }

    /// The side the layout was made for, at 100% scaling.
    pub fn joystick_side(&self) -> f32 {
        self.joystick_side
    }

    /// The side at 100% scaling that puts the window's bottom-right corner
    /// nearest `(x, y)`, in window pixels; the square follows the farther
    /// direction.
    pub fn side_for_corner(&self, x: i32, y: i32) -> f32 {
        let pixels = x.max(y - self.bar) as f32;
        (pixels / self.scale).max(MIN_JOYSTICK_SIDE)
    }

    /// The side at 100% scaling after scrolling `notches` (up is positive).
    pub fn side_after_scroll(&self, notches: f32) -> f32 {
        (self.joystick_side * SCROLL_GROWTH.powf(notches)).max(MIN_JOYSTICK_SIDE)
    }

    pub fn grip_rect(&self) -> Rect {
        let size = (GRIP_SIZE * self.scale).round() as i32;
        Rect {
            left: self.width() - size,
            top: self.height() - size,
            right: self.width(),
            bottom: self.height(),
        }
    }

    pub fn width(&self) -> i32 {
        self.side
    }

    pub fn height(&self) -> i32 {
        self.bar + self.side
    }

    pub fn title_bar_height(&self) -> i32 {
        self.bar
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn button_rect(&self, button: Button) -> Rect {
        let from_right = (Button::ALL.len() - button as usize) as i32;
        let left = self.side - from_right * self.button;
        Rect {
            left,
            top: 0,
            right: left + self.button,
            bottom: self.bar,
        }
    }

    /// Center of the joystick, in window pixels.
    pub fn joystick_center(&self) -> (f32, f32) {
        let half = self.side as f32 / 2.0;
        (half, self.bar as f32 + half)
    }

    /// Radius of the joystick's round base.
    pub fn base_radius(&self) -> f32 {
        self.side as f32 / 2.0 - BASE_MARGIN * self.scale
    }

    pub fn region_at(&self, x: i32, y: i32) -> Region {
        if !(0..self.width()).contains(&x) || !(0..self.height()).contains(&y) {
            return Region::Outside;
        }
        if y < self.bar {
            let buttons_left = self.button_rect(Button::ALL[0]).left;
            return match usize::try_from((x - buttons_left).div_euclid(self.button)) {
                Ok(index) => Region::Button(Button::ALL[index]),
                Err(_) => Region::DragArea,
            };
        }
        let grip = self.grip_rect();
        if x >= grip.left && y >= grip.top {
            return Region::ResizeHandle;
        }
        let (center_x, center_y) = self.joystick_center();
        let distance = (x as f32 + 0.5 - center_x).hypot(y as f32 + 0.5 - center_y);
        if distance <= self.base_radius() {
            Region::Joystick
        } else {
            Region::Outside
        }
    }

    /// Where the cursor is kept in joystick mode: the largest square inside
    /// the base, so every click lands on the joystick and never on the game.
    pub fn cursor_cage(&self) -> Rect {
        let (center_x, center_y) = self.joystick_center();
        let half = (self.base_radius() * std::f32::consts::FRAC_1_SQRT_2).floor() - 1.0;
        Rect {
            left: (center_x - half).ceil() as i32,
            top: (center_y - half).ceil() as i32,
            right: (center_x + half).floor() as i32,
            bottom: (center_y + half).floor() as i32,
        }
    }
}
