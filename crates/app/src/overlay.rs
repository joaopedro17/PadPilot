//! The on-screen joystick window.
//!
//! Always on top, never focused, transparent around the circle. Right-click
//! the joystick to enter joystick mode: the cursor is kept on the joystick
//! and the touchpad drives the virtual controller. Right-click again to leave.
//! In normal mode the window moves by its title bar and resizes by the
//! handle in its corner or by scrolling over the joystick; its place, size
//! and opacity are saved.

mod failsafe;
mod focus;
mod surface;
mod telemetry;
mod watchdog;

use std::cell::RefCell;
use std::mem::size_of;
use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use padpilot_core::gamepad::GamepadState;
use padpilot_core::overlay::layout::{Layout, Region};
use padpilot_core::overlay::model::{Command, HitKind, Mode, Overlay};
use padpilot_core::overlay::paint::Painter;
use padpilot_core::stick::{Contact, StickSettings};
use padpilot_core::watchdog::{HEARTBEAT_EVERY, Message};
use padpilot_core::window_state::{Opacity, WindowState};
use tracing::{debug, error, info, trace, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITORINFO, MonitorFromRect,
    MonitorFromWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::HWINEVENTHOOK;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::HRAWINPUT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{
    ClipCursor, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetMessageW, GetSystemMetrics, GetWindowRect, HTCAPTION, HTCLIENT, HTTRANSPARENT, IDC_ARROW,
    IsIconic, KillTimer, LoadCursorW, MA_NOACTIVATE, MSG, PostQuitMessage, RegisterClassExW,
    SM_SWAPBUTTON, SPI_GETWORKAREA, SW_MINIMIZE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetTimer, SetWindowPos, ShowWindow,
    SystemParametersInfoW, USER_DEFAULT_SCREEN_DPI, WM_ACTIVATE, WM_DESTROY, WM_DPICHANGED,
    WM_EXITSIZEMOVE, WM_INPUT, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEWHEEL,
    WM_MOVING, WM_NCHITTEST, WM_RBUTTONDOWN, WM_TIMER, WNDCLASSEXW, WS_EX_APPWINDOW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
};
use windows::core::w;

use crate::controller::{self, VirtualController};
use crate::error::Error;
use crate::{config, touchpad, window_state};
use failsafe::Failsafe;
use focus::FocusHook;
use surface::Surface;
use telemetry::{Telemetry, Timing};
use watchdog::Watchdog;

/// How long joystick mode may last while the watchdog is not running.
const UNWATCHED_JOYSTICK_LIMIT: Duration = Duration::from_secs(60);
/// A watchdog that keeps dying is not started again forever.
const MAX_WATCHDOG_RESTARTS: u32 = 3;

/// How the overlay runs. The default is normal use.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Joystick mode switches itself off after this long. `None`: only
    /// without the watchdog, after [`UNWATCHED_JOYSTICK_LIMIT`].
    pub joystick_limit: Option<Duration>,
    /// Record per-event timing to a CSV file.
    pub telemetry: bool,
    /// For testing the watchdog: the window freezes after joystick mode has
    /// been on this long.
    pub freeze_after: Option<Duration>,
}

/// Windows drops the cursor lock on its own now and then (for example when
/// another window comes to the front), so it is applied again this often.
const CAGE_TIMER: usize = 1;
const HEARTBEAT_TIMER: usize = 2;
const CAGE_EVERY_MS: u32 = 100;
/// While the resize handle is held, the cursor is followed this often. A
/// window that never takes focus cannot capture the mouse, so it is polled.
const RESIZE_TIMER: usize = 3;
const RESIZE_EVERY_MS: u32 = 16;
/// The window state is saved once it has stayed unchanged this long.
const SAVE_AFTER: Duration = Duration::from_secs(1);
/// One notch of a mouse wheel; touchpads scroll in smaller steps.
const WHEEL_NOTCH: f32 = 120.0;

/// Distance from the screen corner where the window first appears.
const SCREEN_MARGIN: f32 = 24.0;

/// Painting takes several milliseconds, longer than the gap between touchpad
/// events, so the knob is redrawn only when no input is waiting. While input
/// keeps coming, it is still redrawn at least this often.
const MAX_REPAINT_GAP: Duration = Duration::from_millis(33);

