//! Receives touchpad reports through Raw Input on a hidden window.

use std::collections::HashMap;
use std::mem::size_of;
use std::ops::ControlFlow;
use std::time::Duration;

use padpilot_core::stick::Contact;
use padpilot_core::touchpad::{FirstContact, FrameAssembler, RawContact};
use tracing::{info, warn};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList, HRAWINPUT, RAWINPUT,
    RAWINPUTDEVICE, RAWINPUTDEVICELIST, RAWINPUTHEADER, RID_DEVICE_INFO, RID_INPUT,
    RIDEV_INPUTSINK, RIDI_DEVICEINFO, RIDI_DEVICENAME, RIM_TYPEHID, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG, SetTimer,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_INPUT, WM_TIMER,
};
use windows::core::w;

use super::hid::{Layout, PAGE_DIGITIZER, USAGE_TOUCHPAD};
use crate::error::Error;

const STOP_TIMER: usize = 1;

/// Calls `on_contact` for the first finger on the touchpad, from this thread,
/// until it asks to stop or `duration` has passed (forever when `None`).
pub fn listen(
    duration: Option<Duration>,
    mut on_contact: impl FnMut(Contact) -> ControlFlow<()>,
) -> Result<(), Error> {
    if log_touchpads() == 0 {
        warn!(target: "reader", "no Precision Touchpad found");
        println!("No Precision Touchpad found.");
    }
    let window = Window::create()?;
    let registration = RAWINPUTDEVICE {
        usUsagePage: PAGE_DIGITIZER,
        usUsage: USAGE_TOUCHPAD,
        // Deliver reports while another window has focus.
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: window.0,
    };
    // SAFETY: plain calls on a window this thread owns.
    unsafe {
        RegisterRawInputDevices(&[registration], size_of::<RAWINPUTDEVICE>() as u32)
            .map_err(|err| Error::caused("cannot register for touchpad input", err))?;
        if let Some(duration) = duration {
            SetTimer(
                Some(window.0),
                STOP_TIMER,
                duration.as_millis() as u32,
                None,
            );
        }
    }
    info!(target: "reader", "listening for touchpad input");

    let mut reader = Reader::default();
    let mut message = MSG::default();
    // SAFETY: a standard message loop; `message` is valid for every call.
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        match message.message {
            WM_INPUT => {
                let input = HRAWINPUT(message.lParam.0 as _);
                if reader.input(input, &mut on_contact).is_break() {
                    break;
                }
            }
            WM_TIMER if message.wParam.0 == STOP_TIMER => break,
            _ => {}
        }
        // The default window procedure releases the raw input data.
        unsafe { DispatchMessageW(&message) };
    }
    info!(target: "reader", "stopped listening for touchpad input");
    Ok(())
}

/// A message-only window: never visible, never focused.
struct Window(HWND);

