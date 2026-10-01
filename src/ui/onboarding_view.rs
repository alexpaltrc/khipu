/*
 * This file is part of Espanso Manager.
 *
 * Copyright (C) 2026 Alex Palacios
 *
 * Espanso Manager is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Espanso Manager is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Espanso Manager.  If not, see <https://www.gnu.org/licenses/>.
 */

//! The first-run screen: four short steps, one idea each, that never appear again.
//!
//! 1. **Welcome** — the mark, the name, the version. Nothing to read yet.
//! 2. **What an expansion is** — shown, not described: three triggers typed and turned into text.
//! 3. **Try it** — a real field, and a trial expansion espanso really expands. Nothing simulated.
//! 4. **Start with Windows**, and where the icon lives once the window is closed.
//!
//! Under each step, one small line that answers a question nobody has asked yet (is it free, does
//! it cost memory, can I pause it, can I change this later).
//!
//! Espanso's own wizard is suppressed rather than raced — see `main.rs`, which writes the flags it
//! looks for before the daemon ever starts.
//!
//! **The way forward is always on screen.** The buttons and the tip are laid out first, against the
//! bottom edge; the step is centred in what is left and scrolls only if the window is too short for
//! it. **Every animation is drawn here, by the painter** — no image, so nothing blurs on a scaled
//! display or goes stale in the other theme — and asks for frames only while something moves. With
//! Windows' animation effects off, nothing moves: each step appears whole, and the two animated
//! pictures are replaced by still ones that say the same thing.

use crate::app::{AppState, View};
use crate::ui::controls::{self, Tone};
use egui::{pos2, vec2, Color32, Rect, Stroke};

/// What the trial expansion produces. Espanso's own wording, kept deliberately: someone who later
/// reads espanso's documentation should meet the same example.
pub const PROBE_TRIGGER: &str = ":espanso";
pub const PROBE_REPLACE: &str = "Hi there!";

const STEPS: usize = 4;
/// The column every step keeps to. Narrow on purpose: four short steps read as one calm page, not
/// as a form stretched across a wide window.
const COLUMN_MAX_WIDTH: f32 = 560.0;
/// The pictures and the one control sit in a narrower measure still, centred under the text.
const FIGURE_WIDTH: f32 = 440.0;
/// The tip under each step may run wider than the column; see [`tip_line`].
const TIP_MAX_WIDTH: f32 = 680.0;

/// How a step arrives: each block fades in and slides the last few points into place, one shortly
/// after the other, from the side the user is travelling towards.
const ENTER_SECONDS: f64 = 0.32;
const ENTER_STAGGER: f64 = 0.07;
const ENTER_SLIDE: f32 = 14.0;

/// Where the flow stands. View state, not app state: nothing about it outlives the screen.
#[derive(Clone, Copy)]
struct Flow {
    step: usize,
    /// When the current step arrived, on egui's clock.
    since: f64,
    forward: bool,
    /// The trial field takes the keyboard once, the frame its step arrives.
    focus_pending: bool,
}

fn flow_id() -> egui::Id {
    egui::Id::new("onboarding-flow")
}

/// Collects when the screen next needs a frame. Everything here that moves says how soon it will
/// next change; the soonest wins, and a still screen asks for nothing at all.
struct Frames(Option<f64>);

impl Frames {
    fn now(&mut self) {
        self.0 = Some(0.0);
    }
    fn after(&mut self, seconds: f64) {
        let seconds = seconds.max(0.0);
        self.0 = Some(self.0.map_or(seconds, |s| s.min(seconds)));
    }
    fn request(self, ctx: &egui::Context) {
        match self.0 {
            Some(s) if s <= 1.0 / 240.0 => ctx.request_repaint(),
            Some(s) => ctx.request_repaint_after(std::time::Duration::from_secs_f64(s)),
            None => {}
        }
    }
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let animate = crate::theme::animations_enabled();
    let mut flow = ctx.data(|d| d.get_temp::<Flow>(flow_id())).unwrap_or(Flow {
        step: 0,
        since: now,
        forward: true,
        focus_pending: false,
    });
    let mut frames = Frames(None);
    let elapsed = if animate { now - flow.since } else { f64::INFINITY };
    let enter = Enter { elapsed, forward: flow.forward };

    let full = ui.available_width();
    let width = full.min(COLUMN_MAX_WIDTH);
    let left = ui.max_rect().left() + ((full - width) * 0.5).max(0.0);

