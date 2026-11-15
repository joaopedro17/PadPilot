//! Per-event timing of the overlay, for finding input delay. Off unless
//! started with `padpilot telemetry`. One CSV row per input event:
//!
//! - `t_ms`: time since recording started.
//! - `queue_ms`: how long the event waited before it was handled
//!   (Windows' message clock, about 1 ms steps but often 16 ms).
//! - `backlog`: 1 when more input was already waiting behind this event.
//! - `touch` rows: finger position (mm) or `lifted`, the knob it gave, and
//!   the time spent reading, updating the controller, painting and showing.
//! - `mouse` rows: the movement Windows made from the same finger, for
//!   comparison, and where the cursor is.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use padpilot_core::overlay::model::Mode;
use padpilot_core::stick::{Contact, Deflection};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetTickCount};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetMessageTime};

use crate::error::Error;
use crate::touchpad;

const HEADER: &str = "t_ms,kind,mode,queue_ms,backlog,finger_x_mm,finger_y_mm,knob_x,knob_y,\
mouse_dx,mouse_dy,cursor_x,cursor_y,read_ms,send_ms,paint_ms,present_ms";

/// How long each stage of handling one touchpad event took.
#[derive(Default)]
pub struct Timing {
    pub read: Duration,
    pub send: Option<Duration>,
    pub paint: Option<Duration>,
    pub present: Option<Duration>,
}

pub struct Telemetry {
    out: BufWriter<File>,
    path: PathBuf,
    start: Instant,
    rows: u64,
}

impl Telemetry {
    pub fn start(window: HWND) -> Result<Self, Error> {
        touchpad::listen_to_mouse(window)?;
        // SAFETY: a plain call with no arguments.
        let now = unsafe { GetLocalTime() };
        let name = format!(
            "telemetry-{:04}{:02}{:02}-{:02}{:02}{:02}.csv",
            now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
        );
        let path = padpilot_logging::logs_dir().join(name);
        let file = File::create(&path)
            .map_err(|err| Error::caused("cannot create the telemetry file", err))?;
        let mut out = BufWriter::with_capacity(1 << 16, file);
        let _ = writeln!(out, "{HEADER}");
        Ok(Self {
            out,
            path,
            start: Instant::now(),
            rows: 0,
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn rows(&self) -> u64 {
        self.rows
    }

    pub fn touch(&mut self, contact: Option<Contact>, mode: Mode, knob: Deflection, t: &Timing) {
        let finger = match contact {
            Some(Contact::Touching(p)) => format!("{:.2},{:.2}", p.x, p.y),
            Some(Contact::Lifted) => "lifted,lifted".to_owned(),
            None => ",".to_owned(),
        };
        let rest = format!(
            "{finger},{:.3},{:.3},,,{},{},{},{},{}",
            knob.x(),
            knob.y(),
            self.cursor(),
            ms(t.read),
            opt_ms(t.send),
            opt_ms(t.paint),
            opt_ms(t.present),
        );
        self.row("touch", mode, &rest);
    }

    pub fn mouse(&mut self, (dx, dy): (i32, i32), mode: Mode, knob: Deflection, t: &Timing) {
        let rest = format!(
            ",,{:.3},{:.3},{dx},{dy},{},,,{},{}",
            knob.x(),
            knob.y(),
            self.cursor(),
            opt_ms(t.paint),
            opt_ms(t.present),
        );
        self.row("mouse", mode, &rest);
    }

    fn row(&mut self, kind: &str, mode: Mode, rest: &str) {
        let t = ms(self.start.elapsed());
        // SAFETY: plain calls; valid while handling a message from GetMessage.
        let (queue, backlog) = unsafe {
            let queue = GetTickCount().wrapping_sub(GetMessageTime() as u32);
            (queue, touchpad::input_waiting())
        };
        let mode = match mode {
            Mode::Normal => "normal",
            Mode::Joystick => "joystick",
        };
        let _ = writeln!(
            self.out,
            "{t},{kind},{mode},{queue},{},{rest}",
            u8::from(backlog)
        );
        self.rows += 1;
    }

    fn cursor(&self) -> String {
        let mut at = POINT::default();
        // SAFETY: `at` outlives the call.
        match unsafe { GetCursorPos(&mut at) } {
            Ok(()) => format!("{},{}", at.x, at.y),
            Err(_) => ",".to_owned(),
        }
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        let _ = self.out.flush();
    }
}

fn ms(d: Duration) -> String {
    format!("{:.3}", d.as_secs_f64() * 1000.0)
}

fn opt_ms(d: Option<Duration>) -> String {
    d.map(ms).unwrap_or_default()
}
