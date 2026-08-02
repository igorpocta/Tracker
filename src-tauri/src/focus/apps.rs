//! Platform glue for observing and nudging applications.
//!
//! Three operations, each a no-op on unsupported platforms so the engine can
//! stay platform-agnostic:
//!
//! * [`frontmost_app`] — which app the user is looking at right now
//! * [`hide_pid`] / [`kill_pid`] — enforcement
//! * [`running_apps`] — candidates for the rule picker in Settings
//!
//! The engine only ever enforces against the *foreground* app, so none of
//! this walks the full process table looking for things to terminate. The one
//! exception is the opt-in kill sweep when a session starts, which matches
//! explicit rules against [`running_apps`].

use super::rules::AppIdent;

// -----------------------------------------------------------------------------
// macOS
// -----------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use super::AppIdent;
    use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};

    fn ident_from(app: &NSRunningApplication) -> AppIdent {
        AppIdent {
            bundle_id: app.bundleIdentifier().map(|s| s.to_string()),
            exe: None,
            name: app.localizedName().map(|s| s.to_string()),
            pid: i64::from(app.processIdentifier()),
        }
    }

    pub fn frontmost_app() -> Option<AppIdent> {
        let ws = NSWorkspace::sharedWorkspace();
        let front = ws.frontmostApplication()?;
        Some(ident_from(&front))
    }

    pub fn running_apps() -> Vec<AppIdent> {
        let ws = NSWorkspace::sharedWorkspace();
        ws.runningApplications()
            .iter()
            // `Regular` == has a Dock icon. Agents and daemons are not things
            // a user thinks of as "apps", and listing them would bury the
            // handful of entries that matter.
            .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
            .map(|app| ident_from(&app))
            .collect()
    }

    fn app_for_pid(pid: i64) -> Option<objc2::rc::Retained<NSRunningApplication>> {
        let pid = i32::try_from(pid).ok()?;
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
    }

    pub fn hide_pid(pid: i64) -> bool {
        app_for_pid(pid).map(|app| app.hide()).unwrap_or(false)
    }

    /// Graceful terminate only. `forceTerminate` (SIGKILL) would guarantee the
    /// loss of anything unsaved, and the whole point of defaulting to `hide`
    /// is that Focus mode never destroys the user's work.
    pub fn kill_pid(pid: i64) -> bool {
        app_for_pid(pid).map(|app| app.terminate()).unwrap_or(false)
    }

    /// Is an app with this bundle identifier currently running? Used to avoid
    /// AppleScript-ing a browser that isn't open — `tell application "Safari"`
    /// would *launch* it.
    pub fn is_running(bundle_id: &str) -> bool {
        let ws = NSWorkspace::sharedWorkspace();
        ws.runningApplications().iter().any(|app| {
            app.bundleIdentifier()
                .map(|b| b.to_string().eq_ignore_ascii_case(bundle_id))
                .unwrap_or(false)
        })
    }
}