    let last = flow.step + 1 == STEPS;
    let mut go: Option<usize> = None;
    let mut finishing = false;
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        // --- the way forward -------------------------------------------------------------------
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), controls::FIELD_HEIGHT), egui::Sense::hover());
        let row = Rect::from_min_size(pos2(left, row.top()), vec2(width, row.height()));
        if flow.step > 0 {
            let mut back = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(row)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            if controls::button(&mut back, t.back, Tone::Normal, true).clicked() {
                go = Some(flow.step - 1);
            }
        }
        let mut next = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        let label = if last { t.onboarding_start } else { t.onboarding_next };
        if controls::primary_button(&mut next, label).clicked() {
            if last {
                finishing = true;
            } else {
                go = Some(flow.step + 1);
            }
        }
        step_dots(ui, row.center(), flow.step, animate, &mut frames);

        // --- the tip, one small line just above ------------------------------------------------
        ui.add_space(controls::GAP_STACK + controls::GAP_TIGHT);
        let tip = match flow.step {
            0 => t.onboarding_welcome_tip.to_owned(),
            1 => t.onboarding_what_tip.to_owned(),
            2 => t.onboarding_try_tip.to_owned(),
            _ => crate::i18n::fill(t.onboarding_autostart_note, &[("settings", t.settings_title)]),
        };
        tip_line(ui, &tip, left, width, &enter, &mut frames);
        ui.add_space(controls::GAP_SECTION);

        // --- the step, centred in what is left ---------------------------------------------------
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            let viewport = ui.available_height();
            controls::page_scroll(ui, ("onboarding", flow.step), |ui| {
                let full = ui.available_width();
                let width = full.min(COLUMN_MAX_WIDTH);
                // Measured on the frame before, and that frame thrown away whenever the measure
                // changes, so a step never shows for even one frame anywhere but in the middle.
                let key = egui::Id::new("onboarding-height").with(flow.step);
                let known = ctx.data(|d| d.get_temp::<f32>(key)).unwrap_or(0.0);
                // A shade above the true middle, where the eye takes the middle to be.
                ui.add_space(((viewport - known) * 0.44).max(0.0));
                let x = ui.max_rect().left() + ((full - width) * 0.5).max(0.0);
                let area = Rect::from_min_size(pos2(x, ui.cursor().top()), vec2(width, f32::INFINITY));
                let drawn = ui
                    .scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(area)
                            .layout(egui::Layout::top_down(egui::Align::Center)),
                        |ui| {
                            ui.set_width(width);
                            match flow.step {
                                0 => welcome(ui, state, &enter),
                                1 => what(ui, state, &enter, &mut frames),
                                2 => try_it(ui, state, &enter, &mut flow.focus_pending),
                                _ => autostart(ui, state, &enter, viewport, &mut frames),
                            }
                        },
                    )
                    .response
                    .rect
                    .height();
                if (drawn - known).abs() > 0.5 {
                    ctx.data_mut(|d| d.insert_temp(key, drawn));
                    ctx.request_discard("onboarding: centring the step");
                }
            });
        });
    });

    if enter.moving() {
        frames.now();
    }
    if let Some(step) = go {
        flow = Flow { step, since: now, forward: step > flow.step, focus_pending: step == 2 };
    }
    ctx.data_mut(|d| d.insert_temp(flow_id(), flow));
    frames.request(&ctx);

    if finishing {
        ctx.data_mut(|d| d.remove::<Flow>(flow_id()));
        state.finish_onboarding();
        state.view = View::List;
    }
}

// --- arriving -------------------------------------------------------------------------------

/// How far into its arrival the current step is.
struct Enter {
    elapsed: f64,
    forward: bool,
}

impl Enter {
    /// 0 → 1 for the `order`-th block of the step.
    fn amount(&self, order: usize) -> f32 {
        let local = (self.elapsed - order as f64 * ENTER_STAGGER) / ENTER_SECONDS;
        ease_out(local.clamp(0.0, 1.0) as f32)
    }

    fn moving(&self) -> bool {
        self.elapsed < ENTER_SECONDS + 4.0 * ENTER_STAGGER
    }

    /// Draws `add` faded and shifted by how far its block has arrived. Only what is *painted*
    /// moves; the layout stays put, so nothing below jumps while it settles.
    fn reveal<R>(&self, ui: &mut egui::Ui, order: usize, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        // Wrapped the same way when it has arrived as while it arrives: the wrapper is a child Ui,
        // and taking it away would renumber every widget inside — the field would lose its focus
        // the moment it stopped moving.
        let a = self.amount(order);
        let dx = (1.0 - a) * ENTER_SLIDE * if self.forward { 1.0 } else { -1.0 };
        ui.with_visual_transform(egui::emath::TSTransform::from_translation(vec2(dx, 0.0)), |ui| {
            ui.multiply_opacity(a);
            add(ui)
        })
        .inner
    }
}

fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

fn ease_in_out(x: f32) -> f32 {
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) * 0.5 }
}

/// Where `t` stands between `a` and `b`, as 0 → 1.
fn span(t: f64, a: f64, b: f64) -> f32 {
    ((t - a) / (b - a)).clamp(0.0, 1.0) as f32
}

// --- the frame around every step ------------------------------------------------------------

