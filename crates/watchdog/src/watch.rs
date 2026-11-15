//! Listens for heartbeats and steps in when they stop.

use std::io::Read;
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};

use padpilot_core::watchdog::{Action, Heard, Message, SILENCE_LIMIT, Watch};
use tracing::{Level, error, info, warn};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ClipCursor, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MessageBoxW,
};
use windows::core::HSTRING;

/// How long to wait for the main app to end once it was told to.
const END_WAIT_MS: u32 = 5000;

pub fn run(parent: Option<String>) -> ExitCode {
    let logs = padpilot_logging::logs_dir();
    let Ok(_log_guard) = padpilot_logging::init(&logs, "watchdog", Level::INFO) else {
        return ExitCode::FAILURE;
    };
    let Some(pid) = parent.and_then(|pid| pid.parse::<u32>().ok()) else {
        error!(target: "watchdog", "started without the main app's process id");
        return ExitCode::FAILURE;
    };
    // Opened now, while the process surely exists, so the id cannot be
    // reused by another process before it is needed.
    // SAFETY: a plain call; the handle is closed below.
    let app = unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid) }
        .inspect_err(|err| error!(target: "watchdog", pid, "cannot open the main app: {err}"))
        .ok();
    info!(target: "watchdog", pid, silence_limit_ms = SILENCE_LIMIT.as_millis() as u64, "watching");

    let heard = listen();
    let mut watch = Watch::default();
    let mut beats = 0u64;
    loop {
        let next = match heard.recv_timeout(SILENCE_LIMIT) {
            Ok(heard) => heard,
            Err(RecvTimeoutError::Timeout) => Heard::Silence,
            Err(RecvTimeoutError::Disconnected) => Heard::Closed,
        };
        if let Heard::Message(Message::Beat(_)) = next {
            beats += 1;
            if beats == 1 {
                info!(target: "watchdog", "first heartbeat received");
            }
        }
        match watch.hear(next) {
            Action::KeepWatching => continue,
            Action::Done => {
                info!(target: "watchdog", beats, "main app closed normally");
            }
            Action::Crashed(mode) => {
                error!(target: "watchdog", ?mode, beats, "main app ended without saying goodbye");
                release_cursor();
                tell_user(
                    "PadPilot closed unexpectedly.\n\nThe cursor is free and the virtual \
                     controller was unplugged.",
                );
            }
            Action::Hung(mode) => {
                error!(target: "watchdog", ?mode, beats, "heartbeat lost: main app stopped responding");
                release_cursor();
                end(app);
                tell_user(
                    "PadPilot stopped responding, so it was closed.\n\nThe cursor is free \
                     and the virtual controller was unplugged.",
                );
            }
            Action::HungClosing => {
                warn!(target: "watchdog", "main app stuck while closing");
                release_cursor();
                end(app);
            }
        }
        break;
    }
    if let Some(app) = app {
        // SAFETY: the handle was opened above and is not used again.
        let _ = unsafe { CloseHandle(app) };
    }
    info!(target: "watchdog", "stopped");
    ExitCode::SUCCESS
}

/// Reads messages on a separate thread, so silence can be timed.
fn listen() -> mpsc::Receiver<Heard> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for byte in std::io::stdin().lock().bytes() {
            let Ok(byte) = byte else { break };
            match Message::from_byte(byte) {
                Some(message) => {
                    if send.send(Heard::Message(message)).is_err() {
                        return;
                    }
                }
                None => warn!(target: "watchdog", byte, "unknown message"),
            }
        }
        // Dropping `send` tells the watcher the pipe closed.
    });
    receive
}

fn release_cursor() {
    // SAFETY: no pointer is passed.
    match unsafe { ClipCursor(None) } {
        Ok(()) => info!(target: "watchdog", "cursor lock released"),
        Err(err) => error!(target: "watchdog", "cannot release the cursor: {err}"),
    }
}

/// Ends the main app; the virtual controller goes with it.
fn end(app: Option<HANDLE>) {
    let Some(app) = app else {
        error!(target: "watchdog", "cannot end the main app: no handle");
        return;
    };
    // SAFETY: `app` is a valid process handle with terminate and wait rights.
    unsafe {
        if let Err(err) = TerminateProcess(app, 1) {
            error!(target: "watchdog", "cannot end the main app: {err}");
            return;
        }
        let waited = WaitForSingleObject(app, END_WAIT_MS);
        info!(target: "watchdog", wait_result = waited.0, "main app ended");
    }
}

fn tell_user(text: &str) {
    // SAFETY: plain message box with no owner; both strings outlive the call.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from("PadPilot watchdog"),
            MB_OK | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}