thread_local! {
    /// Everything the window procedure needs. Only touched on the window's
    /// thread, and never held across a call that sends window messages.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub fn run(options: Options, settings: StickSettings) -> Result<(), Error> {
    // SAFETY: a process-wide setting, made before any window exists. It
    // fails harmlessly when the setting was already made.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    failsafe::release_cursor_on_panic();

    let watchdog = start_watchdog();
    let pad = controller::connect()?;
    let saved = window_state::load();
    let window = create_window()?;
    // SAFETY: `window` was just created by this thread.
    let scale = unsafe { GetDpiForWindow(window) } as f32 / USER_DEFAULT_SCREEN_DPI as f32;
    let layout = Layout::new(saved.joystick_side, scale);
    let mut state = State {
        window,
        model: Overlay::new(settings),
        layout,
        surface: Surface::new(layout.width(), layout.height())?,
        painter: Painter::new(layout),
        pad: Box::new(pad),
        reader: touchpad::Reader::attach(window)?,
        failsafe: Failsafe::start(),
        telemetry: None,
        stale_since: None,
        watchdog,
        watchdog_restarts: 0,
        options,
        joystick_since: None,
        game: None,
        opacity: saved.opacity,
        position: saved.position,
        resizing: None,
        unsaved_since: None,
        config: config::Watch::new(),
        settings_window: None,
    };
    if options.telemetry {
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
        ?options,
        "window created"
    );
    state.repaint();
    state.heartbeat();
    STATE.set(Some(state));
    let _focus_hook = FocusHook::install(Some(focus_changed));
    // SAFETY: a plain call on a window this thread owns.
    unsafe {
        SetTimer(
            Some(window),
            HEARTBEAT_TIMER,
            HEARTBEAT_EVERY.as_millis() as u32,
            None,
        )
    };
    match saved.position.filter(|&at| on_a_screen(at, layout)) {
        Some((x, y)) => move_window(window, x, y),
        None => place_in_screen_corner(window, layout),
    }
    // SAFETY: plain calls on a window this thread owns.
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            DispatchMessageW(&message);
        }
    }

    failsafe::release_cursor();
    let Some(mut state) = STATE.take() else {
        return Ok(());
    };
    if let Some(watchdog) = &mut state.watchdog {
        // The watchdog now expects the pipe to close.
        let _ = watchdog.tell(Message::Goodbye);
    }
    // Dropping the state unplugs the virtual controller and saves telemetry.
    if let Some(telemetry) = state.telemetry.take() {
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
    /// `None` when it could not be started or went away.
    watchdog: Option<Watchdog>,
    watchdog_restarts: u32,
    options: Options,
    joystick_since: Option<Instant>,
    /// The window in front when joystick mode started: the game.
    game: Option<HWND>,
    /// When the knob on screen stopped matching the stick; `None`: it matches.
    stale_since: Option<Instant>,
    opacity: Opacity,
    /// The last place the window was seen while not minimized.
    position: Option<(i32, i32)>,
    /// While the resize handle is held: where the window's bottom-right
    /// corner is, relative to the cursor.
    resizing: Option<(i32, i32)>,
    /// When the window was last moved, resized or made more see-through, if
    /// that is not saved yet.
    unsaved_since: Option<Instant>,
    config: config::Watch,
    /// The settings window, while it is open.
    settings_window: Option<std::process::Child>,
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

    fn left_click(&mut self, x: i32, y: i32) -> Option<Command> {
        if self.resizing.is_some() {
            self.end_resize();
            return None;
        }
        let command = self.model.left_click(self.layout.region_at(x, y));
        if command == Some(Command::Settings) {
            self.open_settings();
            return None;
        }
        if command == Some(Command::Opacity) {
            self.opacity = self.opacity.next();
            info!(target: "overlay", opacity = self.opacity.value(), "opacity changed");
            self.changed();
            self.repaint();
            return None;
        }
        command
    }

    /// Opens the settings window, unless it is open already.
    fn open_settings(&mut self) {
        if let Some(child) = &mut self.settings_window
            && matches!(child.try_wait(), Ok(None))
        {
            info!(target: "overlay", "settings window already open");
            return;
        }
        let started = std::env::current_exe()
            .map(|exe| exe.with_file_name("padpilot-settings.exe"))
            .and_then(|exe| std::process::Command::new(exe).spawn());
        match started {
            Ok(child) => {
                info!(target: "overlay", pid = child.id(), "settings window opened");
                self.settings_window = Some(child);
            }
            Err(err) => error!(target: "overlay", "cannot open the settings window: {err}"),
        }
    }

    /// Pressing the handle starts resizing; the resize timer takes over.
    fn left_press(&mut self, x: i32, y: i32) {
        if self.model.starts_resize(self.layout.region_at(x, y)) {
            self.resizing = Some((self.layout.width() - x, self.layout.height() - y));
            // SAFETY: a plain call on a window this thread owns.
            unsafe { SetTimer(Some(self.window), RESIZE_TIMER, RESIZE_EVERY_MS, None) };
            info!(target: "overlay", "resizing started");
        }
    }

    fn resize_tick(&mut self) {
        let Some((grab_x, grab_y)) = self.resizing else {
            return;
        };
        if !left_button_down() {
            self.end_resize();
            return;
        }
        let (cursor, origin) = (cursor_position(), window_rect(self.window));
        let side = self.layout.side_for_corner(
            cursor.x - origin.left + grab_x,
            cursor.y - origin.top + grab_y,
        );
        self.resize(side, (origin.left, origin.top));
    }

    fn end_resize(&mut self) {
        if self.resizing.take().is_some() {
            // SAFETY: a plain call; the timer may already be gone.
            let _ = unsafe { KillTimer(Some(self.window), RESIZE_TIMER) };
            info!(target: "overlay", side = self.layout.joystick_side(), "resizing ended");
        }
    }

    /// Scrolling over the joystick grows or shrinks it around its center.
    fn scroll(&mut self, delta: i16, screen_x: i32, screen_y: i32) {
        let origin = window_rect(self.window);
        let region = self
            .layout
            .region_at(screen_x - origin.left, screen_y - origin.top);
        if !self.model.can_resize() || region != Region::Joystick {
            return;
        }
        let side = self
            .layout
            .side_after_scroll(f32::from(delta) / WHEEL_NOTCH);
        let (center_x, center_y) = self.layout.joystick_center();
        let center = (origin.left + center_x as i32, origin.top + center_y as i32);
        let new = Layout::new(side.min(self.max_side()), self.layout.scale());
        let (new_x, new_y) = new.joystick_center();
        self.resize(side, (center.0 - new_x as i32, center.1 - new_y as i32));
    }

    /// Gives the joystick a new side (at 100% scaling) and puts the window's
    /// top-left corner at `origin`.
    fn resize(&mut self, side: f32, (x, y): (i32, i32)) {
        let layout = Layout::new(side.min(self.max_side()), self.layout.scale());
        if layout.width() == self.layout.width() {
            return;
        }
        match Surface::new(layout.width(), layout.height()) {
            Ok(surface) => {
                self.layout = layout;
                self.surface = surface;
                self.painter = Painter::new(layout);
                self.changed();
                self.repaint_at(Some(POINT { x, y }));
            }
            Err(err) => error!(target: "overlay", "{err}"),
        }
    }

    /// The largest side, at 100% scaling, that fits the screen the window is on.
    fn max_side(&self) -> f32 {
        let area = work_area(self.window);
        let pixels =
            (area.right - area.left).min(area.bottom - area.top - self.layout.title_bar_height());
        pixels as f32 / self.layout.scale()
    }

    /// A minimized window has no real place on screen; the last one counts.
    fn window_state(&mut self) -> WindowState {
        // SAFETY: a plain call on this thread's window.
        if !unsafe { IsIconic(self.window) }.as_bool() {
            let origin = window_rect(self.window);
            self.position = Some((origin.left, origin.top));
        }
        WindowState {
            position: self.position,
            joystick_side: self.layout.joystick_side(),
            opacity: self.opacity,
        }
    }

    /// The window moved, changed size or opacity: save it soon.
    fn changed(&mut self) {
        self.unsaved_since = Some(Instant::now());
    }

    /// Runs on the heartbeat timer.
    fn tick(&mut self) {
        self.heartbeat();
        if let Some(config) = self.config.changed() {
            self.model.set_settings(config.stick);
        }
        if self.resizing.is_none()
            && self
                .unsaved_since
                .is_some_and(|since| since.elapsed() >= SAVE_AFTER)
        {
            self.save();
        }
    }

    fn save(&mut self) {
        self.unsaved_since = None;
        let state = self.window_state();
        window_state::save(&state);
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
                self.end_resize();
                self.game = focus::foreground();
                let game = self.game.map(focus::describe);
                info!(target: "overlay", limit = ?self.joystick_limit(), game, "entered joystick mode");
                self.joystick_since = Some(Instant::now());
                self.failsafe.arm(self.joystick_limit());
                self.cage_cursor();
                // SAFETY: a plain call on a window this thread owns.
                unsafe { SetTimer(Some(self.window), CAGE_TIMER, CAGE_EVERY_MS, None) };
            }
            Mode::Normal => {
                self.joystick_since = None;
                self.game = None;
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
        self.heartbeat();
        self.repaint();
    }

    /// Joystick mode drives the game only while it is in front.
    fn focus_moved(&mut self, front: HWND) {
        if self.game.is_some_and(|game| game != front) {
            self.leave_joystick_mode("another window came to the front");
        }
    }

    fn joystick_limit(&self) -> Option<Duration> {
        let unwatched = self.watchdog.is_none().then_some(UNWATCHED_JOYSTICK_LIMIT);
        self.options.joystick_limit.or(unwatched)
    }

    /// Tells the watchdog this thread is alive. If the watchdog is gone,
    /// starts a new one, and failing that, joystick mode gets a time limit.
    fn heartbeat(&mut self) {
        let message = Message::Beat(self.model.mode());
        let Some(watchdog) = &mut self.watchdog else {
            return;
        };
        if let Err(err) = watchdog.tell(message) {
            error!(target: "watchdog", pid = watchdog.pid(), "{err}");
            self.watchdog = None;
            if self.watchdog_restarts < MAX_WATCHDOG_RESTARTS {
                self.watchdog_restarts += 1;
                self.watchdog = start_watchdog();
            }
            match &mut self.watchdog {
                Some(watchdog) => {
                    let _ = watchdog.tell(message);
                }
                None if self.model.mode() == Mode::Joystick => {
                    self.failsafe.arm(self.joystick_limit());
                }
                None => {}
            }
        }
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
        if let (Some(after), Some(since)) = (self.options.freeze_after, self.joystick_since)
            && since.elapsed() >= after
        {
            freeze();
        }
        if let Some(game) = self.game {
            if !focus::exists(game) {
                self.leave_joystick_mode("the game window closed");
                return;
            }
            if let Some(front) = focus::foreground() {
                self.focus_moved(front);
                if self.game.is_none() {
                    return;
                }
            }
        }
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
        let layout = Layout::new(self.layout.joystick_side(), scale);
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
        self.repaint_at(None)
    }

    /// Repaints, also moving the window's top-left corner to `at` if given.
    fn repaint_at(&mut self, at: Option<POINT>) -> (Duration, Duration) {
        let started = Instant::now();
        self.stale_since = None;
        let (mode, knob) = (self.model.mode(), self.model.knob());
        self.painter.paint(&mut self.surface.canvas(), mode, knob);
        let painted = Instant::now();
        if let Err(err) = self.surface.present(self.window, self.opacity.alpha(), at) {
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
        WM_LBUTTONDOWN => with_state(|state| state.left_press(x, y)).map(|()| 0),
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16;
            with_state(|state| state.scroll(delta, x, y)).map(|()| 0)
        }
        WM_EXITSIZEMOVE => with_state(State::changed).map(|()| 0),
        WM_LBUTTONUP => {
            // Minimizing and closing send messages back to this window, so
            // they run after the state is released.
            match with_state(|state| state.left_click(x, y))?? {
                Command::Opacity | Command::Settings => {}
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
        WM_TIMER if wparam.0 == HEARTBEAT_TIMER => with_state(State::tick).map(|()| 0),
        WM_TIMER if wparam.0 == RESIZE_TIMER => with_state(State::resize_tick).map(|()| 0),
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
            with_state(|state| {
                state.leave_joystick_mode("the window is closing");
                // Saved now, while the window still has a place on screen.
                state.save();
            });
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            Some(0)
        }
        _ => None,
    }
}

extern "system" fn focus_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    window: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    info!(target: "focus", window = focus::describe(window), "window in front changed");
    with_state(|state| state.focus_moved(window));
}

