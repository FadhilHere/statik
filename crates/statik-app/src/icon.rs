//! The application icon, drawn in code: a white "S" on a dark blue rounded square.
//!
//! The "S" is two thick circular arcs stacked on top of each other: the upper one open on its
//! lower right, the lower one open on its upper left. Edges are anti-aliased by distance.

/// Edge length of the generated icon in pixels.
pub const ICON_SIZE: u32 = 256;

const BLUE: [f32; 3] = [0x1f as f32, 0x3a as f32, 0x6b as f32];
const WHITE: [f32; 3] = [255.0, 255.0, 255.0];

/// Coverage of a pixel whose signed distance to an edge is `d` (negative = inside).
fn cover(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Signed distance to a rounded square spanning `[m, size - m]` with corner radius `r`.
fn rounded_square(x: f32, y: f32, size: f32, m: f32, r: f32) -> f32 {
    let c = size / 2.0;
    let half = c - m;
    let qx = (x - c).abs() - (half - r);
    let qy = (y - c).abs() - (half - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// Signed distance to a ring arc around `(cx, cy)` with radius `r` and stroke `w`, covering
/// the angles (y pointing down, degrees, measured from +x) from `from` clockwise to `to`.
/// Ends are rounded.
#[allow(clippy::too_many_arguments)]
fn arc(x: f32, y: f32, cx: f32, cy: f32, r: f32, w: f32, from: f32, to: f32) -> f32 {
    let (dx, dy) = (x - cx, y - cy);
    let a = dy.atan2(dx).to_degrees();
    let span = (to - from).rem_euclid(360.0);
    let rel = (a - from).rem_euclid(360.0);
    if rel <= span {
        return ((dx * dx + dy * dy).sqrt() - r).abs() - w / 2.0;
    }
    // Outside the angular range: distance to the nearer rounded end cap.
    let mut best = f32::MAX;
    for end in [from, to] {
        let t = end.to_radians();
        let (ex, ey) = (cx + r * t.cos(), cy + r * t.sin());
        best = best.min(((x - ex).powi(2) + (y - ey).powi(2)).sqrt() - w / 2.0);
    }
    best
}

/// RGBA pixels (row-major, `size * size * 4` bytes) of the icon at `size` pixels.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let k = s / 256.0;
    let mut px = Vec::with_capacity((size * size * 4) as usize);
    for j in 0..size {
        for i in 0..size {
            let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
            let bg = cover(rounded_square(x, y, s, 8.0 * k, 44.0 * k));
            let (r, w) = (38.0 * k, 28.0 * k);
            let upper = arc(x, y, 128.0 * k, 92.0 * k, r, w, 90.0, 330.0);
            let lower = arc(x, y, 128.0 * k, 164.0 * k, r, w, 270.0, 150.0);
            let fg = cover(upper.min(lower)) * bg;
            for c in 0..3 {
                px.push((BLUE[c] * (1.0 - fg) + WHITE[c] * fg).round() as u8);
            }
            px.push((bg * 255.0).round() as u8);
        }
    }
    px
}

/// The icon for `ViewportBuilder::with_icon`.
pub fn icon_data() -> egui::IconData {
    egui::IconData {
        rgba: icon_rgba(ICON_SIZE),
        width: ICON_SIZE,
        height: ICON_SIZE,
    }
}
