//! The Sunrise mark: a blocky sun over dunes, drawn from one pixel grid so the
//! title bar and the window icon are the same logo as the .exe's.

use eframe::egui;
use egui::{Color32, CornerRadius, Rect, Vec2};

include!("logo_grid.rs");

fn palette(index: u8) -> Option<Color32> {
    let (r, g, b) = *LOGO_PALETTE.get(index as usize)?;
    if index == 0 {
        return None;
    }
    Some(Color32::from_rgb(r, g, b))
}

/// Paint the mark into `rect`, one square per logo pixel. Sizes that are a
/// multiple of 16 stay perfectly crisp.
pub(crate) fn paint(painter: &egui::Painter, rect: Rect) {
    let cell = rect.width() / LOGO_W as f32;
    for (y, row) in LOGO_ART.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let Some(color) = palette((ch as u8).wrapping_sub(b'0')) else {
                continue;
            };
            let min = egui::pos2(rect.left() + x as f32 * cell, rect.top() + y as f32 * cell);
            // A hair of overlap keeps scaled-up cells from showing seams.
            painter.rect_filled(
                Rect::from_min_size(min, Vec2::splat(cell + 0.5)),
                CornerRadius::ZERO,
                color,
            );
        }
    }
}

/// RGBA pixels for the window icon, scaled up by whole pixels.
pub(crate) fn rgba(size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let gx = (x * LOGO_W as u32 / size) as usize;
            let gy = (y * LOGO_H as u32 / size) as usize;
            let index = (LOGO_ART[gy].as_bytes()[gx] as u8).wrapping_sub(b'0');
            let (r, g, b) = LOGO_PALETTE[index as usize];
            let alpha = if index == 0 { 0 } else { 255 };
            let i = ((y * size + x) * 4) as usize;
            out[i] = r;
            out[i + 1] = g;
            out[i + 2] = b;
            out[i + 3] = alpha;
        }
    }
    out
}
