//! Turns finger movement on the touchpad into a stick position.

/// Finger position on the touchpad. Y grows downward, as the device reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// What the touchpad reports for the tracked finger.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Contact {
    Touching(Point),
    Lifted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiftBehavior {
    #[default]
    ReturnToCenter,
    HoldPosition,
}

/// Stick position inside the unit circle. X: right is positive. Y: up is positive.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Deflection {
    x: f32,
    y: f32,
}

impl Deflection {
    pub const NEUTRAL: Self = Self { x: 0.0, y: 0.0 };

    /// Values outside the circle are pulled back onto its edge.
    pub fn new(x: f32, y: f32) -> Self {
        let length = x.hypot(y);
        if !length.is_finite() {
            Self::NEUTRAL
        } else if length > 1.0 {
            Self {
                x: x / length,
                y: y / length,
            }
        } else {
            Self { x, y }
        }
    }

    fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    fn scaled(self, factor: f32) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
        }
    }

    pub fn x(self) -> f32 {
        self.x
    }

    pub fn y(self) -> f32 {
        self.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsError {
    FullTravel,
    DeadZone,
}

/// `full_travel`: finger travel for full deflection, in the unit of [`Point`].
/// `dead_zone`: fraction of the stick's radius around center with no effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StickSettings {
    lift_behavior: LiftBehavior,
    full_travel: f32,
    dead_zone: f32,
}

impl StickSettings {
    pub fn new(
        lift_behavior: LiftBehavior,
        full_travel: f32,
        dead_zone: f32,
    ) -> Result<Self, SettingsError> {
        if !(full_travel.is_finite() && full_travel > 0.0) {
            return Err(SettingsError::FullTravel);
        }
        if !(0.0..1.0).contains(&dead_zone) {
            return Err(SettingsError::DeadZone);
        }
        Ok(Self {
            lift_behavior,
            full_travel,
            dead_zone,
        })
    }
}

impl Default for StickSettings {
    /// Springs back to center; 20 mm of finger travel for full deflection.
    fn default() -> Self {
        Self {
            lift_behavior: LiftBehavior::ReturnToCenter,
            full_travel: 20.0,
            dead_zone: 0.05,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Stick {
    settings: StickSettings,
    /// Where the stick is, before the dead zone.
    position: Deflection,
    /// Where the finger was last seen; `None` while it is off the pad.
    finger: Option<Point>,
}

impl Stick {
    pub fn new(settings: StickSettings) -> Self {
        Self {
            settings,
            position: Deflection::NEUTRAL,
            finger: None,
        }
    }

    pub fn apply(&mut self, contact: Contact) {
        match contact {
            Contact::Touching(now) => {
                if let Some(before) = self.finger {
                    let travel = self.settings.full_travel;
                    self.position = Deflection::new(
                        self.position.x + (now.x - before.x) / travel,
                        self.position.y - (now.y - before.y) / travel,
                    );
                }
                self.finger = Some(now);
            }
            Contact::Lifted => {
                self.finger = None;
                if self.settings.lift_behavior == LiftBehavior::ReturnToCenter {
                    self.position = Deflection::NEUTRAL;
                }
            }
        }
    }

    /// Centers the stick. A finger still on the pad starts over from center.
    pub fn reset(&mut self) {
        self.position = Deflection::NEUTRAL;
        self.finger = None;
    }

    /// The stick position the game should see, with the dead zone applied.
    pub fn output(&self) -> Deflection {
        let dead_zone = self.settings.dead_zone;
        let length = self.position.length();
        if length <= dead_zone {
            return Deflection::NEUTRAL;
        }
        self.position
            .scaled((length - dead_zone) / (1.0 - dead_zone) / length)
    }
}