/// Four dots, the current one drawn out into a short bar. Says how long this is, and how far in.
fn step_dots(ui: &egui::Ui, centre: egui::Pos2, step: usize, animate: bool, frames: &mut Frames) {
    const DOT: f32 = 6.0;
    const BAR: f32 = 20.0;
    const SPACE: f32 = 6.0;
    let is_light = !ui.visuals().dark_mode;
    let accent = crate::app::accent(ui.visuals());
    let rest = crate::app::line_strong(is_light);
    let time = if animate { 0.25 } else { 0.0 };
    let widths: Vec<f32> = (0..STEPS)
        .map(|i| {
            let target = if i == step { BAR } else { DOT };
            let w = ui.ctx().animate_value_with_time(flow_id().with(("dot", i)), target, time);
            if (w - target).abs() > 0.01 {
                frames.now();
            }
            w
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + SPACE * (STEPS - 1) as f32;
    let mut x = centre.x - total * 0.5;
    for w in widths {
        // The colour follows the width, so the bar fills in as it grows rather than switching.
        let lit = ((w - DOT) / (BAR - DOT)).clamp(0.0, 1.0);
        let colour = crate::app::mix(rest, accent, lit);
        let rect = Rect::from_min_size(pos2(x, centre.y - DOT * 0.5), vec2(w, DOT));
        ui.painter().rect_filled(rect, DOT * 0.5, colour);
        x += w + SPACE;
    }
}

/// The small line under each step: centred, a size down, in the supporting grey.
fn tip_line(ui: &mut egui::Ui, text: &str, left: f32, width: f32, enter: &Enter, frames: &mut Frames) {
    let is_light = !ui.visuals().dark_mode;
    // Small type runs long for its size, so it gets more room than the column before it wraps:
    // one line reads as an aside, two start to read as a paragraph.
    let wrap = ui.available_width().min(TIP_MAX_WIDTH).max(width);
    let layout = |wrap: f32| {
        let mut job = egui::text::LayoutJob::simple(
            text.to_owned(),
            egui::TextStyle::Small.resolve(ui.style()),
            crate::app::secondary_text(is_light),
            wrap,
        );
        job.halign = egui::Align::Center;
        ui.painter().layout_job(job)
    };
    let mut galley = layout(wrap);
    // When it has to wrap, its lines are evened out: the narrowest measure that still takes no
    // more lines, so a lone last word is never left hanging under a full line.
    let rows = galley.rows.len();
    if rows > 1 {
        let (mut fits, mut short) = (wrap, wrap / rows as f32);
        while fits - short > 4.0 {
            let mid = (fits + short) * 0.5;
            if layout(mid).rows.len() <= rows { fits = mid } else { short = mid }
        }
        galley = layout(fits);
    }
    let (slot, _) = ui.allocate_exact_size(vec2(ui.available_width(), galley.size().y), egui::Sense::hover());
    let a = enter.amount(3);
    if a < 1.0 {
        frames.now();
    }
    let dx = (1.0 - a) * ENTER_SLIDE * if enter.forward { 1.0 } else { -1.0 };
    let mut painter = ui.painter().clone();
    painter.set_opacity(a);
    painter.galley(pos2(left + width * 0.5 + dx, slot.top()), galley, Color32::PLACEHOLDER);
}

fn title(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(controls::h1(text)).wrap());
}

fn lead(ui: &mut egui::Ui, text: &str) {
    let is_light = !ui.visuals().dark_mode;
    ui.add(egui::Label::new(controls::muted(text, is_light)).wrap());
}

/// A fixed-width block centred in the column, for the pictures and the switch.
fn figure<R>(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let width = width.min(ui.available_width());
    let free = ui.available_rect_before_wrap();
    let rect = Rect::from_min_size(
        pos2(free.center().x - width * 0.5, free.top()),
        vec2(width, free.height()),
    );
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_width(width);
            add(ui)
        },
    )
    .inner
}

// --- 1. welcome -----------------------------------------------------------------------------

fn welcome(ui: &mut egui::Ui, state: &AppState, enter: &Enter) {
    let is_light = !ui.visuals().dark_mode;
    enter.reveal(ui, 0, |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(LOGO), egui::Sense::hover());
        paint_mark(ui.painter(), ui.visuals(), rect, true);
    });
    ui.add_space(controls::GAP_STACK + controls::GAP);
    enter.reveal(ui, 1, |ui| {
        ui.add(egui::Label::new(controls::semibold("Espanso Manager").size(30.0)).wrap());
    });
    ui.add_space(controls::GAP_TIGHT);
    enter.reveal(ui, 2, |ui| {
        let version = crate::i18n::fill(
            state.t().onboarding_version,
            &[("version", env!("CARGO_PKG_VERSION"))],
        );
        ui.add(egui::Label::new(controls::muted(version, is_light)).wrap());
    });
}

/// The side of the mark on the welcome step.
const LOGO: f32 = 96.0;

