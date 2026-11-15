//! Joystick mode's optional time limit.
//!
//! The watchdog process is the main protection against a stuck window. When
//! it is not running, or when a limit is asked for, joystick mode ends after
//! a while. A separate thread enforces it, so the cursor is released even if
//! the window's own thread is stuck.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tracing::{info, warn};
use windows::Win32::UI::WindowsAndMessaging::ClipCursor;

const CHECK_EVERY: Duration = Duration::from_millis(200);
const DISARMED: u64 = 0;
const NO_LIMIT: u64 = u64::MAX;

pub struct Failsafe {
    started: Instant,
    /// When the cursor must be free again, in milliseconds since `started`.
    deadline: Arc<AtomicU64>,
}

impl Failsafe {
    pub fn start() -> Self {
        let failsafe = Self {
            started: Instant::now(),
            deadline: Arc::new(AtomicU64::new(DISARMED)),
        };
        let (started, deadline) = (failsafe.started, Arc::clone(&failsafe.deadline));
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(CHECK_EVERY);
                if expired(started, &deadline) {
                    release_cursor();
                    warn!(target: "overlay", "joystick mode time limit reached, cursor released");
                    // Once is enough; the window re-arms it on the next entry.
                    deadline.store(DISARMED, Ordering::SeqCst);
                }
            }
        });
        failsafe
    }

    /// Joystick mode starts, or the watchdog went away: the clock runs
    /// from now. `None`: no time limit.
    pub fn arm(&self, limit: Option<Duration>) {
        let deadline = limit.map_or(NO_LIMIT, |limit| {
            ((self.started.elapsed() + limit).as_millis() as u64).max(1)
        });
        self.deadline.store(deadline, Ordering::SeqCst);
    }

    /// Joystick mode ended normally.
    pub fn disarm(&self) {
        self.deadline.store(DISARMED, Ordering::SeqCst);
    }

    /// False once the time is up or joystick mode is off: the cursor must
    /// not be locked again.
    pub fn allows_lock(&self) -> bool {
        self.deadline.load(Ordering::SeqCst) != DISARMED && !expired(self.started, &self.deadline)
    }
}

fn expired(started: Instant, deadline: &AtomicU64) -> bool {
    let deadline = deadline.load(Ordering::SeqCst);
    deadline != DISARMED && started.elapsed().as_millis() as u64 >= deadline
}

/// Lets the cursor move over the whole screen again.
pub fn release_cursor() {
    // SAFETY: no pointer is passed. Nothing to do if it fails.
    let _ = unsafe { ClipCursor(None) };
}

/// A crash must not leave the cursor locked.
pub fn release_cursor_on_panic() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        release_cursor();
        previous(info);
    }));
    info!(target: "overlay", "cursor is released on panic");
}
