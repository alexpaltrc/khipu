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

//! «Novedades»: the card that rises once after an update, with the few things worth trying.
//!
//! Not the changelog. A handful of things at most, each a line on where to start, so that reading it
//! takes less time than closing it unread. Whatever did not make someone want to try it stays in
//! the changelog.
//!
//! The invariant: **it closes only when asked to.** The ✕ or «¡Genial!» (Enter is «¡Genial!»); a
//! click on the dimmed window behind does nothing, so it cannot be lost to a stray click before it
//! was read. Once closed, [`RELEASE`] is written to the settings and it does not come back until a
//! version with new things changes that constant.
//!
//! **Not here:** deciding whether this install is new (a first run gets the first-run screen, and
//! [`AppState::finish_onboarding`] marks these as seen), and anything about the list behind it.

use crate::app::{accent, AppState};
use crate::ui::controls::{self, Tone};
use crate::ui::glyphs::{self, Glyph};
use crate::ui::studio::in_lang;

/// The release these notes are for. Changing it is what shows the card again, to everyone.
pub const RELEASE: &str = "0.0.10";

/// The widest the card's contents get.
const WIDEST: f32 = 400.0;

/// Seconds the card takes to rise into place, and to drop away.
const RISE: f64 = 0.42;
const FALL: f64 = 0.24;

/// One new thing: its icon, what it is called, where to start, and — when the place is a few
/// clicks deep — the way there, drawn with the very icons the person will be looking for.
struct Item {
    icon: Glyph,
    title: String,
    line: String,
    path: Vec<Step>,
}

/// One stop on the way to a new thing: the control's icon, its name, or both, as it looks on screen.
struct Step {
    glyph: Option<Glyph>,
    label: Option<String>,
}

fn icon(glyph: Glyph) -> Step {
    Step { glyph: Some(glyph), label: None }
}

fn named(glyph: Option<Glyph>, label: &str) -> Step {
    Step { glyph, label: Some(label.to_string()) }
}

fn items(state: &AppState) -> Vec<Item> {
    let lang = state.settings.lang;
    let t = state.t();
    let s = |es, en, fil, hi| in_lang(lang, es, en, fil, hi).to_string();
    vec![
        Item {
            icon: Glyph::Calendar,
            title: s("Fechas que se ven", "Dates you can see", "Mga petsang nakikita", "दिखने वाली तारीखें"),
            line: s(
                "Las expansiones con fecha u hora muestran la de hoy, tal como saldrá al escribirlas.",
                "Expansions with a date or time show today's, just as it will come out when you type them.",
                "Ipinapakita ng mga expansion na may petsa o oras ang sa ngayon, gaya ng lalabas kapag tinype mo.",
                "तारीख या समय वाले विस्तार आज का मान दिखाते हैं, ठीक वैसा जैसा टाइप करने पर आएगा।",
            ),
            path: vec![],
        },
        Item {
            icon: Glyph::Undo,
            title: s("Deshacer y rehacer", "Undo and redo", "I-undo at i-redo", "पूर्ववत और फिर से करें"),
            line: s(
                "Ctrl+Z deshace tu último cambio y Ctrl+Y lo rehace, paso a paso. Por eso borrar ya no pregunta: si te equivocas, se deshace.",
                "Ctrl+Z undoes your last change and Ctrl+Y redoes it, step by step. That is why deleting no longer asks: if you slip, undo it.",
                "Ina-undo ng Ctrl+Z ang huli mong binago at ibinabalik ito ng Ctrl+Y, isa-isa. Kaya hindi na nagtatanong ang pagbura: kapag nagkamali ka, i-undo lang.",
                "Ctrl+Z आपका पिछला बदलाव पूर्ववत करता है और Ctrl+Y उसे फिर से करता है, एक-एक कदम। इसलिए हटाना अब पूछता नहीं: गलती हो तो पूर्ववत कर दें।",
            ),
            path: vec![icon(Glyph::More), named(Some(Glyph::Undo), t.undo)],
        },
        Item {
            icon: Glyph::MultiSelect,
            title: s("Varias, a la vista", "Several, in sight", "Marami, kita agad", "कई, नज़र के सामने"),
            line: s(
                "Al elegir varias, una píldora abajo las cuenta y tiene a mano moverlas o borrarlas.",
                "Pick several and a pill below counts them, with moving or deleting them at hand.",
                "Kapag pumili ka ng ilan, binibilang sila ng pill sa ibaba, handang ilipat o burahin.",
                "कई विस्तार चुनने पर नीचे एक पिल गिनती दिखाती है और उन्हें ले जाना या हटाना पास रखती है।",
            ),
            path: vec![],
        },
        Item {
            icon: Glyph::Paste,
            title: s("Tal como lo escribiste", "Just as you wrote it", "Gaya ng isinulat mo", "जैसा आपने लिखा"),
            line: s(
                "Las líneas en blanco entre párrafos llegan intactas, y el cursor queda justo después de la última palabra, listo para seguir.",
                "Blank lines between paragraphs arrive intact, and the cursor lands right after the last word, ready to carry on.",
                "Buo ang mga blangkong linya sa pagitan ng mga talata, at napupunta ang cursor sa dulo ng huling salita.",
                "अनुच्छेदों के बीच की खाली पंक्तियाँ ज्यों की त्यों पहुँचती हैं, और कर्सर ठीक आखिरी शब्द के बाद रुकता है, आगे लिखने को तैयार।",
            ),
            path: vec![],
        },
    ]
}