// -----------------------------------------------------------------------------
// Windows
// -----------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod imp {
    use super::AppIdent;
    use std::os::raw::c_void;

    type Hwnd = *mut c_void;
    type Handle = *mut c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const SW_MINIMIZE: i32 = 6;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> Hwnd;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
        fn EnumWindows(callback: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetWindowTextW(hwnd: Hwnd, buf: *mut u16, len: i32) -> i32;
        fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn QueryFullProcessImageNameW(
            handle: Handle,
            flags: u32,
            buf: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn TerminateProcess(handle: Handle, exit_code: u32) -> i32;
    }

    /// Full image path of a process, or `None` when the process is gone or
    /// belongs to another user / integrity level.
    fn exe_name_for_pid(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut buf = [0u16; 512];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
            CloseHandle(handle);
            if ok == 0 || size == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..size as usize]);
            // Rules are written against the file name, not the install path.
            path.rsplit(['\\', '/']).next().map(str::to_string)
        }
    }

    fn window_title(hwnd: Hwnd) -> Option<String> {
        unsafe {
            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return None;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let written = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            if written <= 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..written as usize]))
        }
    }

    fn pid_of_window(hwnd: Hwnd) -> u32 {
        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid
    }

    pub fn frontmost_app() -> Option<AppIdent> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return None;
        }
        let pid = pid_of_window(hwnd);
        if pid == 0 {
            return None;
        }
        let exe = exe_name_for_pid(pid);
        Some(AppIdent {
            bundle_id: None,
            // Strip the `.exe` for a friendlier display name; rule matching
            // handles both spellings.
            name: exe
                .as_deref()
                .map(|e| e.trim_end_matches(".exe").to_string()),
            exe,
            pid: i64::from(pid),
        })
    }

    /// Collector handed to `EnumWindows` through the `isize` user parameter.
    /// Only top-level windows that are visible *and* titled are considered —
    /// that filters the process table down to things the user recognises.
    extern "system" fn collect_window(hwnd: Hwnd, param: isize) -> i32 {
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let Some(title) = window_title(hwnd) else {
            return 1;
        };
        if title.trim().is_empty() {
            return 1;
        }
        let pid = pid_of_window(hwnd);
        if pid == 0 {
            return 1;
        }
        let Some(exe) = exe_name_for_pid(pid) else {
            return 1;
        };
        // SAFETY: `param` is the `&mut Vec` we handed to `EnumWindows` below,
        // and the callback only runs for the duration of that call.
        let out = unsafe { &mut *(param as *mut Vec<AppIdent>) };
        out.push(AppIdent {
            bundle_id: None,
            name: Some(exe.trim_end_matches(".exe").to_string()),
            exe: Some(exe),
            pid: i64::from(pid),
        });
        1
    }

    pub fn running_apps() -> Vec<AppIdent> {
        let mut found: Vec<AppIdent> = Vec::new();
        unsafe {
            EnumWindows(collect_window, &mut found as *mut Vec<AppIdent> as isize);
        }
        // One entry per executable — a browser with six windows is still one
        // thing to block.
        let mut seen = std::collections::HashSet::new();
        found.retain(|app| {
            let key = app.exe.clone().unwrap_or_default().to_ascii_lowercase();
            seen.insert(key)
        });
        found
    }

    /// Minimise every visible top-level window belonging to `pid`.
    pub fn hide_pid(pid: i64) -> bool {
        struct Target {
            pid: u32,
            hit: bool,
        }
        extern "system" fn minimize(hwnd: Hwnd, param: isize) -> i32 {
            if unsafe { IsWindowVisible(hwnd) } == 0 {
                return 1;
            }
            // SAFETY: see `collect_window`.
            let target = unsafe { &mut *(param as *mut Target) };
            if pid_of_window(hwnd) == target.pid {
                unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
                target.hit = true;
            }
            1
        }

        let Ok(pid) = u32::try_from(pid) else {
            return false;
        };
        let mut target = Target { pid, hit: false };
        unsafe {
            EnumWindows(minimize, &mut target as *mut Target as isize);
        }
        target.hit
    }

    pub fn kill_pid(pid: i64) -> bool {
        let Ok(pid) = u32::try_from(pid) else {
            return false;
        };
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle.is_null() {
                // Elevated or another user's process — we simply can't, and
                // that is fine: the caller falls back to hiding.
                return false;
            }
            let ok = TerminateProcess(handle, 0);
            CloseHandle(handle);
            ok != 0
        }
    }

    pub fn is_running(_bundle_id: &str) -> bool {
        false
    }
}

// -----------------------------------------------------------------------------
// Everything else (Linux, headless CI)
// -----------------------------------------------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    use super::AppIdent;

    pub fn frontmost_app() -> Option<AppIdent> {
        None
    }
    pub fn running_apps() -> Vec<AppIdent> {
        Vec::new()
    }
    pub fn hide_pid(_pid: i64) -> bool {
        false
    }
    pub fn kill_pid(_pid: i64) -> bool {
        false
    }
    pub fn is_running(_bundle_id: &str) -> bool {
        false
    }
}

/// The application currently in the foreground, if we can tell.
pub fn frontmost_app() -> Option<AppIdent> {
    imp::frontmost_app()
}

/// Applications the user could plausibly want to block — anything with a Dock
/// icon (macOS) or a visible titled window (Windows).
pub fn running_apps() -> Vec<AppIdent> {
    imp::running_apps()
}

/// Push an application out of the way without touching its state.
pub fn hide_pid(pid: i64) -> bool {
    imp::hide_pid(pid)
}

/// Ask an application to quit. Falls back to `false` when we lack the rights.
pub fn kill_pid(pid: i64) -> bool {
    imp::kill_pid(pid)
}

/// macOS only: is an app with this bundle id running? Always `false`
/// elsewhere.
pub fn is_running(bundle_id: &str) -> bool {
    imp::is_running(bundle_id)
}
