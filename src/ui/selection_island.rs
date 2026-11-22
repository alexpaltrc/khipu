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

//! The selection island: what can be done to several expansions at once, at the foot of the list.
//!
//! While several are being chosen the round «+» becomes this pill, as it becomes the island for a
//! message: its accent condenses into the box that counts them, its plus folds into that box's
//! check, and the count and the actions arrive one after another. The count is the box and a
//! number, the same box ticked on every chosen row, so it needs no word to be read; the number
//! rolls as it changes. Leaving is the same played backwards.
//!
//! It only draws and reports what was pressed ([`Pressed`]); `list_view` decides what that does to
//! the selection, and where the pill and the «+» beside it stand. Messages said while choosing rise
//! above it (`island::Source::Over`), so nothing here waits for one to end.

use crate::app::{accent_fill, AppState};
use crate::ui::glyphs::{self, Glyph};
use crate::ui::island::{ease_out, Spring, CHIP_HEIGHT, CLOSE_DELAY, CROSS, ISLAND_HEIGHT, ISLAND_RULE, LETTER_IN, LETTER_RISE};
use crate::ui::keys;
use crate::ui::list_view::{lift, NEW_BUTTON, PLUS_ARM, PLUS_STROKE};
use crate::ui::studio;

/// From either round end to the middle of what sits in it: the count's box on the left, the cross
/// on the right. Half the pill's height, so both sit at the centre of their end's circle.
const END: f32 = ISLAND_HEIGHT * 0.5;
/// The count's box, as big as the one on each row.
const BOX: f32 = 18.0;
/// From the box to the number, and from the number to the first hairline.
const AFTER_BOX: f32 = 9.0;
const AFTER_NUMBER: f32 = 12.0;
/// Each action's place, and the room either side of a hairline.
const SLOT: f32 = 32.0;
const BESIDE_RULE: f32 = 6.0;
/// How many actions sit between the hairlines.
const ACTIONS: usize = 4;

/// The choreography, in seconds: the number waits for the pill to be well on its way, then each
/// action follows the one before it; leaving, they go from the last, quicker.
const NUMBER_DELAY: f64 = 0.15;
const STAGGER: f64 = 0.035;
const OUT: f64 = 0.12;
const OUT_STAGGER: f64 = 0.015;
/// How long the number takes to roll to a new count, and how far it travels doing so.
const ROLL: f64 = 0.26;
const ROLL_RISE: f32 = 9.0;

/// What was pressed this frame.
#[derive(Default)]
pub struct Pressed {
    pub select_all: bool,
    pub clear: bool,
    pub move_them: bool,
    /// Turn them off (`true`) or on.
    pub switch_off: Option<bool>,
    pub delete: bool,
    pub done: bool,
}

/// What the pill shows about the selection.
pub struct Selection {
    pub count: usize,
    /// Every expansion the list shows is ticked: the first action clears instead.
    pub all_ticked: bool,
    /// The list shows anything at all to tick.
    pub any_visible: bool,
    /// Every ticked expansion is off: the switch turns them on.
    pub all_off: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Open,
    Closing,
}

#[derive(Clone)]
struct Pill {
    phase: Phase,
    since: f64,
    /// When the number and the actions began to arrive, and, closing, to leave.
    at: f64,
    out: Option<f64>,
    width: Spring,
    height: Spring,
    /// 0 is the plus, 1 the check.
    morph: Spring,
    /// 0 is the accent, 1 the surface.
    tone: Spring,
    /// 0 is an empty box, 1 a ticked one.
    tick: Spring,
    lift: Spring,
    /// The count drawn, the one it is rolling from, and when it began to.
    count: usize,
    from: usize,
    rolled: f64,
}

impl Pill {
    fn new(now: f64, count: usize) -> Self {
        let ticked = if count > 0 { 1.0 } else { 0.0 };
        Self {
            phase: Phase::Open,
            since: now,
            at: now + NUMBER_DELAY,
            out: None,
            width: Spring::new(NEW_BUTTON, 420.0, 30.0),
            height: Spring::new(NEW_BUTTON, 420.0, 38.0),
            morph: Spring::new(0.0, 240.0, 23.0),
            tone: Spring::new(0.0, 260.0, 32.0),
            tick: Spring::new(ticked, 380.0, 30.0),
            lift: Spring::new(0.0, 300.0, 35.0),
            count,
            from: count,
            rolled: f64::NEG_INFINITY,
        }
    }

