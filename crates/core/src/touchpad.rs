//! Makes sense of what a Precision Touchpad reports, without touching any
//! operating system API.

use crate::stick::{Contact, Point};

/// One finger slot of a touchpad frame, as the device reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawContact {
    pub id: u32,
    /// Tip switch: the finger is on the surface.
    pub tip: bool,
    pub position: Point,
}

/// Follows the first finger that touched down and ignores every other one.
#[derive(Debug, Default)]
pub struct FirstContact {
    tracked: Option<u32>,
}

impl FirstContact {
    /// Takes one complete frame (every contact the pad currently reports).
    /// Returns `None` while no finger is being followed.
    pub fn frame(&mut self, contacts: &[RawContact]) -> Option<Contact> {
        let Some(id) = self.tracked else {
            let landed = contacts.iter().find(|contact| contact.tip)?;
            self.tracked = Some(landed.id);
            return Some(Contact::Touching(landed.position));
        };
        // A finger missing from the frame is treated as lifted: a missed
        // lift would leave the stick deflected.
        match contacts.iter().find(|contact| contact.id == id) {
            Some(contact) if contact.tip => Some(Contact::Touching(contact.position)),
            _ => {
                self.tracked = None;
                Some(Contact::Lifted)
            }
        }
    }
}

/// How the device describes one axis in its HID report descriptor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisRange {
    pub logical_min: i32,
    pub logical_max: i32,
    pub physical_min: i32,
    pub physical_max: i32,
    /// HID unit code: `0x11` centimeters, `0x13` inches.
    pub unit: u32,
    /// HID unit exponent, either signed or as the raw 4-bit nibble.
    pub unit_exponent: i32,
}

impl AxisRange {
    /// Millimeters per logical count, when the device states its real size.
    pub fn mm_per_count(&self) -> Option<f32> {
        let mm_per_unit = match self.unit {
            0x11 => 10.0,
            0x13 => 25.4,
            _ => return None,
        };
        let counts = self.counts()?;
        let physical = self.physical_max.checked_sub(self.physical_min)?;
        if physical <= 0 {
            return None;
        }
        // The exponent is a 4-bit two's complement value: 0xE means -2.
        let exponent = if self.unit_exponent > 7 {
            self.unit_exponent - 16
        } else {
            self.unit_exponent
        };
        Some(physical as f32 * 10f32.powi(exponent) * mm_per_unit / counts)
    }

    fn counts(&self) -> Option<f32> {
        let counts = self.logical_max.checked_sub(self.logical_min)?;
        (counts > 0).then_some(counts as f32)
    }
}

/// Converts device counts to millimeters, so the stick feels the same on
/// every touchpad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PadScale {
    pub x_mm_per_count: f32,
    pub y_mm_per_count: f32,
    /// False when the device did not state its size and a width was assumed.
    pub measured: bool,
}

/// Width assumed for a touchpad that does not state its size.
pub const ASSUMED_PAD_WIDTH_MM: f32 = 100.0;

impl PadScale {
    pub fn new(x: AxisRange, y: AxisRange) -> Self {
        match (x.mm_per_count(), y.mm_per_count()) {
            (Some(x_mm_per_count), Some(y_mm_per_count)) => Self {
                x_mm_per_count,
                y_mm_per_count,
                measured: true,
            },
            _ => {
                let mm_per_count = ASSUMED_PAD_WIDTH_MM / x.counts().unwrap_or(1.0);
                Self {
                    x_mm_per_count: mm_per_count,
                    y_mm_per_count: mm_per_count,
                    measured: false,
                }
            }
        }
    }

    pub fn point(&self, x_counts: u32, y_counts: u32) -> Point {
        Point {
            x: x_counts as f32 * self.x_mm_per_count,
            y: y_counts as f32 * self.y_mm_per_count,
        }
    }
}

/// Puts a frame back together when the pad splits it over several reports.
///
/// Each report has a fixed number of finger slots. The first report of a
/// frame says how many contacts the frame has; when they do not fit, the
/// reports that follow carry the rest and say zero.
#[derive(Debug, Default)]
pub struct FrameAssembler {
    frame: Vec<RawContact>,
    missing: usize,
}

impl FrameAssembler {
    /// Takes one report. Returns the frame once it is complete.
    pub fn report(&mut self, contact_count: u32, slots: &[RawContact]) -> Option<&[RawContact]> {
        if self.missing == 0 {
            self.frame.clear();
            self.missing = contact_count as usize;
        }
        let taken = self.missing.min(slots.len());
        self.frame.extend_from_slice(&slots[..taken]);
        // A report that brings nothing ends the frame, or it would never end.
        self.missing = if taken == 0 { 0 } else { self.missing - taken };
        (self.missing == 0).then_some(&self.frame[..])
    }
}
