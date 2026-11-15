//! Starts PadPilot's helper programs, which sit next to this one and listen
//! on their standard input. The pipe closes when this program ends, however
//! it ends, which tells them to stop too.

use std::process::{Child, ChildStdin, Command, Stdio};

use crate::error::Error;

pub fn start(exe: &str, args: &[&str]) -> Result<(Child, ChildStdin), Error> {
    let exe = std::env::current_exe()
        .map_err(|err| Error::caused("cannot find this program's folder", err))?
        .with_file_name(exe);
    let mut child = Command::new(&exe)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| Error::caused(&format!("cannot start {}", exe.display()), err))?;
    let pipe = child
        .stdin
        .take()
        .ok_or_else(|| Error::new(&format!("{} has no input pipe", exe.display())))?;
    Ok((child, pipe))
}
