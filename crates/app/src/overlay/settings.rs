//! The settings window: started hidden with the joystick, so the Settings
//! button shows it at once.

use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};

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
        let exe = std::env::current_exe()
            .map_err(|err| Error::caused("cannot find this program's folder", err))?
            .with_file_name(EXE);
        let mut child = Command::new(&exe)
            .arg("--hidden")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|err| Error::caused(&format!("cannot start {}", exe.display()), err))?;
        let pipe = child
            .stdin
            .take()
            .ok_or_else(|| Error::new("the settings window has no input pipe"))?;
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
