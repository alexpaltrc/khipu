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
//! Not the changelog. Three things at most, each one line on where to start, so that reading it
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
pub const RELEASE: &str = "0.0.7";

/// The widest the card's contents get.
const WIDEST: f32 = 400.0;

/// Seconds the card takes to rise into place, and to drop away.
const RISE: f64 = 0.42;
const FALL: f64 = 0.24;

/// One new thing: its icon, what it is called, and where to start.
struct Item {
    glyph: Glyph,
    title: String,
    line: String,
}

fn items(state: &AppState) -> Vec<Item> {
    let lang = state.settings.lang;
    let t = state.t();
    let shortcut = state.search_shortcut.label(lang);
    let s = |es, en, fil, hi| in_lang(lang, es, en, fil, hi).to_string();
    vec![
        Item {
            glyph: Glyph::ViewGrid,
            title: s("Vista en cuadrícula", "Grid view", "Grid na view", "ग्रिड दृश्य"),
            line: crate::i18n::fill(
                in_lang(
                    lang,
                    "Tus expansiones como tarjetas, más fáciles de leer y ordenar. Ya la tienes puesta; las otras están en ⋯ › {view}.",
                    "Your expansions as cards, quicker to read and to arrange. It is already on; the others are in ⋯ › {view}.",
                    "Ang mga expansion mo bilang mga card, mas madaling basahin at ayusin. Naka-on na ito; nasa ⋯ › {view} ang iba.",
                    "आपके विस्तार कार्ड के रूप में, पढ़ने और व्यवस्थित करने में आसान। यह पहले से चालू है; बाकी ⋯ › {view} में हैं।",
                ),
                &[("view", t.view_label)],
            ),
        },
        Item {
            glyph: Glyph::Rename,
            title: s("Formularios", "Forms", "Mga form", "फ़ॉर्म"),
            line: s(
                "Una expansión que te pregunta los datos antes de escribir. Crea una con + y pulsa «Insertar campo».",
                "An expansion that asks you for the details before it types. Create one with + and press «Insert field».",
                "Isang expansion na nagtatanong muna ng detalye bago mag-type. Gumawa gamit ang + at pindutin ang «Magdagdag ng field».",
                "ऐसा विस्तार जो लिखने से पहले आपसे जानकारी पूछता है। + से एक बनाएँ और «फ़ील्ड जोड़ें» दबाएँ।",
            ),
        },
        Item {
            glyph: Glyph::StarFilled,
            title: s("Favoritos", "Favorites", "Mga paborito", "पसंदीदा"),
            line: crate::i18n::fill(
                in_lang(
                    lang,
                    "Marca con la estrella las que más usas: salen primero en el buscador ({keys}), y la estrella de arriba te muestra solo esas.",
                    "Star the ones you use most: they come first in search ({keys}), and the star at the top shows only them.",
                    "Lagyan ng bituin ang mga madalas mong gamitin: mauuna sila sa paghahanap ({keys}), at ang bituin sa itaas ay sila lang ang ipapakita.",
                    "जिनका आप सबसे ज़्यादा उपयोग करते हैं उन पर तारा लगाएँ: वे खोज ({keys}) में सबसे पहले आएँगे, और ऊपर का तारा सिर्फ़ उन्हें दिखाता है।",
                ),
                &[("keys", &shortcut)],
            ),
        },
    ]
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

            for item in items(state) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = controls::GAP_WIDE;
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(controls::FIELD_HEIGHT), egui::Sense::hover());
                    glyphs::paint(ui, rect, item.glyph, 22.0, ink);
                    ui.vertical(|ui| {
                        ui.label(controls::h3(item.title));
                        ui.add_space(controls::GAP_TIGHT);
                        ui.add(egui::Label::new(controls::muted(item.line, is_light)).wrap());
                    });
                });
                ui.add_space(controls::GAP_SECTION);
            }

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
