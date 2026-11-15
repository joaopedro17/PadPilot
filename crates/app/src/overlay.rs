//! The on-screen joystick window.
//!
//! Always on top, never focused, transparent around the circle. Right-click
//! the joystick to enter joystick mode: the cursor is kept on the joystick
//! and the touchpad drives the virtual controller. Right-click again to leave.

mod failsafe;
mod surface;
mod telemetry;

use std::cell::RefCell;
use std::mem::size_of;
use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use padpilot_core::gamepad::GamepadState;
use padpilot_core::overlay::layout::{DEFAULT_JOYSTICK_SIDE, Layout};
use padpilot_core::overlay::model::{Command, HitKind, Mode, Overlay};
use padpilot_core::overlay::paint::Painter;
use padpilot_core::stick::{Contact, StickSettings};
use tracing::{debug, error, info, trace, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::HRAWINPUT;
use windows::Win32::UI::WindowsAndMessaging::{
    ClipCursor, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowRect, HTCAPTION, HTCLIENT, HTTRANSPARENT, IDC_ARROW, KillTimer, LoadCursorW,
    MA_NOACTIVATE, MSG, PostQuitMessage, RegisterClassExW, SPI_GETWORKAREA, SW_MINIMIZE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetTimer, SetWindowPos, ShowWindow, SystemParametersInfoW,
    USER_DEFAULT_SCREEN_DPI, WM_ACTIVATE, WM_DESTROY, WM_DPICHANGED, WM_INPUT, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOVING, WM_NCHITTEST, WM_RBUTTONDOWN, WM_TIMER, WNDCLASSEXW,
    WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_POPUP,
    WS_SYSMENU,
};
use windows::core::w;

use crate::controller::{self, VirtualController};
use crate::error::Error;
use crate::touchpad;
use failsafe::Failsafe;
use surface::Surface;
use telemetry::{Telemetry, Timing};

/// How long joystick mode may last before it switches itself off. A stopgap
/// until the watchdog process exists.
pub const DEFAULT_JOYSTICK_LIMIT: Duration = Duration::from_secs(60);

/// Windows drops the cursor lock on its own now and then (for example when
/// another window comes to the front), so it is applied again this often.
const CAGE_TIMER: usize = 1;
const CAGE_EVERY_MS: u32 = 100;

/// Distance from the screen corner where the window first appears.
const SCREEN_MARGIN: f32 = 24.0;
const OPACITY: u8 = 255;

/// Painting takes several milliseconds, longer than the gap between touchpad
/// events, so the knob is redrawn only when no input is waiting. While input
/// keeps coming, it is still redrawn at least this often.
const MAX_REPAINT_GAP: Duration = Duration::from_millis(33);

thread_local! {
    /// Everything the window procedure needs. Only touched on the window's
    /// thread, and never held across a call that sends window messages.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn run(
    joystick_limit: Duration,
    settings: StickSettings,
    record_telemetry: bool,
) -> Result<(), Error> {
    // SAFETY: a process-wide setting, made before any window exists. It
    // fails harmlessly when the setting was already made.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    failsafe::release_cursor_on_panic();

    let pad = controller::connect()?;
    let window = create_window()?;
    // SAFETY: `window` was just created by this thread.
    let scale = unsafe { GetDpiForWindow(window) } as f32 / USER_DEFAULT_SCREEN_DPI as f32;
    let layout = Layout::new(DEFAULT_JOYSTICK_SIDE, scale);
    let mut state = State {
        window,
        model: Overlay::new(settings),
        layout,
        surface: Surface::new(layout.width(), layout.height())?,
        painter: Painter::new(layout),
        pad: Box::new(pad),
        reader: touchpad::Reader::attach(window)?,
        failsafe: Failsafe::start(joystick_limit),
        telemetry: None,
        stale_since: None,
    };
    if record_telemetry {
        let telemetry = Telemetry::start(window)?;
        info!(target: "telemetry", path = %telemetry.path().display(), "recording");
        println!("Recording telemetry to {}", telemetry.path().display());
        state.telemetry = Some(telemetry);
    }
    info!(
        target: "overlay",
        scale,
        width = layout.width(),
        height = layout.height(),
        joystick_limit_s = joystick_limit.as_secs(),
        "window created"
    );
    state.repaint();
    STATE.set(Some(state));
    place_in_screen_corner(window, layout);
    // SAFETY: plain calls on a window this thread owns.
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            DispatchMessageW(&message);
        }
    }

    failsafe::release_cursor();
    // Dropping the state unplugs the virtual controller and saves telemetry.
    if let Some(telemetry) = STATE.take().and_then(|state| state.telemetry) {
        info!(target: "telemetry", rows = telemetry.rows(), path = %telemetry.path().display(), "saved");
        println!(
            "Telemetry saved: {} events in {}",
            telemetry.rows(),
            telemetry.path().display()
        );
    }
    info!(target: "overlay", "window closed");
    Ok(())
}