    fn springs(&mut self) -> [&mut Spring; 6] {
        [&mut self.width, &mut self.height, &mut self.morph, &mut self.tone, &mut self.tick, &mut self.lift]
    }
}

fn id() -> egui::Id {
    egui::Id::new("selection-island")
}

/// Whether the pill is on screen, open or on its way back to the «+».
pub fn alive(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Pill>(id()).is_some())
}

/// How far item `i` (0 is the number) has arrived, and how far below its line it still sits.
fn arrival(now: f64, pill: &Pill, i: usize, items: usize) -> (f32, f32) {
    let came = ease_out((now - pill.at - i as f64 * STAGGER) / LETTER_IN);
    let stayed = pill.out.map_or(1.0, |out| {
        1.0 - ease_out((now - out - (items - 1 - i) as f64 * OUT_STAGGER) / OUT)
    });
    let shown = came * stayed;
    (shown, (1.0 - came) * LETTER_RISE - (1.0 - stayed) * LETTER_RISE)
}

/// The pill, while there is one: where it is this frame and whether it is open, plus what was
/// pressed. `circle` is the round «+» it grows out of and goes back into; `open` whether there is a
/// selection to show.
pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    circle: egui::Rect,
    open: bool,
    sel: &Selection,
) -> (Option<(egui::Rect, bool)>, Pressed) {
    let mut pressed = Pressed::default();
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let motion = crate::theme::animations_enabled();

    let mut pill: Option<Pill> = ctx.data(|d| d.get_temp(id()));
    match (pill.as_mut(), open) {
        (None, true) => pill = Some(Pill::new(now, sel.count)),
        (None, false) => return (None, pressed),
        (Some(p), true) if p.phase == Phase::Closing => {
            p.phase = Phase::Open;
            p.since = now;
            p.at = now;
            p.out = None;
        }
        (Some(p), false) if p.phase == Phase::Open => {
            p.phase = Phase::Closing;
            p.since = now;
            p.out = Some(now);
        }
        _ => {}
    }
    let Some(mut pill) = pill else { return (None, pressed) };

    // The count, rolling to its new value. Closing keeps the last one: the selection is emptied as
    // the pill leaves, and a number rolling to nothing would be the last thing seen.
    if pill.phase == Phase::Open && sel.count != pill.count {
        pill.from = pill.count;
        pill.count = sel.count;
        pill.rolled = if motion { now } else { f64::NEG_INFINITY };
    }

    let is_light = !ui.visuals().dark_mode;
    let contrast = crate::app::contrast().is_some();
    let (fill, plus_ink) = accent_fill(ui.visuals());
    let surface = crate::app::win_control_for(is_light);
    let text = ui.visuals().strong_text_color();
    let body = egui::TextStyle::Body.resolve(ui.style());
    let strong = egui::FontId::new(body.size, crate::fonts::semibold_family());
    let number = |n: usize| ui.painter().layout_no_wrap(n.to_string(), strong.clone(), text);
    let now_number = number(pill.count);

    // Where the springs are headed. Closing waits for the actions to be gone.
    let items = 2 + ACTIONS;
    let opened = pill.phase == Phase::Open || now - pill.since < if motion { CLOSE_DELAY + OUT } else { 0.0 };
    let right_side = BESIDE_RULE + ACTIONS as f32 * SLOT + BESIDE_RULE + 1.0 + BESIDE_RULE + CHIP_HEIGHT * 0.5 + END;
    let width = END + BOX * 0.5 + AFTER_BOX + now_number.size().x + AFTER_NUMBER + 1.0 + right_side;
    if opened {
        pill.width.to = width;
        pill.height.to = ISLAND_HEIGHT;
        pill.morph.to = 1.0;
        pill.tone.to = 1.0;
    } else {
        pill.width.to = NEW_BUTTON;
        pill.height.to = NEW_BUTTON;
        pill.morph.to = 0.0;
        pill.tone.to = 0.0;
    }
    pill.tick.to = if pill.count > 0 { 1.0 } else { 0.0 };
    for spring in pill.springs() {
        if motion {
            spring.step(dt);
        } else {
            spring.snap();
        }
    }
    if pill.phase == Phase::Closing
        && !opened
        && (pill.width.x - NEW_BUTTON).abs() < 0.5
        && pill.morph.x.abs() < 0.01
        && pill.tone.x < 0.01
    {
        ctx.data_mut(|d| d.remove::<Pill>(id()));
        return (None, pressed);
    }

    // The pill, stretched by its springs, resting on the circle's foot.
    let squash = (pill.width.v * 0.004).clamp(0.0, 3.0);
    let size = egui::vec2(pill.width.x, pill.height.x - squash);
    let foot = circle.center_bottom();
    let rect = egui::Rect::from_min_size(egui::pos2(foot.x - size.x * 0.5, foot.y - size.y - 1.5 * pill.lift.x), size);
    let radius = size.x.min(size.y) * 0.5;
    let middle = rect.center().y;

    let live = pill.phase == Phase::Open && pill.morph.x > 0.5;
    // It takes presses and drags too, though it does nothing with them: what floats over the list must
    // not let a press through to start the box that picks rows, which would empty the selection.
    let body_response = ui.interact(rect, id().with("body"), egui::Sense::click_and_drag());
    let t = state.t();
    let count_words = crate::i18n::fill(
        studio::plural(
            pill.count,
            studio::text(state, "{n} expansión seleccionada", "{n} expansion selected", "{n} expansion ang napili", "{n} विस्तार चयनित"),
            studio::text(state, "{n} expansiones seleccionadas", "{n} expansions selected", "{n} expansion ang napili", "{n} विस्तार चयनित"),
        ),
        &[("n", &pill.count.to_string())],
    );
    body_response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &count_words));
    pill.lift.to = if pill.phase == Phase::Open && body_response.hovered() { 1.0 } else { 0.0 };

    let tone = pill.tone.x.clamp(0.0, 1.0);
    let painter = ctx.layer_painter(ui.layer_id()).with_clip_rect(ctx.content_rect());
    lift(&painter, rect, is_light);
    if pill.lift.x > 0.01 && !contrast {
        let alpha = if is_light { 18.0 } else { 40.0 } * pill.lift.x;
        let shadow = egui::Shadow { offset: [0, 10], blur: 28, spread: 0, color: egui::Color32::from_black_alpha(alpha as u8) };
        painter.add(shadow.as_shape(rect, egui::CornerRadius::same(radius as u8)));
    }
    painter.rect_filled(rect, radius, fill.lerp_to_gamma(surface, tone));
    let edge = if contrast {
        text
    } else if is_light {
        egui::Color32::from_black_alpha((15.0 * tone) as u8)
    } else {
        egui::Color32::from_white_alpha((18.0 * tone) as u8)
    };
    painter.rect_stroke(rect, radius, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    let inside = painter.with_clip_rect(rect.intersect(ctx.content_rect()));

    // The box: the accent of the circle, condensed into it as the pill turns to the surface. Empty,
    // it is the row's empty box. It travels from the middle of the circle to its place at the left
    // end, and never trails past that end as the pill closes on it.
    let m = pill.morph.x.clamp(-0.15, 1.15);
    let home = rect.left() + END;
    let mark_x = (foot.x + (home - foot.x) * m.clamp(0.0, 1.0)).clamp(rect.left() + radius.min(END), rect.right() - radius.min(END));
    let mark = egui::pos2(mark_x, middle);
    let tick = pill.tick.x.clamp(0.0, 1.0);
    let boxed = egui::Rect::from_center_size(mark, egui::Vec2::splat(BOX));
    if tone > 0.01 {
        let outline = crate::app::line_strong(is_light);
        inside.rect_filled(boxed, 4.0, surface.gamma_multiply(tone * (1.0 - tick)));
        inside.rect_stroke(boxed, 4.0, egui::Stroke::new(1.0, outline.gamma_multiply(tone * (1.0 - tick))), egui::StrokeKind::Inside);
        inside.rect_filled(boxed, 4.0, fill.gamma_multiply(tick.max(1.0 - tone)));
    }

    // The plus folding into the check: the upright bar becomes the long stroke, the level one the
    // short, as the box on each row draws it. With nothing ticked the check gathers into the
    // corner it starts from and is gone.
    let arm = PLUS_ARM * 0.5 - PLUS_STROKE * 0.5;
    let check = |fx: f32, fy: f32| egui::vec2(BOX * (fx - 0.5), BOX * (fy - 0.5));
    let (a, b, c) = (check(0.24, 0.52), check(0.42, 0.70), check(0.76, 0.31));
    let toward = |p: egui::Vec2| a + (p - a) * tick;
    let bars = [
        ([egui::vec2(0.0, -arm), egui::vec2(0.0, arm)], [toward(b), toward(c)]),
        ([egui::vec2(-arm, 0.0), egui::vec2(arm, 0.0)], [toward(a), toward(b)]),
    ];
    let width_now = PLUS_STROKE + ((BOX * 0.115).max(1.6) - PLUS_STROKE) * m.clamp(0.0, 1.0);
    let shown = 1.0 - (1.0 - tick) * m.clamp(0.0, 1.0);
    if shown > 0.01 {
        let ink = plus_ink.gamma_multiply(shown);
        for (plus, end) in bars {
            let at = |k: usize| mark + plus[k] + (end[k] - plus[k]) * m;
            inside.line_segment([at(0), at(1)], egui::Stroke::new(width_now, ink));
            inside.circle_filled(at(0), width_now * 0.5, ink);
            inside.circle_filled(at(1), width_now * 0.5, ink);
        }
    }

    // The number, rolling: the old one leaves upwards as the new one comes up from below, or the
    // other way round when the count falls.
    let (number_shown, number_rise) = arrival(now, &pill, 0, items);
    let number_x = rect.left() + END + BOX * 0.5 + AFTER_BOX;
    let roll = ease_out((now - pill.rolled) / ROLL);
    let up = if pill.count >= pill.from { 1.0 } else { -1.0 };
    let draw_number = |galley: std::sync::Arc<egui::Galley>, offset: f32, alpha: f32| {
        if alpha > 0.003 {
            let at = egui::pos2(number_x, middle - galley.size().y * 0.5 + number_rise + offset);
            inside.galley_with_override_text_color(at, galley, text.gamma_multiply(alpha * number_shown));
        }
    };
    if roll < 1.0 {
        draw_number(number(pill.from), -up * ROLL_RISE * roll, 1.0 - roll);
    }
    draw_number(now_number.clone(), up * ROLL_RISE * (1.0 - roll), roll);
    let count_rect = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top()), egui::pos2(number_x + now_number.size().x, rect.bottom()));
    ui.interact(count_rect, id().with("count"), egui::Sense::hover()).on_hover_text(&count_words);

    // The hairlines and the actions are laid from the right end, so they ride it as the number
    // changes width.
    let rule = |x: f32, i: usize| {
        let (shown, _) = arrival(now, &pill, i, items);
        let tall = ISLAND_RULE * shown;
        if tall > 0.1 {
            inside.vline(x + 0.5, egui::Rangef::new(middle - tall * 0.5, middle + tall * 0.5), egui::Stroke::new(1.0, text.gamma_multiply(0.2 * shown)));
        }
    };
    let close_center = egui::pos2(rect.right() - END, middle);
    let rule_two = close_center.x - CHIP_HEIGHT * 0.5 - BESIDE_RULE - 1.0;
    let actions_right = rule_two - BESIDE_RULE;
    let actions_left = actions_right - ACTIONS as f32 * SLOT;
    rule(actions_left - BESIDE_RULE - 1.0, 0);
    rule(rule_two, items - 2);

    let lang = state.settings.lang;
    let any = sel.count > 0;
    let switch_tip = if sel.all_off {
        studio::text(state, "Encender", "Turn on", "Buksan", "चालू करें")
    } else {
        studio::text(state, "Apagar", "Turn off", "Patayin", "बंद करें")
    };
    let select_tip = if sel.all_ticked {
        studio::text(state, "Quitar selección", "Clear selection", "Alisin ang pinili", "चयन हटाएँ")
    } else {
        studio::text(state, "Seleccionar todas", "Select all", "Piliin lahat", "सभी चुनें")
    };
    let move_tip = studio::text(state, "Mover a carpeta", "Move to folder", "Ilipat sa folder", "फ़ोल्डर में ले जाएँ");
    let delete_tip = keys::tip(t.delete_selected, keys::DELETE, lang);
    let stop_tip = keys::tip(
        studio::text(state, "Terminar selección", "Stop selecting", "Tapusin ang pagpili", "चयन समाप्त करें"),
        keys::BACK,
        lang,
    );
    let select_glyph = if sel.all_ticked { Glyph::ClearSelection } else { Glyph::SelectAll };
    let actions: [(Glyph, &str, bool, bool); ACTIONS] = [
        (select_glyph, select_tip, sel.any_visible, false),
        (Glyph::MoveToFolder, move_tip, any, false),
        (Glyph::Power, switch_tip, any, false),
        (Glyph::Delete, &delete_tip, any, true),
    ];
    let mut clicked = [false; ACTIONS + 1];
    for (k, (glyph, tip, enabled, danger)) in actions.into_iter().enumerate() {
        let center = egui::pos2(actions_left + SLOT * (k as f32 + 0.5), middle);
        let (shown, rise) = arrival(now, &pill, k + 1, items);
        let r = button(ui, &inside, id().with(("action", k)), center, enabled, live, tip, shown, rise, is_light, |ui, at, ink, hovered| {
            let ink = if danger && hovered { crate::app::danger(is_light) } else { ink };
            glyphs::paint(ui, egui::Rect::from_center_size(at, egui::Vec2::splat(SLOT)), glyph, glyphs::SIZE, ink);
        });
        clicked[k] = r;
    }
    let (shown, rise) = arrival(now, &pill, items - 1, items);
    clicked[ACTIONS] = button(ui, &inside, id().with("done"), close_center, true, live, &stop_tip, shown, rise, is_light, |_, at, ink, _| {
        let stroke = egui::Stroke::new(1.5, ink);
        inside.line_segment([at + egui::vec2(-CROSS, -CROSS), at + egui::vec2(CROSS, CROSS)], stroke);
        inside.line_segment([at + egui::vec2(-CROSS, CROSS), at + egui::vec2(CROSS, -CROSS)], stroke);
    });

    if clicked[0] {
        if sel.all_ticked {
            pressed.clear = true;
        } else {
            pressed.select_all = true;
        }
    }
    pressed.move_them = clicked[1];
    if clicked[2] {
        pressed.switch_off = Some(!sel.all_off);
    }
    pressed.delete = clicked[3];
    pressed.done = clicked[ACTIONS];

    // Frames only while something moves: a pill left open for minutes costs nothing while it waits.
    let springing = pill.springs().into_iter().any(|s| (s.x - s.to).abs() > 0.001 || s.v.abs() > 0.01);
    let arriving = now - pill.at < NUMBER_DELAY + items as f64 * STAGGER + LETTER_IN;
    if springing || arriving || now - pill.rolled < ROLL || pill.phase == Phase::Closing {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id(), pill));
    (Some((rect, opened)), pressed)
}

