//! System notification suppression during a focus session.
//!
//! Neither platform exposes a supported API for toggling Do Not Disturb, so
//! this module does the honest thing rather than the impressive-looking one:
//!
//! * **macOS** — runs a Shortcut the user picks in Settings. Writing to
//!   `~/Library/DoNotDisturb/DB` or `com.apple.ncprefs` is the trick that
//!   circulates online; Apple has broken it repeatedly and it does not work on
//!   current releases. Shortcuts is the only path that keeps working.
//! * **Windows** — opens the Focus Assist page in Settings. There is no
//!   documented toggle, and the undocumented registry blob changes shape
//!   between builds.
//!
//! Tracker's *own* notifications are suppressed unconditionally by the caller
//! checking [`crate::focus::engine::is_active`]; that part needs no OS help.

use std::process::{Command, Stdio};

/// Names of the user's Shortcuts, for the picker in Settings. Empty on
/// non-macOS or when the `shortcuts` binary is unavailable (pre-Monterey).
pub fn list_macos_shortcuts() -> Vec<String> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    let Ok(output) = Command::new("shortcuts")
        .arg("list")
        .stdin(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Run a Shortcut by name. Fire-and-forget: we don't wait for the Shortcut to
/// finish because some of them show UI.
pub fn run_macos_shortcut(name: &str) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Zkratky jsou dostupné jen na macOS.".into());
    }
    if name.trim().is_empty() {
        return Err("Název zkratky je prázdný.".into());
    }
    Command::new("shortcuts")
        .arg("run")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Nepodařilo se spustit zkratku: {e}"))
}

/// Open the OS page where Do Not Disturb / Focus lives, so the user can flip
/// it by hand when automation isn't available.
pub fn open_system_dnd_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:quiethours"])
            .stdin(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Nepodařilo se otevřít nastavení: {e}"))
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.Focus-Settings.extension")
            .stdin(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Nepodařilo se otevřít nastavení: {e}"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Na této platformě není systémové Nerušit dostupné.".into())
    }
}

/// Turn the system's Do Not Disturb on or off for a focus session.
///
/// Returns `Ok(false)` when nothing could be done automatically — the caller
/// surfaces that to the user instead of silently pretending it worked.
pub fn apply(enable: bool, shortcut_on: Option<&str>, shortcut_off: Option<&str>) -> bool {
    if !cfg!(target_os = "macos") {
        // Windows has no scriptable toggle; the Settings deep-link is offered
        // from the UI instead.
        return false;
    }
    let name = if enable { shortcut_on } else { shortcut_off };
    match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => run_macos_shortcut(n).is_ok(),
        None => false,
    }
}
