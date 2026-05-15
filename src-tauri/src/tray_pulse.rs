//! Tray recording-pulse frames.
//!
//! The macOS menu bar cannot color individual characters in a `set_title`
//! string — text is rendered in the system foreground color (white or black).
//! The way to get a visibly-red, smoothly-pulsing recording indicator is to
//! drive it through the **tray icon** instead: swap between several PNG frames
//! at higher frequency than the per-second title tick.
//!
//! This module:
//! 1. Embeds the master red-dot PNG (`tray-rec-base.png`) at compile time.
//! 2. Decodes it once at first access (`OnceLock`).
//! 3. On demand emits a PNG byte buffer with the alpha channel multiplied by
//!    a sine-wave value, yielding a smooth fade in/out.
//!
//! The seven precomputed alpha multipliers approximate one full sine period
//! across the cycle (peak at index 3, troughs at 0 and 6).
//!
//! Frames are produced lazily (per-request) but the underlying RGBA buffer is
//! cached. PNG re-encoding is the only per-frame cost, ~0.2 ms for 44×44.
//!
//! Tested via `cargo test` — alpha math + PNG round-trip.

use std::io::Cursor;
use std::sync::OnceLock;

use image::{ImageBuffer, ImageError, Rgba};

/// Bílé „Zzz" silueta pro idle tray ikonu. Renderuje se z přiloženého SVG
/// jednou (cached), 44×44 px ať odpovídá `tray-rec-base.png`. Když render
/// selže (jiná platforma, exotická Tauri konfigurace), caller se vrátí na
/// předchozí monochrome template ikonu.
const ZZZ_SVG: &str = include_str!("../icons/tray-idle-zzz.svg");
static ZZZ_PNG: OnceLock<Vec<u8>> = OnceLock::new();

pub fn idle_zzz_png() -> Option<&'static [u8]> {
    let v = ZZZ_PNG.get_or_init(|| render_svg_to_png(ZZZ_SVG, 44, 44).unwrap_or_default());
    if v.is_empty() {
        None
    } else {
        Some(v.as_slice())
    }
}

fn render_svg_to_png(svg: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let sx = width as f32 / tree.size().width();
    let sy = height as f32 / tree.size().height();
    let transform = tiny_skia::Transform::from_scale(sx, sy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().ok()
}

/// Alpha multipliers, one per pulse phase. Approximates a sine wave:
/// `0.5 + 0.5 * sin(2π · i / N)` for `i ∈ {1, 2, 3, 4, 5, 6, 0}` to start
/// dim, peak in the middle, end dim.
const PULSE_ALPHAS: [f32; 7] = [0.20, 0.42, 0.78, 1.00, 0.78, 0.42, 0.20];

/// Total number of pulse frames. Public so the ticker can `idx % FRAME_COUNT`.
pub const FRAME_COUNT: usize = PULSE_ALPHAS.len();

static BASE_RGBA: OnceLock<ImageBuffer<Rgba<u8>, Vec<u8>>> = OnceLock::new();

/// Decode the embedded base PNG once and cache its RGBA pixel buffer.
fn base_rgba() -> &'static ImageBuffer<Rgba<u8>, Vec<u8>> {
    BASE_RGBA.get_or_init(|| {
        let bytes = include_bytes!("../icons/tray-rec-base.png");
        image::load_from_memory(bytes)
            .expect("tray-rec-base.png is embedded and must decode")
            .to_rgba8()
    })
}

/// Return PNG bytes for pulse frame `idx`. The frame index is reduced modulo
/// `FRAME_COUNT`, so callers may simply pass a monotonically increasing tick
/// counter.
pub fn frame_png(idx: usize) -> Result<Vec<u8>, ImageError> {
    let alpha_mult = PULSE_ALPHAS[idx % FRAME_COUNT];
    let base = base_rgba();

    // Clone the buffer with alpha scaled. We only touch the alpha channel —
    // RGB stays the same so the red color is preserved.
    let scaled: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_fn(base.width(), base.height(), |x, y| {
            let p = base.get_pixel(x, y);
            let a_scaled = ((p.0[3] as f32) * alpha_mult).round().clamp(0.0, 255.0) as u8;
            Rgba([p.0[0], p.0[1], p.0[2], a_scaled])
        });

    // Encode back to PNG. tray APIs accept raw PNG bytes via `Image::from_bytes`.
    let mut out: Vec<u8> = Vec::with_capacity(1024);
    {
        let encoder = image::codecs::png::PngEncoder::new(Cursor::new(&mut out));
        use image::ImageEncoder;
        encoder.write_image(
            scaled.as_raw(),
            scaled.width(),
            scaled.height(),
            image::ExtendedColorType::Rgba8,
        )?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_count_matches_alpha_table() {
        assert_eq!(FRAME_COUNT, PULSE_ALPHAS.len());
    }

    #[test]
    fn frame_png_returns_decodable_image() {
        for idx in 0..FRAME_COUNT {
            let bytes = frame_png(idx).expect("encode");
            let img = image::load_from_memory(&bytes).expect("decode");
            assert_eq!(img.width(), 44);
            assert_eq!(img.height(), 44);
        }
    }

    #[test]
    fn frame_png_index_wraps_around() {
        // idx 7 should produce the same bytes as idx 0 (modulo).
        let a = frame_png(0).unwrap();
        let b = frame_png(FRAME_COUNT).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn peak_frame_has_more_opaque_pixels_than_trough() {
        let peak_bytes = frame_png(3).unwrap();
        let trough_bytes = frame_png(0).unwrap();
        // Decode each and count pixels above 50% alpha — peak must have more.
        let peak = image::load_from_memory(&peak_bytes).unwrap().to_rgba8();
        let trough = image::load_from_memory(&trough_bytes).unwrap().to_rgba8();
        let peak_strong = peak.pixels().filter(|p| p.0[3] > 128).count();
        let trough_strong = trough.pixels().filter(|p| p.0[3] > 128).count();
        assert!(peak_strong > trough_strong);
    }
}