/// The app's mark — the keycap with a colon and a chevron — drawn from the same 16-unit geometry
/// as the icon in [`crate::icons`], so the welcome and the tray show one and the same thing. Drawn
/// as shapes at whatever size it is asked for, so it is as sharp at 96 points as at 16.
fn paint_mark(painter: &egui::Painter, visuals: &egui::Visuals, rect: Rect, shadow: bool) {
    let is_light = !visuals.dark_mode;
    let (face, mark, edge) = if crate::app::contrast().is_some() {
        let (fill, ink) = crate::app::accent_fill(visuals);
        (fill, ink, ink)
    } else {
        let p = if is_light { crate::icons::Palette::LIGHT } else { crate::icons::Palette::DARK };
        let c = |v: [u8; 3]| Color32::from_rgb(v[0], v[1], v[2]);
        (c(p.face), c(p.mark), c(p.edge))
    };
    let k = rect.width() / 16.0;
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * k;
    let square = |a: f32, b: f32| Rect::from_min_max(at(a, a), at(b, b));
    if shadow {
        let depth = egui::epaint::Shadow {
            offset: [0, (k * 1.2) as i8],
            blur: (k * 3.0).min(60.0) as u8,
            spread: 0,
            color: Color32::from_black_alpha(if is_light { 46 } else { 90 }),
        };
        painter.add(depth.as_shape(square(0.25, 15.75), 3.25 * k));
    }
    painter.rect_filled(square(0.25, 15.75), 3.25 * k, edge);
    painter.rect_filled(square(0.75, 15.25), 2.75 * k, face);
    painter.circle_filled(at(5.0, 5.25), 1.3 * k, mark);
    painter.circle_filled(at(5.0, 10.75), 1.3 * k, mark);
    // The chevron: a stroke with round ends and a round corner, as the icon's distance field has.
    let chevron = [at(8.6, 4.6), at(12.0, 8.0), at(8.6, 11.4)];
    let width = 1.9 * k;
    painter.add(egui::Shape::line(chevron.to_vec(), Stroke::new(width, mark)));
    for p in chevron {
        painter.circle_filled(p, width * 0.5, mark);
    }
}

// --- 2. what an expansion is ----------------------------------------------------------------

/// One of the three things the demonstration types.
struct Example {
    /// What is already on the page before the trigger is typed.
    before: &'static str,
    trigger: &'static str,
    result: String,
}

fn examples(state: &AppState) -> [Example; 3] {
    use crate::ui::studio::text;
    let lang = state.settings.lang;
    // Today, in the friendliest of the formats: the month spelled out, as a person writes it.
    let mut date = crate::datefmt::format("%B %d, %Y", crate::datefmt::Now::local(), lang);
    if let Some(first) = date.chars().next() {
        date = first.to_uppercase().chain(date.chars().skip(1)).collect();
    }
    [
        Example {
            before: "",
            trigger: text(state, ":hola", ":hi", ":kumusta", ":namaste"),
            result: text(
                state,
                "¡Hola! ¿Cómo estás?",
                "Hi! How are you?",
                "Kumusta! Ano'ng balita?",
                "नमस्ते! आप कैसे हैं?",
            )
            .to_owned(),
        },
        Example {
            before: text(state, "Fecha de entrega: ", "Delivery date: ", "Petsa ng paghahatid: ", "डिलीवरी की तारीख़: "),
            trigger: text(state, ":fecha", ":date", ":petsa", ":tarikh"),
            result: date,
        },
        Example {
            before: text(
                state,
                "Gracias por tu ayuda con el informe.\n\n",
                "Thanks for your help with the report.\n\n",
                "Salamat sa tulong mo sa ulat.\n\n",
                "रिपोर्ट में मदद के लिए धन्यवाद।\n\n",
            ),
            trigger: text(state, ":firma", ":sig", ":pirma", ":sign"),
            result: text(
                state,
                "Un saludo,\nAna Torres\nEquipo de Ventas",
                "Best regards,\nAna Torres\nSales team",
                "Lubos na gumagalang,\nAna Torres\nSales team",
                "सादर,\nअना टोरेस\nबिक्री टीम",
            )
            .to_owned(),
        },
    ]
}

fn what(ui: &mut egui::Ui, state: &AppState, enter: &Enter, frames: &mut Frames) {
    let t = state.t();
    enter.reveal(ui, 0, |ui| title(ui, t.onboarding_what_title));
    ui.add_space(controls::GAP_ROW);
    enter.reveal(ui, 1, |ui| lead(ui, t.onboarding_what_body));
    ui.add_space(controls::GAP_STACK + controls::GAP_ROW);
    let examples = examples(state);
    enter.reveal(ui, 2, |ui| {
        figure(ui, FIGURE_WIDTH, |ui| {
            if crate::theme::animations_enabled() {
                typing_demo(ui, &examples, enter.elapsed - ENTER_SECONDS, frames);
            } else {
                still_examples(ui, &examples);
            }
        });
    });
}

/// Timing of one scene of the demonstration, in seconds from its start.
const SCENE: f64 = 4.4;
const TYPE_FROM: f64 = 0.45;
const PER_KEY: f64 = 0.085;
const BEAT: f64 = 0.45;
const FADE_OUT: f64 = 0.35;
const BLINK: f64 = 0.53;
const SURFACE_PAD: f32 = 18.0;

