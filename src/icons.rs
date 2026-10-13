//! Khipu's identity — a bird on a knotted cord — at the size and contrast of the surface showing it.
//!
//! Explorer reads the multi-size resource in the executable, and the MSIX logos are files; both are
//! the identity pack in `assets/khipu/` exactly as delivered, and nothing here writes them. What the
//! app draws for itself at run time is drawn here, from the pack's vector master (`fuentes/`), at its
//! own physical size: the notification area at any DPI (175 % asks for 28 px, a size the pack has
//! no file for), the window when the exe carries no resource, the welcome page. The tests hold this
//! drawing to the pack's PNGs, so the two cannot drift apart unnoticed.
//!
//! Tray colours follow the WINDOWS shell, never the app's independently selectable theme. The
//! caller caches Appearance and only rebuilds on a setting, DPI or state change. This module does
//! no polling, starts no threads and never changes settings. Paused keeps the silhouette and only
//! lowers its intensity, as the identity specifies; the tray menu says Resume in words.

use eframe::egui::IconData;
use windows::core::w;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SM_CXICON, SM_CXSMICON};
use winit::platform::windows::{IconExtWindows, WindowExtWindows};

/// The brand's two fixed colours (`fuentes/tokens.json`): the app icon's square, and the bird on it.
const VIOLET: [u8; 3] = [0xA9, 0x9B, 0xE7];
const INK: [u8; 3] = [0x21, 0x1B, 0x37];

/// The bird's colour on a surface, running and paused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tones {
    pub active: [u8; 3],
    pub paused: [u8; 3],
}

impl Tones {
    /// For a light taskbar or page.
    pub const LIGHT: Self = Self { active: [0x22, 0x25, 0x2C], paused: [0x74, 0x77, 0x81] };
    /// For a dark taskbar or page.
    pub const DARK: Self = Self { active: [0xF3, 0xF4, 0xF7], paused: [0x94, 0x97, 0xA1] };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub size: u32,
    pub tones: Tones,
    /// The dot that says there is news: Windows' accent, in the shade that reads on this taskbar.
    pub accent: [u8; 3],
}

impl Appearance {
    pub fn current() -> Self {
        // The manager can be open on a different monitor from the notification area. Measuring
        // the manager would choose the wrong resource on mixed-DPI desktops.
        let dpi = unsafe {
            FindWindowW(w!("Shell_TrayWnd"), None)
                .ok()
                .map(|hwnd| GetDpiForWindow(hwnd))
                .filter(|&dpi| dpi != 0)
                .unwrap_or_else(|| GetDpiForSystem().max(96))
        };
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.clamp(8, 128) as u32;
        let light = crate::theme::is_light_taskbar();
        let (tones, accent) = if let Some((face, mark)) = crate::theme::contrast_colors() {
            // Only the system's colours in a contrast theme. Paused sits halfway to the
            // background: still the system's two colours, still told apart. The dot is the mark
            // too, set apart from the bird by the gap around it.
            let half = |i: usize| ((mark[i] as u16 + face[i] as u16) / 2) as u8;
            (Tones { active: mark, paused: [half(0), half(1), half(2)] }, mark)
        } else {
            let (r, g, b) = crate::theme::system_accent(light);
            (if light { Tones::LIGHT } else { Tones::DARK }, [r, g, b])
        };
        Self { size, tones, accent }
    }

    /// `news` adds the dot in the corner that says «Novedades» is waiting.
    pub fn tray_icon(self, paused: bool, news: bool) -> Result<tray_icon::Icon, String> {
        let mut data = symbol(self.size, if paused { self.tones.paused } else { self.tones.active });
        if news {
            badge(&mut data, self.accent);
        }
        tray_icon::Icon::from_rgba(data.rgba, data.width, data.height).map_err(|e| e.to_string())
    }
}

/// The window's icon until [`apply_window`] puts the exe's own resource on it.
pub fn window_fallback() -> Option<IconData> {
    Some(app(64))
}

