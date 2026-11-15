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
pub const RELEASE: &str = "0.0.9";

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
            icon: Glyph::Rename,
            title: s("Expansiones que preguntan", "Expansions that ask", "Mga expansion na nagtatanong", "पूछने वाले विस्तार"),
            line: s(
                "Selecciona una palabra del texto y conviértela en pregunta: Khipu te la hace cada vez que uses la expansión, y hasta adivina cómo llamarla. Para verlo con un ejemplo:",
                "Select a word in the text and turn it into a question: Khipu asks it each time you use the expansion, and even guesses what to call it. To see it with an example:",
                "Pumili ng salita sa teksto at gawin itong tanong: itatanong ito ng Khipu tuwing gagamitin mo ang expansion, at hinuhulaan pa nito ang itatawag dito. Para makita sa isang halimbawa:",
                "टेक्स्ट में कोई शब्द चुनें और उसे सवाल बना दें: हर बार विस्तार उपयोग करने पर Khipu वह पूछता है, और उसका नाम भी खुद सोच लेता है। एक उदाहरण से देखने के लिए:",
            ),
            path: vec![
                icon(Glyph::Add),
                named(None, in_lang(lang, "Ver un ejemplo", "See an example", "Tingnan ang halimbawa", "उदाहरण देखें")),
            ],
        },
        Item {
            icon: Glyph::Edit,
            title: s("Clic derecho", "Right-click", "Right-click", "राइट-क्लिक"),
            line: s(
                "Sobre una expansión: editarla, cambiar su disparador, moverla a una carpeta o borrarla, sin abrirla. Sobre su texto: cortar, copiar y pegar.",
                "On an expansion: edit it, change its trigger, move it to a folder or delete it, without opening it. On its text: cut, copy and paste.",
                "Sa isang expansion: i-edit, palitan ang trigger, ilipat sa folder o burahin, nang hindi ito binubuksan. Sa teksto nito: i-cut, kopyahin at i-paste.",
                "किसी विस्तार पर: उसे खोले बिना संपादित करें, ट्रिगर बदलें, फ़ोल्डर में ले जाएँ या हटाएँ। उसके टेक्स्ट पर: काटें, कॉपी करें और चिपकाएँ।",
            ),
            path: vec![],
        },
        Item {
            icon: Glyph::MultiSelect,
            title: s("Varias de un tirón", "Several at once", "Marami nang sabay-sabay", "एक साथ कई"),
            line: s(
                "Arrastra sobre el espacio vacío de la lista, como en el Explorador: cada expansión que toque el recuadro queda elegida, lista para moverla o borrarla.",
                "Drag across empty space in the list, as in File Explorer: every expansion the box touches is picked, ready to move or delete.",
                "I-drag sa bakanteng bahagi ng listahan, gaya ng sa File Explorer: mapipili ang bawat expansion na madaanan ng kahon, handang ilipat o burahin.",
                "सूची की खाली जगह पर खींचें, जैसे फ़ाइल एक्सप्लोरर में: बॉक्स जिस भी विस्तार को छूता है वह चुना जाता है, ले जाने या हटाने के लिए तैयार।",
            ),
            path: vec![],
        },
        Item {
            icon: Glyph::Pause,
            title: s("No expandir en…", "Don't expand in…", "Huwag mag-expand sa…", "इनमें विस्तार न करें…"),
            line: s(
                "Tus expansiones se quedan quietas en los programas que elijas, como un juego o una terminal. En todos los demás siguen igual.",
                "Your expansions stay quiet in the programs you choose, such as a game or a terminal. Everywhere else they work as always.",
                "Tahimik ang mga expansion mo sa mga program na pipiliin mo, gaya ng laro o terminal. Sa iba, gumagana pa rin sila gaya ng dati.",
                "आपके विस्तार आपके चुने हुए प्रोग्राम में शांत रहते हैं, जैसे कोई गेम या टर्मिनल। बाकी हर जगह वे पहले जैसे काम करते हैं।",
            ),
            path: vec![
                named(Some(Glyph::Settings), t.settings_title),
                named(None, in_lang(lang, "No expandir en", "Don't expand in", "Huwag mag-expand sa", "इनमें विस्तार न करें")),
            ],
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