impl Window {
    fn create() -> Result<Self, Error> {
        // SAFETY: creates a window of a built-in class with no parameters.
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("PadPilot touchpad reader"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        };
        window
            .map(Self)
            .map_err(|err| Error::caused("cannot create the touchpad reader window", err))
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the window belongs to this thread. Nothing to do on failure.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

struct Device {
    layout: Layout,
    assembler: FrameAssembler,
    first: FirstContact,
}

#[derive(Default)]
struct Reader {
    /// Known devices by handle. `None`: its reports cannot be decoded.
    devices: HashMap<usize, Option<Device>>,
    /// Reused for every event, so the input path does not allocate.
    event: Vec<u64>,
    slots: Vec<RawContact>,
}

impl Reader {
    fn input(
        &mut self,
        input: HRAWINPUT,
        on_contact: &mut impl FnMut(Contact) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        let Some((device, reports, report_size)) = read_event(input, &mut self.event) else {
            return ControlFlow::Continue(());
        };
        let known = self.devices.entry(device.0 as usize).or_insert_with(|| {
            Layout::read(device)
                .inspect_err(|err| warn!(target: "reader", "ignoring a touchpad: {err}"))
                .ok()
                .map(|layout| Device {
                    layout,
                    assembler: FrameAssembler::default(),
                    first: FirstContact::default(),
                })
        });
        let Some(known) = known else {
            return ControlFlow::Continue(());
        };
        for report in reports.chunks_exact_mut(report_size) {
            let Some(contact_count) = known.layout.parse(report, &mut self.slots) else {
                continue;
            };
            let Some(frame) = known.assembler.report(contact_count, &self.slots) else {
                continue;
            };
            if let Some(contact) = known.first.frame(frame) {
                on_contact(contact)?;
            }
        }
        ControlFlow::Continue(())
    }
}

/// Copies one raw input event into `buffer` and returns its device, its HID
/// reports back to back, and the size of one report.
fn read_event(input: HRAWINPUT, buffer: &mut Vec<u64>) -> Option<(HANDLE, &mut [u8], usize)> {
    let header_size = size_of::<RAWINPUTHEADER>() as u32;
    let mut bytes = 0u32;
    // SAFETY: with no buffer the call only writes the needed size.
    unsafe { GetRawInputData(input, RID_INPUT, None, &mut bytes, header_size) };
    let total = bytes as usize;
    // A HID event is a header, two sizes, then the reports.
    if total < size_of::<RAWINPUTHEADER>() + 2 * size_of::<u32>() {
        return None;
    }
    buffer.resize(total.div_ceil(8), 0);
    // SAFETY: the buffer holds at least `bytes` bytes.
    let read = unsafe {
        GetRawInputData(
            input,
            RID_INPUT,
            Some(buffer.as_mut_ptr().cast()),
            &mut bytes,
            header_size,
        )
    };
    if read != bytes {
        return None;
    }
    let event = buffer.as_mut_ptr().cast::<RAWINPUT>();
    // SAFETY: the buffer is aligned for RAWINPUT and Windows filled it with
    // one. The reports start at `bRawData`; their length is checked against
    // the bytes actually read before the slice is made.
    unsafe {
        let header = (*event).header;
        if header.dwType != RIM_TYPEHID.0 {
            return None;
        }
        let report_size = (*event).data.hid.dwSizeHid as usize;
        let length = report_size.checked_mul((*event).data.hid.dwCount as usize)?;
        let reports = (&raw mut (*event).data.hid.bRawData).cast::<u8>();
        let offset = reports as usize - event as usize;
        if report_size == 0 || offset + length > total {
            return None;
        }
        Some((
            header.hDevice,
            std::slice::from_raw_parts_mut(reports, length),
            report_size,
        ))
    }
}

/// Logs every Precision Touchpad connected right now. Returns how many.
fn log_touchpads() -> usize {
    let entry_size = size_of::<RAWINPUTDEVICELIST>() as u32;
    let mut count = 0u32;
    // SAFETY: with no buffer the call only writes the number of devices.
    unsafe { GetRawInputDeviceList(None, &mut count, entry_size) };
    let mut devices = vec![RAWINPUTDEVICELIST::default(); count as usize];
    // SAFETY: `devices` has room for `count` entries.
    let listed =
        unsafe { GetRawInputDeviceList(Some(devices.as_mut_ptr()), &mut count, entry_size) };
    if listed == u32::MAX {
        warn!(target: "reader", "cannot list input devices");
        return 0;
    }
    devices.truncate(listed as usize);

    let touchpads = devices.iter().filter(|device| is_touchpad(device));
    touchpads
        .inspect(|device| match Layout::read(device.hDevice) {
            Ok(layout) => {
                let scale = layout.scale();
                info!(
                    target: "reader",
                    name = device_name(device.hDevice),
                    finger_slots = layout.slot_count(),
                    x_mm_per_count = scale.x_mm_per_count,
                    y_mm_per_count = scale.y_mm_per_count,
                    size_measured = scale.measured,
                    "Precision Touchpad detected"
                );
            }
            Err(err) => {
                warn!(target: "reader", name = device_name(device.hDevice), "unusable touchpad: {err}");
            }
        })
        .count()
}

fn is_touchpad(device: &RAWINPUTDEVICELIST) -> bool {
    if device.dwType != RIM_TYPEHID {
        return false;
    }
    let mut info = RID_DEVICE_INFO {
        cbSize: size_of::<RID_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let mut bytes = info.cbSize;
    // SAFETY: `info` is as large as `bytes` says.
    let written = unsafe {
        GetRawInputDeviceInfoW(
            Some(device.hDevice),
            RIDI_DEVICEINFO,
            Some((&raw mut info).cast()),
            &mut bytes,
        )
    };
    if written == 0 || written == u32::MAX {
        return false;
    }
    // SAFETY: Windows filled in the HID member for a HID device.
    let hid = unsafe { info.Anonymous.hid };
    hid.usUsagePage == PAGE_DIGITIZER && hid.usUsage == USAGE_TOUCHPAD
}

fn device_name(device: HANDLE) -> String {
    let mut length = 0u32;
    // SAFETY: with no buffer the call only writes the length in characters.
    unsafe { GetRawInputDeviceInfoW(Some(device), RIDI_DEVICENAME, None, &mut length) };
    let mut name = vec![0u16; length as usize];
    // SAFETY: `name` has room for `length` characters.
    let written = unsafe {
        GetRawInputDeviceInfoW(
            Some(device),
            RIDI_DEVICENAME,
            Some(name.as_mut_ptr().cast()),
            &mut length,
        )
    };
    if written == 0 || written == u32::MAX {
        return "unknown".to_owned();
    }
    String::from_utf16_lossy(&name)
        .trim_end_matches('\0')
        .to_owned()
}