/// winit exposes separate small/title and large/taskbar icons on Windows. Let the resource loader
/// choose the exact supplied size, rather than shrink a 256px icon into the 16px title bar.
pub fn apply_window(window: &winit::window::Window) -> u32 {
    let dpi = (window.scale_factor() * 96.0).round() as u32;
    let load = |metric| {
        let size = unsafe { GetSystemMetricsForDpi(metric, dpi) }.clamp(8, 256) as u32;
        winit::window::Icon::from_resource(1, Some(winit::dpi::PhysicalSize::new(size, size)))
            .or_else(|_| {
                // Preserve the existing build-without-Windows-SDK behaviour: resources are
                // optional on that developer machine, but the running app still has an icon.
                let data = app(size);
                winit::window::Icon::from_rgba(data.rgba, data.width, data.height)
            })
            .ok()
    };
    window.set_window_icon(load(SM_CXSMICON));
    window.set_taskbar_icon(load(SM_CXICON));
    dpi
}

/// The bird alone, in one colour, `px` pixels square: the tray's icon.
pub fn symbol(px: u32, colour: [u8; 3]) -> IconData {
    draw(px, false, colour)
}

/// The news dot, on the 40-unit canvas: in the top right corner, which the bird leaves empty, with
/// a clear ring around it so it reads as a mark on the icon and not as part of the drawing.
const DOT_CENTRE: Point = [33.5, 6.5];
const DOT_RADIUS: f32 = 6.0;
const DOT_GAP: f32 = 2.5;

/// Puts the news dot on an icon already drawn. Each pixel is sampled like the bird, so the dot is
/// as round at 16 px as at 64.
fn badge(icon: &mut IconData, accent: [u8; 3]) {
    let px = icon.width;
    let n = 8;
    let step = 40.0 / (px * n) as f32;
    let (dot, clear) = (DOT_RADIUS * DOT_RADIUS, (DOT_RADIUS + DOT_GAP).powi(2));
    for py in 0..px {
        for qx in 0..px {
            let (mut on_dot, mut in_gap) = (0u32, 0u32);
            for sy in 0..n {
                let y = ((py * n + sy) as f32 + 0.5) * step;
                for sx in 0..n {
                    let x = ((qx * n + sx) as f32 + 0.5) * step;
                    let d = (x - DOT_CENTRE[0]).powi(2) + (y - DOT_CENTRE[1]).powi(2);
                    if d <= dot {
                        on_dot += 1;
                    } else if d <= clear {
                        in_gap += 1;
                    }
                }
            }
            if on_dot + in_gap == 0 {
                continue;
            }
            let samples = (n * n) as f32;
            let (f_dot, f_bird) = (on_dot as f32 / samples, 1.0 - (on_dot + in_gap) as f32 / samples);
            let pixel = &mut icon.rgba[((py * px + qx) * 4) as usize..][..4];
            let bird = pixel[3] as f32 / 255.0 * f_bird;
            let alpha = f_dot + bird;
            for i in 0..3 {
                pixel[i] = if alpha > 0.0 {
                    ((accent[i] as f32 * f_dot + pixel[i] as f32 * bird) / alpha).round() as u8
                } else {
                    0
                };
            }
            pixel[3] = (alpha * 255.0).round() as u8;
        }
    }
}

/// The application icon: the bird in ink on the violet square with rounded corners.
pub fn app(px: u32) -> IconData {
    draw(px, true, INK)
}

// --- the master geometry ----------------------------------------------------------------------
//
// `fuentes/khipu-simbolo-maestro.svg`, on its 40-unit canvas, transcribed point for point.

type Point = [f32; 2];
type Cubic = [Point; 4];