/// The way to a new thing, as one stamped tag: each control's own Fluent icon and name, joined by
/// a quiet chevron. Typed characters («⋯ › Vista») set in body text read as punctuation, not as
/// the buttons they stand for.
fn path(ui: &mut egui::Ui, steps: &[Step], is_light: bool) {
    let ink = ui.visuals().text_color();
    let quiet = crate::app::text_tertiary(is_light);
    controls::tag_frame(is_light).show(ui, |ui| {
        // On a narrow window the way breaks between its stops rather than running past the card.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP_TIGHT + 2.0;
            for (i, step) in steps.iter().enumerate() {
                if i > 0 {
                    glyphs::show(ui, Glyph::ChevronRight, 10.0, quiet);
                }
                if let Some(glyph) = step.glyph {
                    glyphs::show(ui, glyph, 14.0, ink);
                }
                if let Some(label) = &step.label {
                    ui.label(egui::RichText::new(label).color(ink));
                }
            }
        });
    });
}

fn opened_id() -> egui::Id {
    egui::Id::new("whats-new-opened")
}

fn closing_id() -> egui::Id {
    egui::Id::new("whats-new-closing")
}

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if !state.whats_new_open {
        return;
    }
    let lang = state.settings.lang;
    let now = ctx.input(|i| i.time);
    // Windows' animation effects turned off means no motion here either.
    let moving = ctx.global_style().animation_time > 0.0;
    let opened = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(opened_id(), || now));
    let closing = ctx.data(|d| d.get_temp::<f64>(closing_id()));
    let progress = match (moving, closing) {
        (false, Some(_)) => 0.0,
        (false, None) => 1.0,
        (true, Some(at)) => 1.0 - ((now - at) / FALL).clamp(0.0, 1.0),
        (true, None) => ((now - opened) / RISE).clamp(0.0, 1.0),
    } as f32;
    if closing.is_some() && progress <= 0.0 {
        ctx.data_mut(|d| {
            d.remove::<f64>(opened_id());
            d.remove::<f64>(closing_id());
        });
        state.dismiss_whats_new();
        return;
    }
    // Eased out on the way up, so it arrives gently; the same curve run backwards is eased in on
    // the way down, so it leaves the way a sheet is pulled away.
    let eased = 1.0 - (1.0 - progress).powi(3);
    if progress < 1.0 || closing.is_some() {
        ctx.request_repaint();
    }

    let below = ctx.content_rect().height() * 0.6;
    let id = egui::Id::new("whats-new");
    let area = egui::Modal::default_area(id)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, (1.0 - eased) * below))
        .fade_in(false);
    let mut close = false;
    controls::dialog(ctx, "whats-new")
        .area(area)
        .backdrop_color(egui::Color32::from_black_alpha((100.0 * eased) as u8))
        .show(ctx, |ui| {
            ui.set_opacity(eased.min(1.0));
            ui.set_width(controls::dialog_width(ctx, WIDEST));
            let is_light = !ui.visuals().dark_mode;
            let ink = accent(ui.visuals());

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(controls::h1(in_lang(lang, "Novedades", "What's new", "Mga bago", "नया क्या है")));
                    ui.label(controls::muted(format!("Khipu {RELEASE}"), is_light));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let tip = in_lang(lang, "Cerrar", "Close", "Isara", "बंद करें");
                    close |= controls::icon_button(ui, Glyph::Close, tip, true).clicked();
                });
            });
            ui.add_space(controls::GAP_STACK);

            // The header and «¡Genial!» always show; on a window too short for every item, the
            // items scroll between them rather than the card running past the window's edges.
            let footer = controls::GAP + controls::FIELD_HEIGHT + ui.spacing().item_spacing.y * 2.0;
            let room = ctx.content_rect().height()
                - 2.0 * (controls::DIALOG_CLEARANCE + f32::from(controls::PAGE_MARGIN))
                - ui.min_rect().height()
                - footer;
            // A bar of its own, beside the words rather than over their last letters, seen only
            // while the pointer is over them.
            ui.spacing_mut().scroll = controls::quiet_scroll();
            let scrolled = egui::ScrollArea::vertical().max_height(room.max(controls::FIELD_HEIGHT * 3.0)).show(ui, |ui| {
                let items = items(state);
                let last = items.len().saturating_sub(1);
                for (i, item) in items.into_iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = controls::GAP_WIDE;
                        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(controls::FIELD_HEIGHT), egui::Sense::hover());
                        glyphs::paint(ui, rect, item.icon, 22.0, ink);
                        ui.vertical(|ui| {
                            ui.label(controls::h3(item.title));
                            ui.add_space(controls::GAP_TIGHT);
                            ui.add(egui::Label::new(controls::muted(item.line, is_light)).wrap());
                            if !item.path.is_empty() {
                                ui.add_space(controls::GAP);
                                path(ui, &item.path, is_light);
                            }
                        });
                    });
                    if i < last {
                        ui.add_space(controls::GAP_SECTION);
                    }
                }
            });
            // Where the items run on past an edge, they fade into the card there instead of
            // being sliced through a line of text.
            let view = scrolled.inner_rect;
            let top = scrolled.state.offset.y;
            let hidden_below = scrolled.content_size.y - top - view.height();
            let card = crate::app::win_card_for(is_light);
            let reach = controls::FIELD_HEIGHT * 1.5;
            for (edge, hidden) in [(view.top(), top), (view.bottom(), hidden_below)] {
                let strength = (hidden / reach).clamp(0.0, 1.0);
                if strength <= 0.0 {
                    continue;
                }
                let inner = if edge == view.top() { edge + reach } else { edge - reach };
                let solid = card.gamma_multiply(strength);
                let clear = egui::Color32::TRANSPARENT;
                let mut mesh = egui::Mesh::default();
                mesh.colored_vertex(egui::pos2(view.left(), edge), solid);
                mesh.colored_vertex(egui::pos2(view.right(), edge), solid);
                mesh.colored_vertex(egui::pos2(view.right(), inner), clear);
                mesh.colored_vertex(egui::pos2(view.left(), inner), clear);
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(mesh);
            }
            ui.add_space(controls::GAP_SECTION);

            ui.add_space(controls::GAP);
            controls::dialog_buttons(ui, |ui| {
                let great = in_lang(lang, "¡Genial!", "Great!", "Ayos!", "बढ़िया!");
                close |= controls::button(ui, great, Tone::Primary, true).clicked();
            });
        });
    // Enter is «¡Genial!»; Esc and a click outside are left alone on purpose. See the module doc.
    close |= ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if close && closing.is_none() {
        ctx.data_mut(|d| d.insert_temp(closing_id(), now));
        ctx.request_repaint();
    }
}

/// Whether the studio should show the card: only when asked, so that every other capture is of
/// the screen it was meant to be of.
pub fn studio_wants_it() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("preview-whats-new")))
        .is_some_and(|flag| flag.exists())
}

#[cfg(test)]
mod tests {
    /// A release goes out with its own card. The commit that drops `-dev` from the version fails
    /// here until [`super::RELEASE`] — and the items under it — are this release's: a card left
    /// on the last one tells everyone who updates that nobody looked.
    #[test]
    fn a_release_carries_its_own_card() {
        let version = env!("CARGO_PKG_VERSION");
        if !version.contains('-') {
            assert_eq!(super::RELEASE, version, "renew «Novedades» for {version} before releasing it");
        }
    }
}
