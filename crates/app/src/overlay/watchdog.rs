//! Starts the watchdog process and keeps it informed.

use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};

use padpilot_core::watchdog::Message;
use tracing::info;

use crate::error::Error;

const EXE: &str = "padpilot-watchdog.exe";

pub struct Watchdog {
    child: Child,
    pipe: ChildStdin,
}

impl Watchdog {
    /// Starts `padpilot-watchdog.exe` from this program's folder.
    pub fn start() -> Result<Self, Error> {
        let exe = std::env::current_exe()
            .map_err(|err| Error::caused("cannot find this program's folder", err))?
            .with_file_name(EXE);
        let mut child = Command::new(&exe)
            .arg(std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| Error::caused(&format!("cannot start {}", exe.display()), err))?;
        let pipe = child
            .stdin
            .take()
            .ok_or_else(|| Error::new("the watchdog has no input pipe"))?;
        info!(target: "watchdog", pid = child.id(), "watchdog started");
        Ok(Self { child, pipe })
    }

    /// Fails when the watchdog is gone.
    pub fn tell(&mut self, message: Message) -> Result<(), Error> {
        self.pipe
            .write_all(&[message.byte()])
            .map_err(|err| Error::caused("the watchdog is not listening", err))
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}