/// A page that types each trigger, waits a beat, and has it become the text — over and over, one
/// example after another. The text that arrives is lit in the accent for a moment, which is the
/// one thing the eye should follow, and the caret blinks where a caret would.
fn typing_demo(ui: &mut egui::Ui, examples: &[Example; 3], clock: f64, frames: &mut Frames) {
    let is_light = !ui.visuals().dark_mode;
    let accent = crate::app::accent(ui.visuals());
    let ink = ui.visuals().text_color();
    let quiet = crate::app::secondary_text(is_light);
    let card = crate::app::win_card_for(is_light);
    let body = egui::TextStyle::Body.resolve(ui.style());
    let mono = egui::FontId::monospace(body.size);
    let width = ui.available_width();
    let inner = width - 2.0 * SURFACE_PAD;
    let typesetter = ui.painter().clone();

    // The trigger is not laid out in the same job as the page around it: a row that mixes the two
    // faces lines them up by their bottoms, and Cascadia's baseline is not Segoe's, so the trigger
    // sat higher than the words beside it and the text it became. It is drawn on its own, moved
    // down onto the body's baseline.
    let layout = |before: &str, result: Option<(&str, f32, f32)>| {
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = inner;
        job.append(before, 0.0, egui::TextFormat::simple(body.clone(), quiet));
        if let Some((text, shown, lit)) = result {
            job.append(
                text,
                0.0,
                egui::TextFormat {
                    font_id: body.clone(),
                    color: ink.gamma_multiply(shown),
                    background: crate::app::mix(card, accent, (if is_light { 0.16 } else { 0.28 }) * lit),
                    ..Default::default()
                },
            );
        }
        typesetter.layout_job(job)
    };
    let typed_line = |typed: String| typesetter.layout_no_wrap(typed, mono.clone(), accent);
    // How far the trigger's own top sits above the body row it is typed into.
    let drop = controls::baseline(&typesetter.layout_no_wrap("A".into(), body.clone(), ink))
        - controls::baseline(&typed_line("A".into()));

    // The page is as tall as the tallest example once it has become text, so it never resizes.
    let tallest = examples
        .iter()
        .map(|e| layout(e.before, Some((&e.result, 1.0, 0.0))).size().y)
        .fold(0.0_f32, f32::max);
    let (rect, _) = ui.allocate_exact_size(vec2(width, tallest + 2.0 * SURFACE_PAD), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        controls::RADIUS_SECTION,
        card,
        Stroke::new(1.0, crate::app::hairline(is_light)),
        egui::StrokeKind::Inside,
    );
    if clock < 0.0 {
        // Still arriving: the empty page, the caret not yet blinking.
        frames.after(-clock);
        return;
    }

    let round = clock % (SCENE * examples.len() as f64);
    let example = &examples[(round / SCENE) as usize];
    let s = round % SCENE;
    let keys = example.trigger.chars().count();
    let typed_by = TYPE_FROM + keys as f64 * PER_KEY;
    let turns_at = typed_by + BEAT;
    let leaving = SCENE - FADE_OUT;

    let mut painter = painter.with_clip_rect(rect);
    let scene_in = ease_out(span(s, 0.0, 0.3));
    let scene_out = 1.0 - ease_out(span(s, leaving, SCENE));
    painter.set_opacity(scene_in.min(scene_out));
    let origin = rect.min + vec2(SURFACE_PAD, SURFACE_PAD);

    let caret = if s < turns_at {
        let n = if s < TYPE_FROM { 0 } else { (((s - TYPE_FROM) / PER_KEY) as usize + 1).min(keys) };
        let page = layout(example.before, None);
        let end = page.pos_from_cursor(page.end()).translate(origin.to_vec2());
        let trigger = typed_line(example.trigger.chars().take(n).collect());
        let after = end.left() + trigger.size().x;
        painter.galley(origin, page, ink);
        painter.galley(pos2(end.left(), end.top() + drop), trigger, ink);
        Rect::from_x_y_ranges(after..=after, end.y_range())
    } else {
        let shown = ease_out(span(s, turns_at, turns_at + 0.22));
        let lit = 1.0 - ease_in_out(span(s, turns_at + 0.5, turns_at + 1.5));
        let page = layout(example.before, Some((&example.result, shown, lit)));
        let caret = page.pos_from_cursor(page.end()).translate(origin.to_vec2());
        painter.galley(origin, page, ink);
        caret
    };

    // The caret: solid while keys are going down, blinking while the page waits.
    let typing = (TYPE_FROM..typed_by).contains(&s);
    let blink_on = ((s / BLINK) as i64) % 2 == 0;
    if typing || blink_on {
        painter.line_segment(
            [pos2(caret.left() + 1.0, caret.top() + 2.0), pos2(caret.left() + 1.0, caret.bottom() - 2.0)],
            Stroke::new(1.5, ink),
        );
    }

    // Frames: continuously while something fades, otherwise at the next key or the next blink.
    let next_blink = ((s / BLINK).floor() + 1.0) * BLINK - s;
    if s < 0.3 || (turns_at..turns_at + 1.5).contains(&s) || s >= leaving {
        frames.now();
    } else if typing {
        frames.after(TYPE_FROM + (((s - TYPE_FROM) / PER_KEY).floor() + 1.0) * PER_KEY - s);
    } else if s < turns_at {
        frames.after(next_blink.min(turns_at - s));
    } else {
        frames.after(next_blink.min(leaving - s));
    }
}

