//! Decodes Precision Touchpad HID reports with the Windows HID parser.

use std::collections::BTreeMap;

use padpilot_core::touchpad::{AxisRange, PadScale, RawContact};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS, HidP_GetCaps, HidP_GetUsageValue,
    HidP_GetUsages, HidP_GetValueCaps, HidP_Input, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Input::{GetRawInputDeviceInfoW, RIDI_PREPARSEDDATA};

use crate::error::Error;

pub const PAGE_DIGITIZER: u16 = 0x0D;
pub const USAGE_TOUCHPAD: u16 = 0x05;

const PAGE_GENERIC_DESKTOP: u16 = 0x01;
const USAGE_X: u16 = 0x30;
const USAGE_Y: u16 = 0x31;
const USAGE_TIP_SWITCH: u16 = 0x42;
const USAGE_CONTACT_ID: u16 = 0x51;
const USAGE_CONTACT_COUNT: u16 = 0x54;

/// Where one touchpad keeps each piece of a report.
pub struct Layout {
    /// The device's parsed report descriptor, owned here and only read by the
    /// HID parser. `u64` keeps it aligned.
    preparsed: Vec<u64>,
    /// Link collection of every finger slot in a report.
    slots: Vec<u16>,
    contact_count: Option<u16>,
    scale: PadScale,
}

#[derive(Default)]
struct SlotCaps {
    x: Option<AxisRange>,
    y: Option<AxisRange>,
}

