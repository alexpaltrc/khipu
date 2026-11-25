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

//! The drop island: the folders an expansion being dragged can be dropped on, at the foot of the
//! list.
//!
//! While a row is dragged the round «+» becomes this pill, as it becomes the island for a message
//! and the selection island for choosing several: its accent melts into the surface, its plus fades,
//! and the folders arrive one after another. The one under the pointer fills with the accent and
//! widens; the one the expansion is already in is dimmed and takes nothing; «Sin carpeta» stands
//! apart, after a hairline, and only when there is a folder to take it out of. When the folders do
//! not all fit, the most recently used come first and «Más…» opens the whole list. The one the last
//! move went to always comes first, and arrives lit, so the next move is a short reach to it.
//!
//! A drop on a folder does not close the pill: it hands its width to the island
//! ([`crate::ui::island::seed`]), which says «Movida a …» with Deshacer from there, so the same pill
//! is seen to change rather than one leaving as another comes. Released anywhere else, it closes
//! back into the «+».
//!
//! «+ Nueva», last, is for a folder that does not exist yet. Dropped on, the pill keeps what it
//! carries and becomes the box for the new folder's name: the folders go, the pill draws in to the
//! field, and the keyboard is in it. Enter files what was carried under that name, which is what
//! makes the folder, and hands over to the island like a drop on a folder; Esc or a click
//! elsewhere makes nothing, and the pill closes back into the «+».
//!
//! It only draws and reports what was dropped where ([`Dropped`]); `list_view` decides what that
//! does.

use std::sync::Arc;

use crate::app::{accent_fill, AppState, DragPayload};
use crate::ui::glyphs::{self, Glyph};
use crate::ui::island::{ease_out, Spring, CHIP_HEIGHT, CLOSE_DELAY, ISLAND_HEIGHT, ISLAND_RULE, LETTER_IN, LETTER_RISE};
use crate::ui::list_view::{lift, NEW_BUTTON, PLUS_ARM, PLUS_STROKE};
use crate::ui::studio;

/// From either round end to the first and last chip: what is left of the pill's height round one.
const END: f32 = (ISLAND_HEIGHT - CHIP_HEIGHT) * 0.5;
/// Inside a chip, from its sides to what it holds, and from its folder to its name.
const PAD: f32 = 12.0;
const ICON_GAP: f32 = 6.0;
/// Between two chips, and either side of the hairline before «Sin carpeta».
const BETWEEN: f32 = 4.0;
const BESIDE_RULE: f32 = 6.0;
/// How much wider the chip under the pointer grows.
const WIDEN: f32 = 8.0;
/// The longest folder name shown whole; a longer one ends in «…».
const NAME_CHARS: usize = 20;
/// The narrowest the box for a new folder's name draws in to.
const NAME_MIN: f32 = 140.0;

/// The choreography, in seconds: the folders wait for the pill to be well on its way, then each
/// follows the one before; leaving, they go together, quickly.
const CHIPS_DELAY: f64 = 0.12;
const STAGGER: f64 = 0.03;
const OUT: f64 = 0.1;

/// A place the expansion can be dropped: a folder, or none.
pub struct Target {
    pub folder: Option<String>,
    /// The folder the dragged expansions are all in already: shown, dimmed, and takes nothing.
    pub current: bool,
    /// The folder something was last moved to: first, and lit, so the next move is one short reach.
    pub last: bool,
}

/// What a drop landed on.
pub enum Dropped {
    /// A folder, or «Sin carpeta»; a folder named in «+ Nueva» is one that does not exist yet.
    On(Option<String>),
    /// «Más…»: the whole list of folders, to choose from.
    More,
}

/// What a chip does with what is dropped on it.
enum Take {
    Folder(Option<String>),
    More,
    New,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Open,
    /// Dropped on «+ Nueva»: the pill is the box for the name.
    Naming,
    Closing,
}

#[derive(Clone)]
struct Pill {
    phase: Phase,
    since: f64,
    width: Spring,
    height: Spring,
    /// 0 is the accent and the plus, 1 the surface.
    tone: Spring,
    /// While naming: what was dropped, and the name so far.
    naming: Option<(Arc<DragPayload>, String)>,
}