fn start_watchdog() -> Option<Watchdog> {
    Watchdog::start()
        .inspect_err(|err| {
            warn!(target: "watchdog", "{err}; joystick mode will switch itself off after {} s", UNWATCHED_JOYSTICK_LIMIT.as_secs());
            println!("Watchdog not running: joystick mode will switch itself off after {} s.", UNWATCHED_JOYSTICK_LIMIT.as_secs());
        })
        .ok()
}

/// Stops this thread for good, holding the controller where it is, to
/// check that the watchdog steps in.
fn freeze() -> ! {
    error!(target: "overlay", "freezing on purpose to test the watchdog");
    loop {
        std::thread::sleep(Duration::from_secs(3600));
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

fn cursor_position() -> POINT {
    let mut at = POINT::default();
    // SAFETY: `at` outlives the call.
    let _ = unsafe { GetCursorPos(&mut at) };
    at
}

/// The physical left button, which is the right one when they are swapped.
fn left_button_down() -> bool {
    // SAFETY: plain calls.
    unsafe {
        let key = if GetSystemMetrics(SM_SWAPBUTTON) != 0 {
            VK_RBUTTON
        } else {
            VK_LBUTTON
        };
        GetAsyncKeyState(i32::from(key.0)) < 0
    }
}

/// The work area (screen without the taskbar) of the window's monitor.
fn work_area(window: HWND) -> RECT {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` outlives the call and has its size set.
    unsafe {
        let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(monitor, &mut info);
    }
    info.rcWork
}

/// Whether the title bar at `(x, y)` would be on a screen, so a window saved
/// on a monitor that is gone does not come back out of reach.
fn on_a_screen((x, y): (i32, i32), layout: Layout) -> bool {
    let title_bar = RECT {
        left: x,
        top: y,
        right: x + layout.width(),
        bottom: y + layout.title_bar_height(),
    };
    // SAFETY: a plain call on a local rectangle.
    !unsafe { MonitorFromRect(&title_bar, MONITOR_DEFAULTTONULL) }.is_invalid()
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