struct State {
    window: HWND,
    model: Overlay,
    layout: Layout,
    surface: Surface,
    painter: Painter,
    pad: Box<dyn VirtualController>,
    reader: touchpad::Reader,
    failsafe: Failsafe,
    telemetry: Option<Telemetry>,
    /// When the knob on screen stopped matching the stick; `None`: it matches.
    stale_since: Option<Instant>,
}

impl State {
    fn hit(&self, screen_x: i32, screen_y: i32) -> HitKind {
        let origin = window_rect(self.window);
        let region = self
            .layout
            .region_at(screen_x - origin.left, screen_y - origin.top);
        self.model.hit(region)
    }

    fn right_click(&mut self, x: i32, y: i32) {
        if let Some(mode) = self.model.right_click(self.layout.region_at(x, y)) {
            self.mode_changed(mode);
        }
    }

    fn left_click(&self, x: i32, y: i32) -> Option<Command> {
        self.model.left_click(self.layout.region_at(x, y))
    }

    fn leave_joystick_mode(&mut self, reason: &str) {
        if self.model.leave_joystick_mode() {
            warn!(target: "overlay", reason, "leaving joystick mode");
            self.mode_changed(Mode::Normal);
        }
    }

    fn mode_changed(&mut self, mode: Mode) {
        match mode {
            Mode::Joystick => {
                info!(target: "overlay", "entered joystick mode");
                self.failsafe.arm();
                self.cage_cursor();
                // SAFETY: a plain call on a window this thread owns.
                unsafe { SetTimer(Some(self.window), CAGE_TIMER, CAGE_EVERY_MS, None) };
            }
            Mode::Normal => {
                self.failsafe.disarm();
                // SAFETY: as above. The timer may already be gone.
                let _ = unsafe { KillTimer(Some(self.window), CAGE_TIMER) };
                failsafe::release_cursor();
                info!(target: "overlay", "left joystick mode, cursor lock released");
            }
        }
        // Entering and leaving both start from a released controller.
        if let Err(err) = self.pad.send(GamepadState::default()) {
            error!(target: "controller", "{err}");
        }
        self.repaint();
    }

    /// Keeps the cursor on the joystick, so every click lands on it.
    fn cage_cursor(&self) {
        let origin = window_rect(self.window);
        let cage = self.layout.cursor_cage();
        let rect = RECT {
            left: origin.left + cage.left,
            top: origin.top + cage.top,
            right: origin.left + cage.right,
            bottom: origin.top + cage.bottom,
        };
        // SAFETY: `rect` outlives the call.
        match unsafe { ClipCursor(Some(&rect)) } {
            Ok(()) => trace!(target: "overlay", ?rect, "cursor lock applied"),
            Err(err) => warn!(target: "overlay", "cannot lock the cursor: {err:?}"),
        }
    }

    fn cage_tick(&mut self) {
        if self.failsafe.allows_lock() {
            self.cage_cursor();
        } else {
            self.leave_joystick_mode("time limit reached");
        }
    }

