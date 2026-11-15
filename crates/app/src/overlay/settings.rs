//! The settings window: started hidden with the joystick, so the Settings
//! button shows it at once.

use std::io::Write;
use std::process::{Child, ChildStdin};

use padpilot_core::config::SHOW_SETTINGS;
use tracing::info;
use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

use crate::error::Error;

const EXE: &str = "padpilot-settings.exe";

pub struct SettingsWindow {
    child: Child,
    pipe: ChildStdin,
}

impl SettingsWindow {
    /// Starts it hidden. It closes when this program closes.
    pub fn start() -> Result<Self, Error> {
        let (child, pipe) = super::helper::start(EXE, &["--hidden"])?;
        info!(target: "overlay", pid = child.id(), "settings window started hidden");
        Ok(Self { child, pipe })
    }

    /// Fails when the settings window is gone.
    pub fn show(&mut self) -> Result<(), Error> {
        // The click came to this program, which may hand the front to the
        // settings window. Nothing to do if it fails: it then just opens behind.
        // SAFETY: a plain call.
        let _ = unsafe { AllowSetForegroundWindow(self.child.id()) };
        self.pipe
            .write_all(&[SHOW_SETTINGS])
            .map_err(|err| Error::caused("the settings window is not listening", err))
    }
}