/// The same three examples with nothing moving: each trigger, and what it becomes.
fn still_examples(ui: &mut egui::Ui, examples: &[Example; 3]) {
    let is_light = !ui.visuals().dark_mode;
    controls::section_frame(is_light).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let key_width = examples
            .iter()
            .map(|e| controls::widget_line(ui, controls::code(e.trigger, ui.visuals()), egui::TextStyle::Body).size().x)
            .fold(0.0_f32, f32::max);
        for (i, example) in examples.iter().enumerate() {
            if i > 0 {
                ui.add_space(controls::GAP_ROW);
            }
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = controls::GAP_WIDE;
                let (slot, _) = ui.allocate_exact_size(
                    vec2(key_width, ui.text_style_height(&egui::TextStyle::Body)),
                    egui::Sense::hover(),
                );
                // On the baseline of the words beside it, not at the top of the row: Cascadia's
                // baseline sits higher in its box than Segoe's, so top-aligned it floated.
                let line = controls::widget_line(ui, controls::code(example.trigger, ui.visuals()), egui::TextStyle::Body);
                let words = controls::widget_line(ui, egui::RichText::new("A"), egui::TextStyle::Body);
                let drop = controls::baseline(&words) - controls::baseline(&line);
                ui.painter().galley(slot.left_top() + vec2(0.0, drop), line, Color32::PLACEHOLDER);
                ui.label(controls::muted("→", is_light));
                ui.add(egui::Label::new(example.result.as_str()).wrap());
            });
        }
    });
}

// --- 3. try it ------------------------------------------------------------------------------

fn try_it(ui: &mut egui::Ui, state: &mut AppState, enter: &Enter, focus_pending: &mut bool) {
    let t = state.t();
    let is_light = !ui.visuals().dark_mode;
    enter.reveal(ui, 0, |ui| title(ui, t.onboarding_try_title));
    ui.add_space(controls::GAP_ROW);
    enter.reveal(ui, 1, |ui| {
        // The trigger set as a trigger is set everywhere else: monospaced, in the accent.
        let (before, after) = t.onboarding_try_body.split_once("{trigger}").unwrap_or((t.onboarding_try_body, ""));
        let body = egui::TextStyle::Body.resolve(ui.style());
        let quiet = crate::app::secondary_text(is_light);
        let mut job = egui::text::LayoutJob { halign: egui::Align::Center, ..Default::default() };
        job.wrap.max_width = ui.available_width();
        job.append(before, 0.0, egui::TextFormat::simple(body.clone(), quiet));
        job.append(
            PROBE_TRIGGER,
            0.0,
            egui::TextFormat::simple(egui::FontId::monospace(body.size), crate::app::accent(ui.visuals())),
        );
        job.append(after, 0.0, egui::TextFormat::simple(body, quiet));
        ui.add(egui::Label::new(job).wrap());
    });
    ui.add_space(controls::GAP_STACK + controls::GAP_ROW);
    enter.reveal(ui, 2, |ui| {
        let field = ui.add(
            controls::text_field(&mut state.onboarding_probe)
                .desired_width(ui.available_width().min(320.0))
                .hint_text(t.onboarding_try_placeholder),
        );
        // Asked for until it holds: the first pass of a step is often thrown away to centre it,
        // and a request made in a discarded pass is lost with it.
        if *focus_pending {
            if field.has_focus() {
                *focus_pending = false;
            } else {
                field.request_focus();
            }
        }
    });
    // Espanso replaces the trigger wherever the cursor happens to be, including in this field —
    // which is the whole point: nothing here is simulated. The moment the replacement text turns
    // up, it worked.
    if state.onboarding_probe.contains(PROBE_REPLACE) {
        state.onboarding_expanded = true;
    }
    ui.add_space(controls::GAP_SECTION);
    // The room for the answer is kept from the start, so the step does not jump when it comes.
    let shown = ui.ctx().animate_bool_with_time(
        flow_id().with("try-ok"),
        state.onboarding_expanded,
        if crate::theme::animations_enabled() { 0.3 } else { 0.0 },
    );
    let answer = egui::RichText::new(t.onboarding_try_ok).color(crate::app::success(is_light).gamma_multiply(shown));
    ui.add(egui::Label::new(answer).wrap());
}

// --- 4. start with Windows ------------------------------------------------------------------