    /// The controller is updated at once for every event; the knob is
    /// redrawn once the waiting input is handled.
    fn raw_input(&mut self, input: HRAWINPUT) {
        let mouse = self
            .telemetry
            .as_ref()
            .and_then(|_| touchpad::mouse_motion(input));
        let (contact, mut timing) = match mouse {
            Some(_) => (None, Timing::default()),
            None => self.touchpad_input(input),
        };
        if let Some(stale_since) = self.stale_since
            && (!touchpad::input_waiting() || stale_since.elapsed() >= MAX_REPAINT_GAP)
        {
            let (paint, present) = self.repaint();
            timing.paint = Some(paint);
            timing.present = Some(present);
        }
        if let Some(telemetry) = &mut self.telemetry {
            let (mode, knob) = (self.model.mode(), self.model.knob());
            match mouse {
                Some(motion) => telemetry.mouse(motion, mode, knob, &timing),
                None => telemetry.touch(contact, mode, knob, &timing),
            }
        }
    }

    /// Returns the last finger contact in the event and how long each stage took.
    fn touchpad_input(&mut self, input: HRAWINPUT) -> (Option<Contact>, Timing) {
        let started = Instant::now();
        let mut latest = None;
        let mut last_contact = None;
        let model = &mut self.model;
        let _ = self.reader.input(input, &mut |contact| {
            last_contact = Some(contact);
            if let Some(state) = model.contact(contact) {
                trace!(target: "stick", ?contact, ?state, "update");
                latest = Some(state);
            }
            ControlFlow::Continue(())
        });
        let mut timing = Timing {
            read: started.elapsed(),
            ..Timing::default()
        };
        let Some(state) = latest else {
            return (last_contact, timing);
        };
        let sending = Instant::now();
        if let Err(err) = self.pad.send(state) {
            error!(target: "controller", "{err}");
            self.leave_joystick_mode("the virtual controller failed");
            return (last_contact, timing);
        }
        timing.send = Some(sending.elapsed());
        self.stale_since.get_or_insert_with(Instant::now);
        (last_contact, timing)
    }

    fn rescale(&mut self, scale: f32) {
        let layout = Layout::new(DEFAULT_JOYSTICK_SIDE, scale);
        match Surface::new(layout.width(), layout.height()) {
            Ok(surface) => {
                info!(target: "overlay", scale, "display scaling changed");
                self.layout = layout;
                self.surface = surface;
                self.painter = Painter::new(layout);
                self.repaint();
            }
            Err(err) => error!(target: "overlay", "{err}"),
        }
    }

    /// Returns how long painting and showing took.
    fn repaint(&mut self) -> (Duration, Duration) {
        let started = Instant::now();
        self.stale_since = None;
        let (mode, knob) = (self.model.mode(), self.model.knob());
        self.painter.paint(&mut self.surface.canvas(), mode, knob);
        let painted = Instant::now();
        if let Err(err) = self.surface.present(self.window, OPACITY) {
            debug!(target: "overlay", "{err}");
        }
        (painted - started, painted.elapsed())
    }
}

/// Runs `action` on the window's state. `None` while the state is in use
/// further up the call stack (a message sent from inside a handler) or gone.
fn with_state<T>(action: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE.with(|state| state.try_borrow_mut().ok()?.as_mut().map(action))
}

extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match handle(window, message, wparam, lparam) {
        Some(result) => LRESULT(result),
        // SAFETY: forwards the message exactly as it was received.
        None => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// Handles one message. `None` leaves it to Windows' default handling.
fn handle(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
    let (x, y) = point(lparam);
    match message {
        // Clicking the window must never take focus from the game.
        WM_MOUSEACTIVATE => Some(MA_NOACTIVATE as isize),
        WM_NCHITTEST => with_state(|state| match state.hit(x, y) {
            HitKind::Caption => HTCAPTION as isize,
            HitKind::Client => HTCLIENT as isize,
            HitKind::PassThrough => HTTRANSPARENT as isize,
        }),
        WM_RBUTTONDOWN => with_state(|state| state.right_click(x, y)).map(|()| 0),
        WM_LBUTTONUP => {
            // Minimizing and closing send messages back to this window, so
            // they run after the state is released.
            match with_state(|state| state.left_click(x, y))?? {
                Command::Minimize => {
                    info!(target: "overlay", "minimized");
                    // SAFETY: a plain call on this thread's window.
                    let _ = unsafe { ShowWindow(window, SW_MINIMIZE) };
                }
                Command::Close => {
                    info!(target: "overlay", "close button pressed");
                    // SAFETY: as above.
                    let _ = unsafe { DestroyWindow(window) };
                }
            }
            Some(0)
        }
        WM_INPUT => {
            with_state(|state| state.raw_input(HRAWINPUT(lparam.0 as _)));
            // The default handling releases the raw input data.
            None
        }
        WM_TIMER if wparam.0 == CAGE_TIMER => with_state(State::cage_tick).map(|()| 0),
        WM_MOVING => {
            // A window that is never activated does not follow the cursor
            // while dragged unless it is moved by hand.
            // SAFETY: for WM_MOVING, lparam points at the proposed position.
            let to = unsafe { *(lparam.0 as *const RECT) };
            move_window(window, to.left, to.top);
            None
        }
        WM_DPICHANGED => {
            let scale = (wparam.0 & 0xFFFF) as f32 / USER_DEFAULT_SCREEN_DPI as f32;
            // SAFETY: for WM_DPICHANGED, lparam points at the suggested position.
            let to = unsafe { *(lparam.0 as *const RECT) };
            with_state(|state| state.rescale(scale));
            move_window(window, to.left, to.top);
            Some(0)
        }
        WM_ACTIVATE if wparam.0 & 0xFFFF != 0 => {
            warn!(target: "overlay", "the window took focus");
            None
        }
        WM_DESTROY => {
            with_state(|state| state.leave_joystick_mode("the window is closing"));
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            Some(0)
        }
        _ => None,
    }
}

/// The signed x and y packed into a mouse message's lparam.
fn point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as u16 as i16;
    let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16;
    (i32::from(x), i32::from(y))
}

fn window_rect(window: HWND) -> RECT {
    let mut rect = RECT::default();
    // SAFETY: `rect` outlives the call. On failure it stays at the origin.
    let _ = unsafe { GetWindowRect(window, &mut rect) };
    rect
}

fn move_window(window: HWND, x: i32, y: i32) {
    let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
    // SAFETY: a plain call on this thread's window. Nothing to do on failure.
    let _ = unsafe { SetWindowPos(window, None, x, y, 0, 0, flags) };
}

fn place_in_screen_corner(window: HWND, layout: Layout) {
    let mut area = RECT::default();
    // SAFETY: SPI_GETWORKAREA writes one RECT into `area`.
    let found = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&raw mut area).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if found.is_err() {
        return;
    }
    let margin = (SCREEN_MARGIN * layout.scale()) as i32;
    move_window(
        window,
        area.right - layout.width() - margin,
        area.bottom - layout.height() - margin,
    );
}

fn create_window() -> Result<HWND, Error> {
    let failed = |err| Error::caused("cannot create the window", err);
    // SAFETY: registers a class whose procedure lives for the whole program,
    // then creates one window of it.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(failed)?.into();
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW).map_err(failed)?,
            lpszClassName: w!("PadPilotOverlay"),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(Error::new("cannot register the window class"));
        }
        CreateWindowExW(
            // Above the game, never focused, per-pixel transparent, and still
            // listed in the taskbar.
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_APPWINDOW,
            class.lpszClassName,
            w!("PadPilot"),
            WS_POPUP | WS_SYSMENU | WS_MINIMIZEBOX,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
        .map_err(failed)
    }
}