fn id() -> egui::Id {
    egui::Id::new("drop-island")
}

fn field_id() -> egui::Id {
    id().with("name")
}

/// Whether the pill is on screen, open or on its way back to the «+».
pub fn alive(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Pill>(id()).is_some())
}

/// Whether the pill is the box for a new folder's name, which has the keyboard.
pub fn naming(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Pill>(id()).is_some_and(|p| p.phase == Phase::Naming))
}

/// What the pill is holding up, laid out: each chip's place, what it says and what it takes.
struct Chip {
    drop: Option<Take>,
    glyph: Option<Glyph>,
    words: Arc<egui::Galley>,
    width: f32,
    /// After the hairline.
    apart: bool,
    /// Where the last move went: washed in the accent before the pointer is anywhere near.
    lit: bool,
}

/// The pill, while there is one: where it is this frame (never `open`, so the «+» does not bud
/// beside it), and what was dropped on it. `circle` is the round «+» it grows out of; `room` the
/// widest it may be; `targets` the folders in the order they are offered, «Sin carpeta» among them
/// when it is to be shown. `hand_over` says whether a drop on a folder becomes the island's message
/// here; when that message rises elsewhere, over the selection island, the pill closes as usual.
pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    circle: egui::Rect,
    room: f32,
    targets: &[Target],
    hand_over: bool,
) -> (Option<egui::Rect>, Option<(Dropped, Arc<DragPayload>)>) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let motion = crate::theme::animations_enabled();
    let open = egui::DragAndDrop::has_payload_of_type::<DragPayload>(&ctx);

    let mut pill: Option<Pill> = ctx.data(|d| d.get_temp(id()));
    match (pill.as_mut(), open) {
        (None, true) => {
            pill = Some(Pill {
                phase: Phase::Open,
                since: now,
                // Nearly critical: a pill this wide that overshoots, even by 3 %, is seen pulling back in
                // at both ends, a tremble just as the folders settle. At 0.9 of critical it stops within a
                // pixel of its width.
                width: Spring::new(NEW_BUTTON, 420.0, 37.0),
                height: Spring::new(NEW_BUTTON, 420.0, 38.0),
                tone: Spring::new(0.0, 260.0, 32.0),
                naming: None,
            })
        }
        (None, false) => return (None, None),
        // A new drag while a name was being typed: that name is let go, for this drag's folders.
        (Some(p), true) if p.phase == Phase::Closing || (p.phase == Phase::Naming && p.since < now) => {
            p.phase = Phase::Open;
            p.since = now;
            p.naming = None;
        }
        (Some(p), false) if p.phase == Phase::Open => {
            p.phase = Phase::Closing;
            p.since = now;
        }
        _ => {}
    }
    let Some(mut pill) = pill else { return (None, None) };

    let is_light = !ui.visuals().dark_mode;
    let contrast = crate::app::contrast().is_some();
    let (fill, on_fill) = accent_fill(ui.visuals());
    let surface = crate::app::win_control_for(is_light);
    let text = ui.visuals().strong_text_color();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let words = |s: &str| ui.painter().layout_no_wrap(s.to_owned(), font.clone(), text);

    // The chips, and as many of the folders as fit: «Sin carpeta» always, «Más…» when some are left.
    let t = state.t();
    let chip = |drop: Take, glyph: Option<Glyph>, label: &str, apart: bool| {
        let words = words(label);
        let width = PAD * 2.0 + words.size().x + glyph.map_or(0.0, |_| glyphs::SIZE + ICON_GAP);
        Chip { drop: Some(drop), glyph, words, width, apart, lit: false }
    };
    let none = targets
        .iter()
        .find(|target| target.folder.is_none())
        .map(|_| chip(Take::Folder(None), None, t.no_folder, true));
    let new = chip(Take::New, Some(Glyph::Add), studio::text(state, "Nueva", "New", "Bago", "नया"), none.is_none());
    let more = chip(Take::More, None, studio::text(state, "Más…", "More…", "Higit pa…", "और…"), false);
    let fixed = END * 2.0
        + WIDEN
        + BESIDE_RULE * 2.0
        + 1.0
        + new.width
        + none.as_ref().map_or(0.0, |c| c.width + BETWEEN);
    let folders: Vec<(Chip, bool)> = targets
        .iter()
        .filter_map(|target| {
            let name = target.folder.as_deref()?;
            let mut c = chip(Take::Folder(Some(name.to_owned())), Some(Glyph::Folder), &crate::app::truncate(name, NAME_CHARS), false);
            if target.current {
                c.drop = None;
            }
            c.lit = target.last && !target.current;
            Some((c, target.current))
        })
        .collect();
    let all: f32 = folders.iter().map(|(c, _)| c.width + BETWEEN).sum();
    let mut chips: Vec<(Chip, bool)> = Vec::new();
    if fixed + all <= room {
        chips = folders;
    } else {
        let mut used = fixed + more.width;
        for (c, current) in folders {
            if used + c.width + BETWEEN > room {
                break;
            }
            used += c.width + BETWEEN;
            chips.push((c, current));
        }
        chips.push((more, false));
    }
    chips.extend(none.map(|c| (c, false)));
    chips.push((new, false));

    // How far each chip has arrived, and how far below its line it still sits. Leaving, for the
    // «+» or for the name, they go together.
    let items = chips.len();
    let (phase, since) = (pill.phase, pill.since);
    let arrival = |i: usize| {
        if phase == Phase::Open {
            let came = ease_out((now - since - CHIPS_DELAY - i as f64 * STAGGER) / LETTER_IN);
            (came, (1.0 - came) * LETTER_RISE)
        } else {
            (1.0 - ease_out((now - since) / OUT), 0.0)
        }
    };

    // While naming: the name, or what it asks for, sets how wide the box is.
    let hint = studio::text(state, "Nombre de la carpeta", "Folder name", "Pangalan ng folder", "फ़ोल्डर का नाम");
    let naming_width = pill.naming.as_ref().map(|(_, name)| {
        let shown = words(if name.is_empty() { hint } else { name }).size().x;
        END * 2.0 + PAD * 2.0 + glyphs::SIZE + ICON_GAP + shown.max(NAME_MIN) + 2.0
    });

    // The pill under the pointer: its chips find which, and how far each has widened, before the
    // springs are told how wide to be.
    let pointer = ctx.pointer_latest_pos();
    let live = pill.phase == Phase::Open;
    let pace = if motion { 0.12 } else { 0.0 };
    let mut widened: Vec<f32> = Vec::with_capacity(items);
    let mut width = END * 2.0;
    for (k, (c, _)) in chips.iter().enumerate() {
        let over = live && c.drop.is_some() && pill_slot(&ctx, k).is_some_and(|r| pointer.is_some_and(|p| r.contains(p)));
        let w = ctx.animate_bool_with_time(id().with(("widen", k)), over, pace);
        widened.push(w);
        width += c.width + WIDEN * w + if c.apart { BESIDE_RULE * 2.0 + 1.0 } else if k > 0 { BETWEEN } else { 0.0 };
    }

    let opened = pill.phase != Phase::Closing || now - pill.since < if motion { CLOSE_DELAY } else { 0.0 };
    if opened {
        pill.width.to = naming_width.unwrap_or(width).min(room);
        pill.height.to = ISLAND_HEIGHT;
        pill.tone.to = 1.0;
    } else {
        pill.width.to = NEW_BUTTON;
        pill.height.to = NEW_BUTTON;
        pill.tone.to = 0.0;
    }
    for spring in [&mut pill.width, &mut pill.height, &mut pill.tone] {
        if motion {
            spring.step(dt);
        } else {
            spring.snap();
        }
    }
    if pill.phase == Phase::Closing && !opened && (pill.width.x - NEW_BUTTON).abs() < 0.5 && pill.tone.x < 0.01 {
        ctx.data_mut(|d| d.remove::<Pill>(id()));
        return (None, None);
    }

    // The pill, on the circle's foot.
    let size = egui::vec2(pill.width.x, pill.height.x);
    let foot = circle.center_bottom();
    let rect = egui::Rect::from_min_size(egui::pos2(foot.x - size.x * 0.5, foot.y - size.y), size);
    let radius = size.x.min(size.y) * 0.5;
    let middle = rect.center().y;
    // It takes the pointer, so nothing under it answers a drop meant for it.
    ui.interact(rect, id().with("body"), egui::Sense::hover());

    let tone = pill.tone.x.clamp(0.0, 1.0);
    let painter = ctx.layer_painter(ui.layer_id()).with_clip_rect(ctx.content_rect());
    lift(&painter, rect, is_light);
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

    // The «+», fading as the accent it is drawn on does.
    if tone < 0.99 {
        let arm = PLUS_ARM * 0.5;
        let ink = on_fill.gamma_multiply(1.0 - tone);
        let c = egui::pos2(foot.x, middle);
        inside.line_segment([c - egui::vec2(arm, 0.0), c + egui::vec2(arm, 0.0)], egui::Stroke::new(PLUS_STROKE, ink));
        inside.line_segment([c - egui::vec2(0.0, arm), c + egui::vec2(0.0, arm)], egui::Stroke::new(PLUS_STROKE, ink));
    }

    // The chips, from the left end, centred in the pill as it grows.
    let mut x = rect.center().x - width * 0.5 + END;
    let mut dropped = None;
    for (k, (c, current)) in chips.into_iter().enumerate() {
        let grown = WIDEN * widened[k];
        if c.apart {
            let (shown, _) = arrival(k);
            x += BESIDE_RULE;
            let tall = ISLAND_RULE * shown;
            inside.vline(x + 0.5, egui::Rangef::new(middle - tall * 0.5, middle + tall * 0.5), egui::Stroke::new(1.0, text.gamma_multiply(0.2 * shown)));
            x += 1.0 + BESIDE_RULE;
        } else if k > 0 {
            x += BETWEEN;
        }
        let slot = egui::Rect::from_min_size(egui::pos2(x, middle - CHIP_HEIGHT * 0.5), egui::vec2(c.width + grown, CHIP_HEIGHT));
        x = slot.right();
        ctx.data_mut(|d| d.insert_temp(id().with(("slot", k)), slot));
        let (shown, rise) = arrival(k);
        if shown < 0.003 {
            continue;
        }
        let h = widened[k];
        // The last folder moved to wears the wash a row saved a moment ago wears, under the fill the
        // pointer brings; in a contrast theme the system's colours carry nothing of the sort.
        if c.lit && !contrast && h < 0.99 {
            let wash = crate::app::mix(surface, fill, if is_light { 0.11 } else { 0.20 });
            inside.rect_filled(slot, CHIP_HEIGHT * 0.5, wash.gamma_multiply(shown * (1.0 - h)));
        }
        if h > 0.01 {
            inside.rect_filled(slot, CHIP_HEIGHT * 0.5, fill.gamma_multiply(h * shown));
        }
        let dim = if current { 0.45 } else { 1.0 };
        let ink = text.lerp_to_gamma(on_fill, h).gamma_multiply(shown * dim);
        let inner = c.width - PAD * 2.0;
        let mut at = slot.center().x - inner * 0.5;
        if let Some(glyph) = c.glyph {
            let icon = egui::Rect::from_min_size(egui::pos2(at, middle - glyphs::SIZE * 0.5 + rise), egui::Vec2::splat(glyphs::SIZE));
            // Through the pill's own clip: a chip still coming in must not show outside it.
            match glyphs::shape(ui, icon, glyph, glyphs::SIZE, ink) {
                Some(shape) => {
                    inside.add(shape);
                }
                None => glyphs::paint(ui, icon, glyph, glyphs::SIZE, ink),
            }
            at += glyphs::SIZE + ICON_GAP;
        }
        let top = middle - c.words.size().y * 0.5 + rise;
        inside.galley_with_override_text_color(egui::pos2(at.round(), top), c.words, ink);

        if live {
            if let Some(drop) = c.drop {
                let response = ui.interact(slot, id().with(("chip", k)), egui::Sense::hover());
                if let Some(payload) = response.dnd_release_payload::<DragPayload>() {
                    match drop {
                        Take::Folder(folder) => dropped = Some((Dropped::On(folder), payload)),
                        Take::More => dropped = Some((Dropped::More, payload)),
                        Take::New => {
                            // Kept, not filed: the pill becomes the box for the name.
                            pill.phase = Phase::Naming;
                            pill.since = now;
                            pill.naming = Some((payload, String::new()));
                        }
                    }
                }
            }
        }
    }

    // The box for the name, once the folders have gone: a folder, and the keyboard beside it.
    if let Some((carried, mut name)) = pill.naming.take().filter(|_| pill.phase == Phase::Naming) {
        let shown = ease_out((now - pill.since - OUT) / LETTER_IN);
        let rise = (1.0 - shown) * LETTER_RISE;
        let left = rect.left() + END + PAD;
        let icon = egui::Rect::from_min_size(egui::pos2(left, middle - glyphs::SIZE * 0.5 + rise), egui::Vec2::splat(glyphs::SIZE));
        glyphs::paint(ui, icon, Glyph::Folder, glyphs::SIZE, text.gamma_multiply(shown));
        let at = icon.right() + ICON_GAP;
        let field = egui::Rect::from_min_max(
            egui::pos2(at, middle - CHIP_HEIGHT * 0.5 + rise),
            egui::pos2((rect.right() - END - PAD).max(at), middle + CHIP_HEIGHT * 0.5 + rise),
        );
        let response = ui
            .scope(|ui| {
                // The pill stands over the foot of the list, outside what the list's own area shows.
                ui.set_clip_rect(rect.intersect(ctx.content_rect()));
                ui.multiply_opacity(shown);
                ui.put(
                    field,
                    egui::TextEdit::singleline(&mut name)
                        .id(field_id())
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .vertical_align(egui::Align::Center)
                        .text_color(text)
                        .hint_text(hint)
                        .desired_width(field.width()),
                )
            })
            .inner;
        // The keyboard is in it from the first frame it is drawn; when it leaves, so does the box.
        if !response.has_focus() && !response.lost_focus() {
            response.request_focus();
        }
        if response.lost_focus() {
            let entered = ui.input(|i| i.key_pressed(egui::Key::Enter));
            // Spent here: the list, which hears the keyboard again now the box is going, must not take
            // the same Enter as a request to open the row it carried.
            ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            let typed = name.trim();
            if entered && !typed.is_empty() {
                // A name a folder already has, in other capitals, is that folder.
                let folder = targets
                    .iter()
                    .filter_map(|target| target.folder.as_deref())
                    .find(|f| f.to_lowercase() == typed.to_lowercase())
                    .unwrap_or(typed)
                    .to_owned();
                dropped = Some((Dropped::On(Some(folder)), carried));
            }
            pill.phase = Phase::Closing;
            pill.since = now;
        } else {
            pill.naming = Some((carried, name));
        }
    }

    if hand_over && matches!(&dropped, Some((Dropped::On(Some(_)), _))) {
        // The island takes it from here, at this width.
        crate::ui::island::seed(&ctx, rect.width());
        ctx.data_mut(|d| d.remove::<Pill>(id()));
        return (Some(rect), dropped);
    }

    let springing = [pill.width, pill.height, pill.tone].iter().any(|s| (s.x - s.to).abs() > 0.001 || s.v.abs() > 0.01);
    let arriving = now - pill.since < CHIPS_DELAY + items as f64 * STAGGER + LETTER_IN;
    let naming = pill.phase == Phase::Naming && now - pill.since < OUT + LETTER_IN;
    if springing || arriving || naming || pill.phase == Phase::Closing {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id(), pill));
    (Some(rect), dropped)
}

/// Where chip `k` stood last frame: the pointer is tested against it before this frame lays it out.
fn pill_slot(ctx: &egui::Context, k: usize) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp::<egui::Rect>(id().with(("slot", k))))
}