fn autostart(ui: &mut egui::Ui, state: &mut AppState, enter: &Enter, viewport: f32, frames: &mut Frames) {
    let t = state.t();
    let is_light = !ui.visuals().dark_mode;
    enter.reveal(ui, 0, |ui| title(ui, t.onboarding_autostart_title));
    ui.add_space(controls::GAP_STACK + controls::GAP_ROW);
    enter.reveal(ui, 1, |ui| {
        figure(ui, FIGURE_WIDTH, |ui| {
            controls::section_frame(is_light)
                .inner_margin(egui::Margin::symmetric(controls::SECTION_PAD, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut autostart = state.autostart_enabled;
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            changed = controls::toggle(ui, &mut autostart).changed();
                            ui.add_space(controls::GAP_WIDE);
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(t.autostart_checkbox).wrap());
                            });
                        });
                    });
                    if changed {
                        state.set_autostart(autostart);
                    }
                });
        });
    });
    ui.add_space(controls::GAP_STACK + controls::GAP_ROW);
    enter.reveal(ui, 2, |ui| lead(ui, t.onboarding_where_body));
    ui.add_space(controls::GAP_STACK);
    // The picture is last, so what is above it is already laid out: it takes what is left of the
    // page, up to its full size — a short window gets a smaller tray, never a cut-off one.
    let above = ui.cursor().top() - ui.min_rect().top();
    let room = viewport - above;
    let animate = crate::theme::animations_enabled();
    enter.reveal(ui, 3, |ui| {
        enlarged(ui, room, |ui, rect| {
            if animate {
                tray_demo(ui, rect, enter.elapsed - ENTER_SECONDS, frames);
            } else {
                crate::ui::tips_view::tray_illustration(ui);
            }
        });
    });
}

/// The tray picture's own measure, the one it has in the tips.
const TRAY_SIZE: egui::Vec2 = vec2(264.0, 112.0);
/// How much larger than in the tips the tray is drawn here, when the page has the room.
const TRAY_SCALE: f32 = 1.5;

/// Draws the tray picture at its own measure and enlarges it as a whole, outlines included, so it
/// stays the same picture — only big enough to be the subject of the step. It takes the `room`
/// there is, between its own size and [`TRAY_SCALE`] times it.
fn enlarged(ui: &mut egui::Ui, room: f32, draw: impl FnOnce(&mut egui::Ui, Rect)) {
    // Whole pixels of height, so the picture does not shimmer while a window is being resized.
    let scale = ((room / TRAY_SIZE.y).clamp(1.0, TRAY_SCALE) * TRAY_SIZE.y).floor() / TRAY_SIZE.y;
    let (outer, _) = ui.allocate_exact_size(TRAY_SIZE * scale, egui::Sense::hover());
    if !ui.is_rect_visible(outer) {
        return;
    }
    let rect = Rect::from_min_size(outer.min, TRAY_SIZE);
    let zoom = egui::emath::TSTransform::new(outer.min.to_vec2() * (1.0 - scale), scale);
    ui.with_visual_transform(zoom, |ui| {
        // The clip is enlarged along with the drawing; undone first, it lands back where it was
        // and the picture cannot spill out of a scrolled page onto the tip below it.
        ui.set_clip_rect(zoom.inverse().mul_rect(ui.clip_rect()));
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
        draw(&mut inner, rect);
    });
}

/// One round of the tray demonstration, in seconds.
const TRAY_ROUND: f64 = 5.2;

