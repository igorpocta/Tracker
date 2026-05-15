//! Dynamic app icon — renderuje `icons/icon.svg` s accent paletou a aplikuje
//! výsledný PNG na:
//!   * macOS — dock ikonu (`NSApplication.setApplicationIconImage`),
//!   * Windows / Linux — window/taskbar ikonu všech webview oken.
//!
//! Logika: bere zdrojový SVG, nahradí dva hex kódy v `<linearGradient id="brand">`
//! (od → do) za uživatelovu primary / secondary barvu z palety. resvg-render
//! pak vyrobí 512×512 PNG.
//!
//! Mono palety předají stejný hex dvakrát (gradient se sám zploští).

use std::sync::OnceLock;

use tauri::{image::Image, AppHandle, Manager, Runtime};

const APP_ICON_SVG: &str = include_str!("../icons/icon.svg");
const TARGET_SIZE: u32 = 512;

/// Pixely posledně-aplikované ikony, abychom při změně palety nesyntetizovali
/// víckrát to samé. Klíč je `primary|secondary` hex string.
type IconCache = std::sync::Mutex<Option<(String, Vec<u8>)>>;
static LAST: OnceLock<IconCache> = OnceLock::new();

fn cache() -> &'static IconCache {
    LAST.get_or_init(|| std::sync::Mutex::new(None))
}

/// Vyrobí PNG ikony obarvenou paletou. `primary` a `secondary` musí být
/// validní `#RRGGBB`. Pokud `secondary` chybí, použije se primary i pro
/// druhou zastávku (mono palette).
pub fn render_png(primary: &str, secondary: Option<&str>) -> Option<Vec<u8>> {
    let p = sanitize_hex(primary)?;
    let s = secondary
        .and_then(sanitize_hex)
        .unwrap_or_else(|| p.clone());
    let key = format!("{p}|{s}");
    {
        let guard = cache().lock().ok()?;
        if let Some((k, v)) = guard.as_ref() {
            if k == &key {
                return Some(v.clone());
            }
        }
    }

    // Brand gradient v icon.svg má dvě statické zastávky — nahradíme je
    // dynamicky. Použít explicit hex match, ne regex, ať se nesplete s
    // jiným výskytem stejného stringu.
    let svg = APP_ICON_SVG
        .replacen("stop-color=\"#14B8A6\"", &format!("stop-color=\"{p}\""), 1)
        .replacen("stop-color=\"#0F766E\"", &format!("stop-color=\"{s}\""), 1);

    let png = render_svg_to_png(&svg, TARGET_SIZE, TARGET_SIZE)?;
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((key, png.clone()));
    }
    Some(png)
}

/// Aplikuje danou paletu na dock (macOS) / window (Win/Linux) ikonu.
pub fn apply<R: Runtime>(
    app: &AppHandle<R>,
    primary: &str,
    secondary: Option<&str>,
) -> Result<(), String> {
    let png = render_png(primary, secondary).ok_or_else(|| "render failed".to_string())?;
    apply_png(app, &png)
}

fn apply_png<R: Runtime>(app: &AppHandle<R>, png: &[u8]) -> Result<(), String> {
    // Windows / Linux: window icon = taskbar entry. Aplikuje se na všechna
    // webview okna ať popover i main mají stejnou identitu.
    let img = Image::from_bytes(png).map_err(|e| e.to_string())?;
    for w in app.webview_windows().values() {
        let _ = w.set_icon(img.clone());
    }
    // macOS: dock icon přes NSApplication.
    #[cfg(target_os = "macos")]
    set_macos_dock_icon(png);
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_macos_dock_icon(png: &[u8]) {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};

    // Musíme být na main threadu — NSApplication APIs jsou main-thread only.
    // `MainThreadMarker::new()` vrátí None, pokud nejsme; pak skipneme.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let data = NSData::with_bytes(png);
    if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
        unsafe { app.setApplicationIconImage(Some(&img)) };
    }
}

fn render_svg_to_png(svg: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let sx = width as f32 / tree.size().width();
    let sy = height as f32 / tree.size().height();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

fn sanitize_hex(s: &str) -> Option<String> {
    let s = s.trim();
    let body = s.strip_prefix('#').unwrap_or(s);
    if body.len() != 6 {
        return None;
    }
    if !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", body.to_uppercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_hex() {
        assert_eq!(sanitize_hex("#14b8a6"), Some("#14B8A6".to_string()));
        assert_eq!(sanitize_hex("0F766E"), Some("#0F766E".to_string()));
        assert_eq!(sanitize_hex("nope"), None);
        assert_eq!(sanitize_hex("#zzzzzz"), None);
    }

    #[test]
    fn renders_some_png() {
        let png = render_png("#14B8A6", Some("#0F766E")).expect("render");
        assert!(png.len() > 100);
        // PNG magic header.
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn mono_palette_falls_back_to_primary() {
        let png = render_png("#FF0000", None).expect("render");
        assert!(png.len() > 100);
    }
}