/// One round action in the pill: a patch under the pointer, sinking a hair while pressed, its
/// mark arriving as a letter would. Returns whether it was clicked.
#[allow(clippy::too_many_arguments)]
fn button(
    ui: &mut egui::Ui,
    inside: &egui::Painter,
    id: egui::Id,
    center: egui::Pos2,
    enabled: bool,
    live: bool,
    tip: &str,
    shown: f32,
    rise: f32,
    is_light: bool,
    draw: impl FnOnce(&egui::Ui, egui::Pos2, egui::Color32, bool),
) -> bool {
    let patch = egui::Rect::from_center_size(center, egui::vec2(CHIP_HEIGHT, CHIP_HEIGHT));
    let sense = if live && enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let response = ui.interact(patch, id, sense).on_hover_text(tip);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, live && enabled, tip));
    let hovered = live && enabled && response.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let pace = if crate::theme::animations_enabled() { 0.1 } else { 0.0 };
    let hover = ui.ctx().animate_bool_with_time(id.with("hover"), hovered, pace) * shown;
    let down = ui.ctx().animate_bool_with_time(id.with("down"), live && enabled && response.is_pointer_button_down_on(), pace);
    let text = ui.visuals().strong_text_color();
    if hover > 0.01 {
        let wash = if is_light { 0.06 } else { 0.08 };
        let r = patch.width() * 0.5 * (1.0 - 0.06 * down);
        inside.circle_filled(center, r, text.gamma_multiply(wash * hover));
    }
    if response.has_focus() {
        inside.circle_stroke(center, CHIP_HEIGHT * 0.5 + 1.0, egui::Stroke::new(2.0, text));
    }
    if shown > 0.003 {
        let ink = if enabled { text } else { crate::app::disabled_text(is_light) };
        draw(ui, center + egui::vec2(0.0, rise), ink.gamma_multiply(shown), hovered);
    }
    live && enabled && response.clicked()
}
