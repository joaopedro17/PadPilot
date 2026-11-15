//! Which window is in front, and hearing when that changes.

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{
    HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent, WINEVENTPROC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    IsWindow, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};
use windows::core::PWSTR;

/// Calls the given function on this thread, from its message loop, each
/// time another program's window comes to the front. Stops when dropped.
pub struct FocusHook(HWINEVENTHOOK);

impl FocusHook {
    pub fn install(on_change: WINEVENTPROC) -> Self {
        // SAFETY: an out-of-context hook; the callback runs on this thread.
        Self(unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                on_change,
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        })
    }
}

impl Drop for FocusHook {
    fn drop(&mut self) {
        // SAFETY: the hook was installed by this thread.
        let _ = unsafe { UnhookWinEvent(self.0) };
    }
}

/// The window in front, if any.
pub fn foreground() -> Option<HWND> {
    // SAFETY: a plain call.
    let window = unsafe { GetForegroundWindow() };
    (!window.is_invalid()).then_some(window)
}

pub fn exists(window: HWND) -> bool {
    // SAFETY: a plain call; any value is allowed.
    unsafe { IsWindow(Some(window)) }.as_bool()
}

/// `"title" (program.exe)`, for the log.
pub fn describe(window: HWND) -> String {
    let mut title = [0u16; 256];
    // SAFETY: the buffer outlives the call.
    let length = unsafe { GetWindowTextW(window, &mut title) }.max(0) as usize;
    let title = String::from_utf16_lossy(&title[..length]);
    let mut pid = 0;
    // SAFETY: `pid` outlives the call.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    format!(
        "{title:?} ({})",
        program(pid).unwrap_or_else(|| format!("pid {pid}"))
    )
}

fn program(pid: u32) -> Option<String> {
    // SAFETY: the handle is closed before returning; the buffer outlives the call.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path = [0u16; 512];
        let mut length = path.len() as u32;
        let named = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        named.ok()?;
        let path = String::from_utf16_lossy(&path[..length as usize]);
        Some(path.rsplit('\\').next().unwrap_or(&path).to_owned())
    }
}
