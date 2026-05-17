//! Global mouse-down monitor pro popover (macOS).
//!
//! `WindowEvent::Focused(false)` v `popover::setup` zachytí typický blur
//! (klik do main okna, alt-tab atd.), ale řadu macOS-specifických případů
//! nepokrývá:
//!  - Klik na menubar widget (Wi-Fi, hodiny, status icon jiné appky) ne
//!    vždy přepne aktivní app — popover zůstal viditelný.
//!  - Klik do system tray / Control Center popoveru jiné aplikace.
//!  - Klik na desktop (Finder), který fokus pošle do desktopu, ale naše
//!    `NSPanel`-like okno ho dostávalo zpět dřív, než blur vůbec doletěl.
//!
//! Řešením je `NSEvent.addGlobalMonitorForEventsMatchingMask:handler:`,
//! což je dokumentovaný Apple-pattern pro NSPopover-like chování:
//! handler dostane každý mouse-down kdekoliv MIMO naši aplikaci. Naše
//! aplikace má vlastní `local` handler pro kliky dovnitř, ten není potřeba
//! protože uvnitř popoveru klik mít efekt MÁ.
//!
//! Monitor se instaluje při zobrazení popoveru a odinstaluje při skrytí,
//! takže pokud popover není vidět, nevoláme zbytečně handler na každý
//! klik kdekoliv na obrazovce.

use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask};

use tauri::{AppHandle, Manager, Runtime};

use crate::popover::POPOVER_LABEL;

/// Wrap kolem `Retained<AnyObject>` aby šel uložit v `static Mutex<...>`.
/// `AnyObject` není automaticky `Send`/`Sync` protože v Obj-C runtime se s ním
/// musí pracovat výhradně z main threadu. K instalaci a sundání monitoru
/// přistupujeme jen z hlavního vlákna (NSEvent API je main-thread only),
/// takže manuální `unsafe impl` je bezpečný.
struct MonitorHandle(Retained<AnyObject>);

// SAFETY: viz komentář nad `MonitorHandle` — vždy se k němu sahá z main
// threadu, Mutex zde slouží jen pro borrow-check, ne pro cross-thread sync.
unsafe impl Send for MonitorHandle {}
unsafe impl Sync for MonitorHandle {}

static MONITOR: Mutex<Option<MonitorHandle>> = Mutex::new(None);

/// Nainstaluje global mouse monitor (idempotentní — opakovaný call NIC
/// neudělá). Při kliku kdekoliv mimo naši appku schová popover.
///
/// Musí být volán z main threadu. Volá se v `show_under` / `show_centered`.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    let mut guard = match MONITOR.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if guard.is_some() {
        return;
    }

    // Při kliku schováme popover. AppHandle je `Send + Sync + Clone`, takže
    // ho můžeme přesunout do bloku.
    let app_handle = app.clone();
    let block = RcBlock::new(move |_evt: std::ptr::NonNull<NSEvent>| {
        if let Some(win) = app_handle.get_webview_window(POPOVER_LABEL) {
            if win.is_visible().unwrap_or(false) {
                let _ = win.hide();
            }
        }
    });

    let mask =
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown;

    let handle = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &block);
    *guard = handle.map(MonitorHandle);
}

/// Sundá monitor (idempotentní). Volá se v `hide`.
pub fn uninstall() {
    let mut guard = match MONITOR.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(monitor) = guard.take() {
        unsafe { NSEvent::removeMonitor(&monitor.0) };
    }
}
