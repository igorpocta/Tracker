//! Systémová idle-detekce — kolik sekund uplynulo od posledního vstupu
//! (myš, klávesnice) na úrovni OS, ne jen v okně aplikace.
//!
//! Hook `useIdleDetection` na frontendu dříve registroval `mousemove` /
//! `keydown` na `window`, takže ho probudil jakýkoliv pohyb v Trackeru —
//! ale když uživatel pracoval v IDE nebo prohlížeči, Tracker pořád viděl
//! "byl si pryč" a vyhodil idle-modal po návratu. Tento command vrací
//! reálný systémový idle čas, takže hook může polling-em (každých pár
//! sekund) sledovat opravdovou (ne)aktivitu.
//!
//! Platform support:
//! - **macOS** — `CGEventSourceSecondsSinceLastEventType` z
//!   ApplicationServices.framework. Bere combined keyboard+mouse state
//!   a vrací f64 sekund od posledního libovolného input eventu.
//! - **Windows** — `GetLastInputInfo` + `GetTickCount` (rozdíl v ms).
//! - **Linux / ostatní** — vrací `0` (neumíme bezpečně bez X11/Wayland
//!   bindings; Tracker je primárně macOS app, takže fallback je OK).

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    /// `CGEventSourceSecondsSinceLastEventType(stateID, eventType) -> double`.
    /// `stateID = 0` = `kCGEventSourceStateCombinedSessionState` (combined HID).
    /// `eventType = !0u32` = `kCGAnyInputEventType` (libovolný input).
    fn CGEventSourceSecondsSinceLastEventType(state: u32, event_type: u32) -> std::os::raw::c_double;
}

/// Vrátí počet sekund od posledního systémového input eventu.
/// Na nepodporovaných platformách (Linux, headless) vrací 0.
pub fn system_idle_seconds() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe {
        let secs = CGEventSourceSecondsSinceLastEventType(0, !0u32);
        if secs.is_finite() && secs >= 0.0 {
            secs as u64
        } else {
            0
        }
    }
    #[cfg(target_os = "windows")]
    unsafe {
        windows_idle()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        0
    }
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct LastInputInfo {
    cb_size: u32,
    dw_time: u32,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
}

#[cfg(target_os = "windows")]
unsafe fn windows_idle() -> u64 {
    let mut info = LastInputInfo {
        cb_size: std::mem::size_of::<LastInputInfo>() as u32,
        dw_time: 0,
    };
    if GetLastInputInfo(&mut info) == 0 {
        return 0;
    }
    let now = GetTickCount();
    // `dw_time` je tickcount posledního inputu — rozdíl v ms.
    // `wrapping_sub` ošetří 49.7-day rollover.
    let diff_ms = now.wrapping_sub(info.dw_time);
    (diff_ms as u64) / 1000
}

#[tauri::command]
pub async fn get_system_idle_seconds() -> Result<u64, String> {
    Ok(system_idle_seconds())
}