/// The gesture the step asks for, acted out: the pointer opens the hidden icons, picks ours up,
/// carries it down onto the taskbar and lets go; then the picture resets and does it again.
///
/// The same drawing as [`crate::ui::tips_view::tray_illustration`] — taskbar, clock, chevron,
/// flyout, an outline where the icon belongs — with our icon drawn as the app's own mark.
fn tray_demo(ui: &mut egui::Ui, rect: Rect, clock: f64, frames: &mut Frames) {
    let is_light = !ui.visuals().dark_mode;
    let accent = crate::app::accent(ui.visuals());
    let s = clock.max(0.0) % TRAY_ROUND;
    if clock < 0.0 {
        frames.after(-clock);
    } else if (3.7..4.5).contains(&s) {
        frames.after(4.5 - s);
    } else {
        frames.now();
    }

    let ink = crate::app::text_tertiary(is_light);
    let quiet = ink.gamma_multiply(0.55);
    let surface = crate::app::win_control_for(is_light);
    let edge = crate::app::line_strong(is_light);
    let painter = ui.painter();

    // --- the taskbar ---------------------------------------------------------------------------
    let bar = Rect::from_min_size(pos2(rect.left(), rect.bottom() - 34.0), vec2(rect.width(), 34.0));
    painter.rect_filled(bar, 5.0, surface);
    painter.rect_stroke(bar, 5.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    for (i, w) in [34.0_f32, 40.0].into_iter().enumerate() {
        let y = bar.center().y - 5.0 + i as f32 * 10.0;
        painter.rect_filled(Rect::from_min_size(pos2(bar.right() - 12.0 - w, y), vec2(w, 4.0)), 2.0, quiet);
    }
    let chevron_x = bar.left() + 34.0;
    let landing_x = chevron_x + 34.0;
    for i in 0..2 {
        let slot = Rect::from_center_size(pos2(landing_x + 34.0 + i as f32 * 30.0, bar.center().y), vec2(15.0, 15.0));
        painter.rect_filled(slot, 3.0, quiet);
    }
    let landing = Rect::from_center_size(pos2(landing_x, bar.center().y), vec2(17.0, 17.0));

    // The timeline.
    let open = ease_out(span(s, 0.75, 0.95)) * (1.0 - ease_out(span(s, 2.85, 3.05)));
    let lift = ease_out(span(s, 1.45, 1.6)) * (1.0 - ease_out(span(s, 2.6, 2.75)));
    let carry = ease_in_out(span(s, 1.6, 2.6));
    let reset = ease_out(span(s, 4.5, 5.0));
    let landed = s >= 2.6;

    let outline = if landed { reset } else { 1.0 };
    if outline > 0.0 {
        painter.rect_stroke(landing, 3.0, Stroke::new(1.5, accent.gamma_multiply(outline)), egui::StrokeKind::Inside);
    }
    let chevron = pos2(chevron_x, bar.center().y + 2.0);
    let chevron_ink = crate::app::mix(ink, accent, open);
    painter.line_segment([chevron + vec2(-5.0, 2.0), chevron + vec2(0.0, -3.0)], Stroke::new(1.8, chevron_ink));
    painter.line_segment([chevron + vec2(0.0, -3.0), chevron + vec2(5.0, 2.0)], Stroke::new(1.8, chevron_ink));

    // --- the flyout ----------------------------------------------------------------------------
    let rise = (1.0 - open) * 6.0;
    let flyout = Rect::from_min_size(pos2(chevron_x - 20.0, rect.top() + rise), vec2(104.0, 36.0));
    let ours_home = pos2(flyout.left() + 52.0, flyout.center().y);
    if open > 0.0 {
        let mut fly = painter.clone();
        fly.set_opacity(open);
        fly.rect_filled(flyout, 7.0, surface);
        fly.rect_stroke(flyout, 7.0, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
        for i in [0, 2] {
            let x = flyout.left() + 22.0 + i as f32 * 30.0;
            fly.rect_filled(Rect::from_center_size(pos2(x, flyout.center().y), vec2(16.0, 16.0)), 3.0, quiet);
        }
    }

    // --- our icon --------------------------------------------------------------------------------
    let home = pos2(ours_home.x, rect.top() + 18.0);
    let icon_at = home + (landing.center() - home) * carry;
    let in_flyout = s < 1.45;
    let icon_alpha = if in_flyout { open } else if landed { 1.0 - reset } else { 1.0 };
    if icon_alpha > 0.0 {
        let size = 16.0 * (1.0 + 0.18 * lift);
        let icon = Rect::from_center_size(icon_at + vec2(0.0, if in_flyout { rise } else { 0.0 }), egui::Vec2::splat(size));
        let mut layer = painter.clone();
        layer.set_opacity(icon_alpha);
        paint_mark(&layer, ui.visuals(), icon, lift > 0.0);
    }

    // --- the pointer -----------------------------------------------------------------------------
    let rest = pos2(bar.left() + 150.0, bar.center().y + 4.0);
    let at_chevron = chevron + vec2(2.0, 0.0);
    let pointer = if s < 0.75 {
        rest + (at_chevron - rest) * ease_in_out(span(s, 0.15, 0.7))
    } else if s < 1.45 {
        at_chevron + (home - at_chevron) * ease_in_out(span(s, 0.95, 1.4))
    } else if s < 2.75 {
        icon_at
    } else {
        icon_at + (rest - icon_at) * ease_in_out(span(s, 3.0, 3.6))
    };
    let pressed = (0.7..0.85).contains(&s) || (1.45..2.6).contains(&s);
    let pointer_alpha = 1.0 - reset;
    paint_pointer(painter, pointer + vec2(2.0, 3.0), if pressed { 0.92 } else { 1.0 }, pointer_alpha);
}

/// Windows' arrow pointer: white, edged in black, whatever the theme — as it is on the real screen.
fn paint_pointer(painter: &egui::Painter, tip: egui::Pos2, scale: f32, alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let k = scale;
    let p = |x: f32, y: f32| tip + vec2(x, y) * k;
    let outline = vec![
        p(0.0, 0.0),
        p(0.0, 13.0),
        p(3.4, 9.9),
        p(5.9, 15.2),
        p(8.0, 14.3),
        p(5.6, 9.2),
        p(9.8, 9.2),
    ];
    let white = Color32::WHITE.gamma_multiply(alpha);
    let black = Color32::from_rgb(0x10, 0x10, 0x10).gamma_multiply(alpha);
    // Filled as triangles without feathering; the outline drawn over it is what is antialiased.
    let mut mesh = egui::Mesh::default();
    for v in &outline {
        mesh.colored_vertex(*v, white);
    }
    for [a, b, c] in [[0, 1, 2], [0, 2, 5], [0, 5, 6], [2, 3, 4], [2, 4, 5]] {
        mesh.add_triangle(a, b, c);
    }
    painter.add(mesh);
    painter.add(egui::Shape::closed_line(outline, Stroke::new(1.0, black)));
}