impl Layout {
    pub fn read(device: HANDLE) -> Result<Self, Error> {
        let preparsed = read_preparsed(device)?;
        let handle = PHIDP_PREPARSED_DATA(preparsed.as_ptr() as isize);

        let mut caps = HIDP_CAPS::default();
        // SAFETY: `handle` points at preparsed data that outlives the call.
        if unsafe { HidP_GetCaps(handle, &mut caps) } != HIDP_STATUS_SUCCESS {
            return Err(Error::new("cannot read the touchpad's capabilities"));
        }
        let mut count = caps.NumberInputValueCaps;
        let mut values = vec![HIDP_VALUE_CAPS::default(); usize::from(count)];
        // SAFETY: `values` has room for `count` entries, as the call requires.
        let status =
            unsafe { HidP_GetValueCaps(HidP_Input, values.as_mut_ptr(), &mut count, handle) };
        if status != HIDP_STATUS_SUCCESS {
            return Err(Error::new("cannot read the touchpad's value descriptions"));
        }
        values.truncate(usize::from(count));

        let mut by_link = BTreeMap::<u16, SlotCaps>::new();
        let mut contact_count = None;
        for value in &values {
            let link = value.LinkCollection;
            match value.UsagePage {
                PAGE_GENERIC_DESKTOP if has_usage(value, USAGE_X) => {
                    by_link.entry(link).or_default().x = Some(axis_range(value));
                }
                PAGE_GENERIC_DESKTOP if has_usage(value, USAGE_Y) => {
                    by_link.entry(link).or_default().y = Some(axis_range(value));
                }
                PAGE_DIGITIZER if has_usage(value, USAGE_CONTACT_COUNT) => {
                    contact_count = Some(link);
                }
                _ => {}
            }
        }
        let fingers: Vec<(u16, AxisRange, AxisRange)> = by_link
            .into_iter()
            .filter_map(|(link, caps)| Some((link, caps.x?, caps.y?)))
            .collect();
        let &(_, x, y) = fingers
            .first()
            .ok_or_else(|| Error::new("the touchpad reports no finger positions"))?;

        Ok(Self {
            preparsed,
            slots: fingers.iter().map(|&(link, ..)| link).collect(),
            contact_count,
            scale: PadScale::new(x, y),
        })
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn scale(&self) -> PadScale {
        self.scale
    }

    /// Reads every finger slot of one report into `slots` and returns the
    /// report's contact count. `None` when this is not a touch report.
    pub fn parse(&self, report: &mut [u8], slots: &mut Vec<RawContact>) -> Option<u32> {
        slots.clear();
        for (index, &link) in self.slots.iter().enumerate() {
            let x = self.value(report, PAGE_GENERIC_DESKTOP, link, USAGE_X)?;
            let y = self.value(report, PAGE_GENERIC_DESKTOP, link, USAGE_Y)?;
            let id = self
                .value(report, PAGE_DIGITIZER, link, USAGE_CONTACT_ID)
                .unwrap_or(index as u32);
            slots.push(RawContact {
                id,
                tip: self.tip_switch(report, link),
                position: self.scale.point(x, y),
            });
        }
        let stated = self
            .contact_count
            .and_then(|link| self.value(report, PAGE_DIGITIZER, link, USAGE_CONTACT_COUNT));
        Some(stated.unwrap_or(slots.len() as u32))
    }

    fn handle(&self) -> PHIDP_PREPARSED_DATA {
        PHIDP_PREPARSED_DATA(self.preparsed.as_ptr() as isize)
    }

    fn value(&self, report: &[u8], page: u16, link: u16, usage: u16) -> Option<u32> {
        let mut value = 0;
        // SAFETY: the preparsed data belongs to `self`; the parser checks the
        // report against it and only reads within the slice.
        let status = unsafe {
            HidP_GetUsageValue(
                HidP_Input,
                page,
                Some(link),
                usage,
                &mut value,
                self.handle(),
                report,
            )
        };
        (status == HIDP_STATUS_SUCCESS).then_some(value)
    }

    fn tip_switch(&self, report: &mut [u8], link: u16) -> bool {
        let mut pressed = [0u16; 8];
        let mut count = pressed.len() as u32;
        // SAFETY: as in `value`; `pressed` has room for `count` usages.
        let status = unsafe {
            HidP_GetUsages(
                HidP_Input,
                PAGE_DIGITIZER,
                Some(link),
                pressed.as_mut_ptr(),
                &mut count,
                self.handle(),
                report,
            )
        };
        status == HIDP_STATUS_SUCCESS && pressed[..count as usize].contains(&USAGE_TIP_SWITCH)
    }
}

fn read_preparsed(device: HANDLE) -> Result<Vec<u64>, Error> {
    let failed = || Error::new("cannot read the touchpad's report descriptor");
    let mut bytes = 0u32;
    // SAFETY: with no buffer the call only writes the needed size.
    unsafe { GetRawInputDeviceInfoW(Some(device), RIDI_PREPARSEDDATA, None, &mut bytes) };
    if bytes == 0 {
        return Err(failed());
    }
    let mut preparsed = vec![0u64; (bytes as usize).div_ceil(8)];
    // SAFETY: the buffer holds at least `bytes` bytes.
    let written = unsafe {
        GetRawInputDeviceInfoW(
            Some(device),
            RIDI_PREPARSEDDATA,
            Some(preparsed.as_mut_ptr().cast()),
            &mut bytes,
        )
    };
    if written == 0 || written == u32::MAX {
        return Err(failed());
    }
    Ok(preparsed)
}

fn has_usage(value: &HIDP_VALUE_CAPS, usage: u16) -> bool {
    // SAFETY: `IsRange` says which member of the union the parser filled in.
    unsafe {
        if value.IsRange {
            let range = value.Anonymous.Range;
            (range.UsageMin..=range.UsageMax).contains(&usage)
        } else {
            value.Anonymous.NotRange.Usage == usage
        }
    }
}

fn axis_range(value: &HIDP_VALUE_CAPS) -> AxisRange {
    AxisRange {
        logical_min: value.LogicalMin,
        logical_max: value.LogicalMax,
        physical_min: value.PhysicalMin,
        physical_max: value.PhysicalMax,
        unit: value.Units,
        unit_exponent: value.UnitsExp as i32,
    }
}
