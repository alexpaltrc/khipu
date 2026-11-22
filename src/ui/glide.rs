/*
 * This file is part of Khipu.
 *
 * Copyright (C) 2026 Alex Palacios
 *
 * Khipu is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Khipu is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Khipu.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Fluid hover: one wash per group of controls, gliding to whichever of them the pointer is on.
//!
//! Controls that stand side by side — the lines of a flyout, the segments of a switch, the
//! borderless buttons of the command bar — do not light up and go dark one by one. The group owns
//! a single wash that fades in where the pointer first lands, glides from control to control while
//! the pointer stays among them, and fades out in place when it leaves. With Windows' animation
//! effects off it neither fades nor travels: it is simply on the control under the pointer.
//!
//! **A group is the `Ui` its controls are placed in**, so nothing has to be declared: the lines of
//! one popup, the segments of one track, the buttons of one row share one. The first control of a
//! group drawn each frame moves the wash and paints it *before* painting itself, which is what puts
//! the wash under every control of the group. It moves towards where the pointer was last frame —
//! the group cannot know this frame's answer until its last control has been drawn — so the wash
//! is one frame behind the pointer, which at 60 frames a second nobody sees.
//!
//! Positions are kept relative to the group's first control, so a group that moves as a whole — a
//! popup opening, a dialog sliding, a scroll — carries its wash with it instead of leaving it to
//! catch up.
//!
//! The library's list has its own wash, for its own reason — see `list_view`'s `Glide` — and
//! borrows only the spring from here.
//!
//! What does **not** glide: outlined buttons and chips. Their fill is part of their shape, and a
//! wash sliding between two outlined boxes reads as something coming loose.

use std::collections::HashMap;

/// How quickly a wash reaches its target: a critically damped spring, so it arrives without
/// bouncing. At 34 it covers nine tenths of the way in about 0.11 s.
const OMEGA: f32 = 34.0;
/// On a trip, the edge in front and the edge behind: the front one leaves sooner and the back one
/// follows, so a wash on its way stretches towards where it is going and gathers itself as it
/// arrives, like something liquid being drawn along rather than a box sliding. On the hop to the
/// next row the stretch is a few pixels; on a fast sweep across a menu it is the movement itself.
const OMEGA_FRONT: f32 = 44.0;
const OMEGA_BACK: f32 = 26.0;
/// The longest a wash may draw out on its way, as a share of where it is going: past this, the
/// edge behind is pulled along, so a very fast sweep never turns into a long smear.
const STRETCH: f32 = 0.6;
/// How long a wash takes to appear, and to go.
const FADE_IN: f32 = 0.10;
const FADE_OUT: f32 = 0.16;
/// How long a wash waits where it is after the pointer leaves its control, before it fades: long
/// enough to cross the gap to the next control without going out and coming back, too short to be
/// seen as a delay when the pointer leaves for good.
const LINGER: f32 = 0.07;
/// Coming back while fainter than this, a wash lands where it is going instead of travelling: a
/// ghost of a wash crossing the whole menu to catch up reads as a glitch, not a glide.
const LAND_BELOW: f32 = 0.35;

/// One step of a critically damped spring, exact for any `dt`: `x` is the distance still to go,
/// `v` the speed. Returns both after `dt` seconds.
pub fn spring_step(x: f32, v: f32, dt: f32) -> (f32, f32) {
    spring_step_at(OMEGA, x, v, dt)
}

fn spring_step_at(omega: f32, x: f32, v: f32, dt: f32) -> (f32, f32) {
    let decay = (-omega * dt).exp();
    let b = v + omega * x;
    ((x + b * dt) * decay, (v - omega * b * dt) * decay)
}

/// Moves one axis of a wash, `[low edge, high edge]`, a frame towards `to`, the edge in front
/// faster than the one behind.
fn chase_span(span: [f32; 2], speed: &mut [f32; 2], to: [f32; 2], dt: f32) -> [f32; 2] {
    let ahead = (to[0] + to[1]) - (span[0] + span[1]);
    let (low, high) = if ahead > 1.0 {
        (OMEGA_BACK, OMEGA_FRONT)
    } else if ahead < -1.0 {
        (OMEGA_FRONT, OMEGA_BACK)
    } else {
        (OMEGA, OMEGA)
    };
    let (x0, v0) = spring_step_at(low, span[0] - to[0], speed[0], dt);
    let (x1, v1) = spring_step_at(high, span[1] - to[1], speed[1], dt);
    let (mut lo, mut hi) = (to[0] + x0, to[1] + x1);
    *speed = [v0, v1];
    let longest = (to[1] - to[0]).max(0.0) * (1.0 + STRETCH);
    if hi - lo > longest {
        if ahead > 0.0 {
            lo = hi - longest;
            speed[0] = speed[1];
        } else {
            hi = lo + longest;
            speed[1] = speed[0];
        }
    }
    [lo, hi.max(lo)]
}

/// How one wash moves: where it is, how fast each edge travels, and how lit it is. The groups
/// below and the library's list both move theirs with it, so every list in Khipu feels the same.
#[derive(Clone, Copy)]
pub struct Wash {
    pub rect: egui::Rect,
    /// Left, right, top, bottom.
    speed: [f32; 4],
    /// 0 is gone, 1 is fully lit.
    pub alpha: f32,
    lingered: f32,
}

impl Default for Wash {
    fn default() -> Self {
        Self { rect: egui::Rect::NOTHING, speed: [0.0; 4], alpha: 0.0, lingered: 0.0 }
    }
}

impl Wash {
    /// Moves the wash a frame towards `target`, or towards going out when nothing is lit. Returns
    /// whether it is still on its way and needs another frame.
    pub fn step(&mut self, target: Option<egui::Rect>, dt: f32) -> bool {
        let animate = crate::theme::animations_enabled();
        match target {
            Some(to) => {
                self.lingered = 0.0;
                if self.alpha < LAND_BELOW || !animate {
                    self.rect = to;
                    self.speed = [0.0; 4];
                } else {
                    let (mut sx, mut sy) = ([self.speed[0], self.speed[1]], [self.speed[2], self.speed[3]]);
                    let [l, r] = chase_span([self.rect.left(), self.rect.right()], &mut sx, [to.left(), to.right()], dt);
                    let [t, b] = chase_span([self.rect.top(), self.rect.bottom()], &mut sy, [to.top(), to.bottom()], dt);
                    self.rect = egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b));
                    self.speed = [sx[0], sx[1], sy[0], sy[1]];
                }
                self.alpha = if animate { (self.alpha + dt / FADE_IN).min(1.0) } else { 1.0 };
                let moving = (self.rect.min - to.min).length() > 0.1
                    || (self.rect.max - to.max).length() > 0.1
                    || self.speed.iter().any(|s| s.abs() > 0.5);
                moving || self.alpha < 1.0
            }
            None if animate && self.alpha > 0.0 && self.lingered < LINGER => {
                self.lingered += dt;
                true
            }
            None => {
                self.alpha = if animate { (self.alpha - dt / FADE_OUT).max(0.0) } else { 0.0 };
                self.alpha > 0.0
            }
        }
    }

    /// Gone at once, as for a menu that was closed: it must never fly in from where it was left.
    pub fn forget(&mut self) {
        *self = Self::default();
    }
}

/// What a control said about itself last frame: where it is, relative to its group's first
/// control, and how to wash it. Pointer beats keyboard: the control under the pointer wins over
/// the one Tab is on.
#[derive(Clone, Copy)]
struct Target {
    rect: egui::Rect,
    fill: egui::Color32,
    radius: u8,
    by_pointer: bool,
}

#[derive(Clone, Copy)]
struct Group {
    /// The wash, relative to the group's first control.
    wash: Wash,
    fill: egui::Color32,
    radius: u8,
    /// Where this frame's controls are measured from, and which frame that is.
    origin: egui::Pos2,
    frame: u64,
    /// What the controls reported last frame, which this frame moves towards, and what they are
    /// reporting now, for the next.
    target: Option<Target>,
    reporting: Option<Target>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            wash: Wash::default(),
            fill: egui::Color32::TRANSPARENT,
            radius: 0,
            origin: egui::Pos2::ZERO,
            frame: 0,
            target: None,
            reporting: None,
        }
    }
}

#[derive(Clone, Default)]
struct Groups(HashMap<egui::Id, Group>);

fn groups_key() -> egui::Id {
    egui::Id::new("glide-groups")
}

/// The call a control makes where it would have painted its own hover fill: `fill` is what it
/// would have painted, `lit` whether it would have painted it — under the pointer, pressed, or
/// with the keyboard on it.
pub fn item(ui: &egui::Ui, response: &egui::Response, radius: u8, lit: bool, fill: egui::Color32) {
    let ctx = ui.ctx();
    let id = egui::Id::new("glide").with(ui.id());
    let frame = ctx.cumulative_pass_nr();
    let rect = response.rect;
    let by_pointer = response.hovered() || response.is_pointer_button_down_on();
    // Everything that asks the context something is asked here, outside `data_mut`: the context
    // is one lock, and asking it anything from inside that closure waits on itself forever.
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 1.0 / 20.0);

    let (paint, busy) = ctx.data_mut(|d| {
        let groups = d.get_temp_mut_or_default::<Groups>(groups_key());
        let g = groups.0.entry(id).or_default();
        let mut paint = (None, false);
        if g.frame != frame {
            // The first control of the group this frame: last frame's reports become the target.
            if g.frame + 1 != frame {
                // Not drawn last frame — a menu that was closed. Whatever it held is stale.
                g.wash.forget();
                g.reporting = None;
            }
            g.target = g.reporting.take();
            g.origin = rect.min;
            g.frame = frame;
            paint = step(g, dt);
        }
        if lit {
            let offered = Target {
                rect: rect.translate(-g.origin.to_vec2()),
                fill,
                radius,
                by_pointer,
            };
            if g.reporting.is_none_or(|t| offered.by_pointer || !t.by_pointer) {
                g.reporting = Some(offered);
            }
        }
        paint
    });

    if busy {
        ctx.request_repaint();
    }
    if let Some((rect, fill, radius)) = paint {
        ui.painter().rect_filled(rect, radius, fill);
    }
}

/// Moves one group's wash a frame towards its target. Returns what to paint, in screen space, and
/// whether it is still on its way and needs another frame.
fn step(g: &mut Group, dt: f32) -> (Option<(egui::Rect, egui::Color32, u8)>, bool) {
    if let Some(t) = g.target {
        g.fill = t.fill;
        g.radius = t.radius;
    }
    let busy = g.wash.step(g.target.map(|t| t.rect), dt);
    let paint = (g.wash.alpha > 0.0).then(|| {
        (
            g.wash.rect.translate(g.origin.to_vec2()),
            g.fill.gamma_multiply(g.wash.alpha),
            g.radius,
        )
    });
    (paint, busy)
}

/// Called once, after everything has been drawn. A group whose controls just reported something
/// other than what its wash is heading for needs one more frame to act on it — the pointer
/// leaving the window is a single event, and without this the wash would stay lit until the next
/// one. Groups not drawn for a while are forgotten.
pub fn end_frame(ctx: &egui::Context) {
    let frame = ctx.cumulative_pass_nr();
    let changed = ctx.data_mut(|d| {
        let groups = d.get_temp_mut_or_default::<Groups>(groups_key());
        groups.0.retain(|_, g| g.frame + 120 > frame);
        groups.0.values().any(|g| {
            g.frame == frame
                && g.target.map(|t| (t.rect, t.fill)) != g.reporting.map(|t| (t.rect, t.fill))
        })
    });
    if changed {
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs one axis from `from` to `to` at 60 frames a second, and returns every frame's span.
    fn trip(from: [f32; 2], to: [f32; 2], frames: usize) -> Vec<[f32; 2]> {
        let (mut span, mut speed) = (from, [0.0; 2]);
        (0..frames).map(|_| { span = chase_span(span, &mut speed, to, 1.0 / 60.0); span }).collect()
    }

    #[test]
    fn a_wash_on_its_way_leans_forward_and_never_smears() {
        // A sweep from the first row to the twentieth, 36 px each.
        let frames = trip([0.0, 36.0], [684.0, 720.0], 40);
        assert!(frames.iter().any(|s| s[1] - s[0] > 37.0), "it never stretched");
        for s in &frames {
            assert!(s[1] - s[0] <= 36.0 * (1.0 + STRETCH) + 0.01, "it smeared to {}", s[1] - s[0]);
            assert!(s[1] >= s[0]);
        }
    }

    #[test]
    fn a_wash_arrives_without_overshooting() {
        for (from, to) in [([0.0, 36.0], [36.0, 72.0]), ([684.0, 720.0], [0.0, 36.0])] {
            let frames = trip(from, to, 30);
            let down = to[0] > from[0];
            for s in &frames {
                if down {
                    assert!(s[1] <= to[1] + 0.01 && s[0] <= to[0] + 0.01);
                } else {
                    assert!(s[0] >= to[0] - 0.01 && s[1] >= to[1] - 0.01);
                }
            }
            let last = frames.last().unwrap();
            assert!((last[0] - to[0]).abs() < 0.1 && (last[1] - to[1]).abs() < 0.1, "still on its way: {last:?}");
        }
    }
}