/// The body, head to tail and back, closed. The eye is a hole in it.
const BODY: [Cubic; 6] = [
    [[7.0, 24.0], [6.0, 18.0], [12.0, 19.0], [12.0, 14.0]],
    [[12.0, 14.0], [12.0, 7.0], [20.0, 6.0], [26.0, 9.0]],
    [[26.0, 9.0], [30.0, 11.0], [28.0, 20.0], [25.0, 23.0]],
    [[25.0, 23.0], [25.0, 28.0], [18.0, 31.0], [13.0, 29.0]],
    [[13.0, 29.0], [11.0, 33.0], [7.0, 32.0], [8.0, 27.0]],
    [[8.0, 27.0], [3.0, 25.0], [4.0, 20.0], [7.0, 24.0]],
];
const EYE: Point = [22.1, 14.1];
const BEAK: [Point; 3] = [[26.0, 17.0], [33.0, 18.0], [26.0, 21.0]];
/// The cord, a stroke with round ends, and the leg that stands on it.
const PERCH: Cubic = [[4.0, 32.0], [12.0, 35.0], [25.0, 31.0], [35.0, 32.0]];
const LEG: [Point; 2] = [[17.0, 29.0], [17.0, 33.0]];
const KNOTS: [Point; 2] = [[10.85, 33.95], [29.35, 31.85]];

/// The app icon's square (`fuentes/khipu-aplicacion.svg`): its corner, and where the bird sits on it.
const PLATE_RADIUS: f32 = 8.8;
const PLATE_SCALE: f32 = 0.83;
const PLATE_OFFSET: Point = [3.95, 3.49];

fn cubic(c: &Cubic, t: f32) -> Point {
    let u = 1.0 - t;
    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [
        a * c[0][0] + b * c[1][0] + cc * c[2][0] + d * c[3][0],
        a * c[0][1] + b * c[1][1] + cc * c[2][1] + d * c[3][1],
    ]
}

fn segment_distance_squared(x: f32, y: f32, a: Point, b: Point) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let t = (((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (x - a[0] - t * dx).powi(2) + (y - a[1] - t * dy).powi(2)
}

/// The bird at one size: its curves flattened, and the weights of that size. From 24 px down the
/// pack's optical variant applies (`khipu-simbolo-optico-16-24.svg`): a larger eye, a thicker cord
/// and larger knots, so they survive at a handful of pixels.
struct Bird {
    outline: Vec<Point>,
    perch: Vec<Point>,
    eye: f32,
    half_cord: f32,
    knot: f32,
}

impl Bird {
    fn new(px: u32) -> Self {
        let optical = px <= 24;
        let outline = BODY.iter()
            .flat_map(|c| (0..32).map(move |i| cubic(c, i as f32 / 32.0)))
            .collect();
        let perch = (0..=64).map(|i| cubic(&PERCH, i as f32 / 64.0)).collect();
        Self {
            outline,
            perch,
            eye: if optical { 2.45 } else { 2.1 },
            half_cord: if optical { 1.25 } else { 0.9 },
            knot: if optical { 2.6 } else { 2.35 },
        }
    }

    /// Where the body's outline crosses the row at `y`. Computed once per row of samples.
    fn crossings(&self, y: f32) -> Vec<f32> {
        let n = self.outline.len();
        (0..n)
            .filter_map(|i| {
                let (a, b) = (self.outline[i], self.outline[(i + 1) % n]);
                ((a[1] <= y) != (b[1] <= y)).then(|| a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]))
            })
            .collect()
    }

    fn covers(&self, x: f32, y: f32, crossings: &[f32]) -> bool {
        let inside_body = crossings.iter().filter(|&&c| c < x).count() % 2 == 1;
        if inside_body && (x - EYE[0]).powi(2) + (y - EYE[1]).powi(2) > self.eye * self.eye {
            return true;
        }
        let side = |a: Point, b: Point| (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
        let [p, q, r] = BEAK;
        let (s1, s2, s3) = (side(p, q), side(q, r), side(r, p));
        if (s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0) {
            return true;
        }
        if KNOTS.iter().any(|k| (x - k[0]).powi(2) + (y - k[1]).powi(2) <= self.knot * self.knot) {
            return true;
        }
        // The cord and the leg live between y 29 and 35: everything above skips the distances.
        if y < 29.0 - self.half_cord || y > 35.0 + self.half_cord {
            return false;
        }
        let reach = self.half_cord * self.half_cord;
        segment_distance_squared(x, y, LEG[0], LEG[1]) <= reach
            || self.perch.windows(2).any(|s| segment_distance_squared(x, y, s[0], s[1]) <= reach)
    }
}

fn in_plate(x: f32, y: f32) -> bool {
    let r = PLATE_RADIUS;
    let dx = x - x.clamp(r, 40.0 - r);
    let dy = y - y.clamp(r, 40.0 - r);
    (0.0..=40.0).contains(&x) && (0.0..=40.0).contains(&y) && dx * dx + dy * dy <= r * r
}

/// Many samples per pixel, so curves and the round ends stay round at fractional DPI sizes.
/// Colour accumulates unpremultiplied over covered samples only, which avoids dark fringes on
/// light backgrounds. No bitmap is ever scaled: each size is drawn from the curves.
fn draw(px: u32, plate: bool, ink: [u8; 3]) -> IconData {
    let px = px.clamp(8, 256);
    let n = if px <= 64 { 8 } else { 4 };
    let bird = Bird::new(px);
    let (scale, offset) = if plate { (PLATE_SCALE, PLATE_OFFSET) } else { (1.0, [0.0, 0.0]) };
    let step = 40.0 / (px * n) as f32;
    let mut cells = vec![[0u32; 4]; (px * px) as usize];
    for sy in 0..px * n {
        let y = (sy as f32 + 0.5) * step;
        let bird_y = (y - offset[1]) / scale;
        let crossings = bird.crossings(bird_y);
        for sx in 0..px * n {
            let x = (sx as f32 + 0.5) * step;
            if plate && !in_plate(x, y) {
                continue;
            }
            let colour = if bird.covers((x - offset[0]) / scale, bird_y, &crossings) {
                ink
            } else if plate {
                VIOLET
            } else {
                continue;
            };
            let cell = &mut cells[((sy / n) * px + sx / n) as usize];
            for i in 0..3 { cell[i] += colour[i] as u32; }
            cell[3] += 1;
        }
    }
    let samples = n * n;
    let mut rgba = Vec::with_capacity(cells.len() * 4);
    for [r, g, b, covered] in cells {
        match covered {
            0 => rgba.extend_from_slice(&[0, 0, 0, 0]),
            c => {
                for channel in [r, g, b] { rgba.push(((channel + c / 2) / c) as u8); }
                rgba.push(((c * 255 + samples / 2) / samples) as u8);
            }
        }
    }
    IconData { rgba, width: px, height: px }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(png: &[u8]) -> IconData {
        eframe::icon_data::from_png_bytes(png).unwrap()
    }

    /// The pack's own tray PNGs, every theme, state and size.
    const PACK_TRAY: [(&str, u32, &[u8]); 28] = {
        macro_rules! tray { ($($t:literal $s:literal $n:literal),*) => {
            [$((concat!($t, "-", $s), $n, include_bytes!(concat!("../assets/khipu/bandeja/khipu-", $t, "-", $s, "-", $n, ".png")) as &[u8])),*]
        } }
        tray!(
            "light" "active" 16, "light" "active" 20, "light" "active" 24, "light" "active" 32,
            "light" "active" 40, "light" "active" 48, "light" "active" 64,
            "light" "paused" 16, "light" "paused" 20, "light" "paused" 24, "light" "paused" 32,
            "light" "paused" 40, "light" "paused" 48, "light" "paused" 64,
            "dark" "active" 16, "dark" "active" 20, "dark" "active" 24, "dark" "active" 32,
            "dark" "active" 40, "dark" "active" 48, "dark" "active" 64,
            "dark" "paused" 16, "dark" "paused" 20, "dark" "paused" 24, "dark" "paused" 32,
            "dark" "paused" 40, "dark" "paused" 48, "dark" "paused" 64
        )
    };

    /// Measured on 2026-10-05: a mean of 0.11 to 1.65 per channel and at most 42 on any single
    /// edge pixel, with total ink within 0.6 % at every size. The pack resamples with another
    /// filter, so only the anti-aliasing of the edges differs; a moved or missing part would be
    /// far over either limit.
    const LIMIT_MEAN: f64 = 2.0;
    const LIMIT_MAX: u8 = 48;

    /// Mean and largest difference per channel, over premultiplied colour and alpha, so a
    /// colour hidden by zero alpha doesn't count.
    fn difference(a: &IconData, b: &IconData) -> (f64, u8) {
        assert_eq!((a.width, a.height), (b.width, b.height));
        let premultiplied = |p: &[u8]| [0, 1, 2].map(|i| (p[i] as u32 * p[3] as u32 / 255) as u8).into_iter().chain([p[3]]);
        let (mut sum, mut max) = (0u64, 0u8);
        for (p, q) in a.rgba.chunks(4).zip(b.rgba.chunks(4)) {
            for (u, v) in premultiplied(p).zip(premultiplied(q)) {
                let d = u.abs_diff(v);
                sum += d as u64;
                max = max.max(d);
            }
        }
        (sum as f64 / a.rgba.len() as f64, max)
    }

    #[test]
    fn the_tray_matches_the_identity_pack() {
        for (name, size, png) in PACK_TRAY {
            let tones = if name.starts_with("light") { Tones::LIGHT } else { Tones::DARK };
            let colour = if name.ends_with("paused") { tones.paused } else { tones.active };
            let (mean, max) = difference(&symbol(size, colour), &decode(png));
            assert!(mean < LIMIT_MEAN && max <= LIMIT_MAX, "{name} {size}: mean {mean:.3}, max {max}");
        }
    }

    #[test]
    fn the_app_icon_matches_the_identity_pack() {
        // The .ico's entries, read straight out of the file: every one is a PNG.
        let ico = include_bytes!("../assets/khipu/app/Khipu.ico");
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        assert_eq!(count, 9);
        for i in 0..count {
            let entry = &ico[6 + 16 * i..];
            let size = if entry[0] == 0 { 256 } else { entry[0] as u32 };
            let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            let (mean, max) = difference(&app(size), &decode(&ico[offset..offset + length]));
            assert!(mean < LIMIT_MEAN && max <= LIMIT_MAX, "{size}: mean {mean:.3}, max {max}");
        }
    }

    #[test]
    fn the_news_dot_sits_in_the_empty_corner() {
        for size in [16, 20, 24, 28, 32, 40, 48, 64] {
            let plain = symbol(size, Tones::DARK.active);
            let mut news = plain.clone();
            badge(&mut news, [0x4C, 0xC2, 0xFF]);
            // The dot's centre is the accent, solid.
            let c = |p: Point| ((p[1] / 40.0 * size as f32) as u32 * size + (p[0] / 40.0 * size as f32) as u32) as usize * 4;
            assert_eq!(&news.rgba[c(DOT_CENTRE)..][..4], &[0x4C, 0xC2, 0xFF, 0xFF], "{size}");
            // Away from the corner nothing moved: the left half and the bottom half are the bird's.
            for (i, (a, b)) in plain.rgba.chunks(4).zip(news.rgba.chunks(4)).enumerate() {
                let (x, y) = (i as u32 % size, i as u32 / size);
                if x < size / 2 || y > size / 2 {
                    assert_eq!(a, b, "{size} at {x},{y}");
                }
            }
            // And the bird lost almost nothing to the ring: the corner is empty space.
            let ink: u32 = plain.rgba.chunks(4).map(|p| p[3] as u32).sum();
            let lost: u32 = plain.rgba.chunks(4).zip(news.rgba.chunks(4)).map(|(a, b)| a[3].saturating_sub(b[3]) as u32).sum();
            assert!(lost * 50 < ink, "{size}: lost {lost} of {ink}");
        }
    }

    #[test]
    fn paused_keeps_the_silhouette_at_every_size() {
        for size in [16, 20, 24, 28, 32, 40, 48, 56, 64] {
            for tones in [Tones::LIGHT, Tones::DARK] {
                let active = symbol(size, tones.active);
                let paused = symbol(size, tones.paused);
                assert_eq!(active.rgba.len(), (size * size * 4) as usize);
                assert_eq!(&active.rgba[..4], &[0, 0, 0, 0]);
                assert_ne!(active.rgba, paused.rgba);
                assert!(active.rgba.chunks(4).zip(paused.rgba.chunks(4)).all(|(a, b)| a[3] == b[3]));
            }
        }
    }
}
