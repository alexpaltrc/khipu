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
use crate::theme::ThemeMode;
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

/// One new thing: its icon, what it is called, where to start, and — when the place is a few
/// clicks deep — the way there, drawn with the very icons the person will be looking for.
struct Item {
    icon: Icon,
    title: String,
    line: String,
    path: Vec<Step>,
    /// A thing that can be tried right here, without going anywhere.
    action: Option<Action>,
}

/// What an item's own button does.
#[derive(Clone, Copy)]
enum Action {
    /// Put «Ultra oscuro» on; once on, the same button takes back the theme worn before.
    UltraDark,
}

/// One stop on the way to a new thing: the control's icon, its name, or both, as it looks on screen.
struct Step {
    glyph: Option<Glyph>,
    label: Option<String>,
}

/// What stands beside a new thing's title: a Fluent icon, or — for a change that is itself a
/// key — that key, drawn as one.
enum Icon {
    Glyph(Glyph),
    Key(&'static str),
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
    let shortcut = state.search_shortcut.label(lang);
    let s = |es, en, fil, hi| in_lang(lang, es, en, fil, hi).to_string();
    vec![
        Item {
            icon: Icon::Glyph(Glyph::ViewGrid),
            title: s("Vista en cuadrícula", "Grid view", "Grid na view", "ग्रिड दृश्य"),
            line: s(
                "Tus expansiones como tarjetas, más fáciles de leer y ordenar. Ya la tienes puesta; las otras vistas siguen aquí:",
                "Your expansions as cards, quicker to read and to arrange. It is already on; the other views are still here:",
                "Ang mga expansion mo bilang mga card, mas madaling basahin at ayusin. Naka-on na ito; narito pa rin ang ibang view:",
                "आपके विस्तार कार्ड के रूप में, पढ़ने और व्यवस्थित करने में आसान। यह पहले से चालू है; बाकी दृश्य यहाँ हैं:",
            ),
            path: vec![icon(Glyph::More), named(Some(Glyph::View), t.view_label)],
            action: None,
        },
        ultra_dark(state),
        Item {
            icon: Icon::Glyph(Glyph::Rename),
            title: s("Formularios", "Forms", "Mga form", "फ़ॉर्म"),
            line: s(
                "Una expansión que te pregunta los datos antes de escribir. Para crear una:",
                "An expansion that asks you for the details before it types. To make one:",
                "Isang expansion na nagtatanong muna ng detalye bago mag-type. Para gumawa ng isa:",
                "ऐसा विस्तार जो लिखने से पहले आपसे जानकारी पूछता है। एक बनाने के लिए:",
            ),
            path: vec![
                icon(Glyph::Add),
                named(Some(Glyph::Add), in_lang(lang, "Añadir", "Add", "Magdagdag", "जोड़ें")),
                named(
                    None,
                    in_lang(lang, "Un campo para rellenar", "A field to fill in", "Isang field na pupunan", "भरने के लिए एक फ़ील्ड"),
                ),
            ],
            action: None,
        },
        Item {
            icon: Icon::Glyph(Glyph::StarFilled),
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
            path: vec![],
            action: None,
        },
        Item {
            icon: Icon::Key(";"),
            title: s("Nuevo prefijo «;»", "A new prefix: “;”", "Bagong prefix: “;”", "नया उपसर्ग: “;”"),
            line: s(
                "Outlook, Teams y otras apps convierten «:palabra» en emoji antes de que salga la expansión; por eso Khipu, recién instalado, empieza con «;». Tus expansiones siguen tal cual. Si quieres el nuevo prefijo:",
                "Outlook, Teams and other apps turn “:word” into an emoji before the expansion can appear, so a fresh Khipu now starts with “;”. Your expansions are just as they were. If you want the new prefix:",
                "Ginagawang emoji ng Outlook, Teams at iba pang app ang “:salita” bago lumabas ang expansion, kaya “;” na ang simula ng bagong install ng Khipu. Walang binago sa mga expansion mo. Kung gusto mo ang bagong prefix:",
                "Outlook, Teams और दूसरे ऐप “:शब्द” को विस्तार आने से पहले इमोजी बना देते हैं, इसलिए नया इंस्टॉल किया Khipu अब “;” से शुरू होता है। आपके विस्तार जैसे थे वैसे ही हैं। अगर आप नया उपसर्ग चाहते हैं:",
            ),
            path: vec![named(Some(Glyph::Settings), t.settings_title), named(None, t.prefix_section)],
            action: None,
        },
    ]
}

/// «Ultra oscuro», tried from right here. Someone already wearing it, not through this button, gets
/// the way to Ajustes instead: there is nothing to try, and no theme of theirs to go back to.
fn ultra_dark(state: &AppState) -> Item {
    let lang = state.settings.lang;
    let t = state.t();
    let s = |es, en, fil, hi| in_lang(lang, es, en, fil, hi).to_string();
    let found_there = state.settings.theme_mode == ThemeMode::UltraDark && state.whats_new_theme_before.is_none();
    Item {
        icon: Icon::Glyph(Glyph::Moon),
        title: s("Tema Ultra oscuro", "Ultra dark theme", "Itim na tema", "अति गहरी थीम"),
        line: s(
            "El tema oscuro sobre negro puro, ideal para pantallas OLED. Tarjetas y botones conservan su gris, para que nada se pierda.",
            "The dark theme on pure black, made for OLED screens. Cards and buttons keep their gray, so nothing gets lost.",
            "Ang madilim na tema sa purong itim, para sa mga OLED screen. Nananatiling abo ang mga card at button, kaya walang nawawala.",
            "शुद्ध काले पर गहरी थीम, OLED स्क्रीन के लिए। कार्ड और बटन अपना धूसर रंग रखते हैं, ताकि कुछ भी खो न जाए।",
        ),
        path: if found_there {
            vec![named(Some(Glyph::Settings), t.settings_title), named(None, t.appearance_section)]
        } else {
            vec![]
        },
        action: (!found_there).then_some(Action::UltraDark),
    }
}

/// The words on an [`Action`]'s button, as things stand now.
fn action_label(state: &AppState, action: Action) -> String {
    let lang = state.settings.lang;
    match action {
        Action::UltraDark => match state.whats_new_theme_before {
            Some(before) if state.settings.theme_mode == ThemeMode::UltraDark => {
                let name = crate::ui::settings_view::theme_name(state.t(), before, true);
                crate::i18n::fill(
                    in_lang(lang, "Volver a «{theme}»", "Back to “{theme}”", "Bumalik sa “{theme}”", "“{theme}” पर लौटें"),
                    &[("theme", name)],
                )
            }
            _ => in_lang(lang, "Probarlo", "Try it", "Subukan", "आज़माएँ").to_string(),
        },
    }
}

/// What an [`Action`]'s button does when pressed.
fn run(state: &mut AppState, action: Action) {
    match action {
        Action::UltraDark => match state.whats_new_theme_before {
            Some(before) if state.settings.theme_mode == ThemeMode::UltraDark => {
                state.whats_new_theme_before = None;
                state.set_theme_mode(before);
            }
            _ => {
                state.whats_new_theme_before = Some(state.settings.theme_mode);
                state.set_theme_mode(ThemeMode::UltraDark);
            }
        },
    }
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

/// A key drawn as a key: its legend in the code face inside a rounded outline, in the accent, the
/// size the Fluent icons beside it are drawn at.
fn keycap(ui: &egui::Ui, slot: egui::Rect, legend: &str, ink: egui::Color32) {
    let cap = egui::Rect::from_center_size(slot.center(), egui::Vec2::splat(24.0));
    ui.painter().rect_stroke(cap, 5.0, egui::Stroke::new(1.5, ink), egui::StrokeKind::Inside);
    ui.painter().text(cap.center(), egui::Align2::CENTER_CENTER, legend, egui::FontId::monospace(15.0), ink);
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
    let mut pressed = None;
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
                        match item.icon {
                            Icon::Glyph(glyph) => glyphs::paint(ui, rect, glyph, 22.0, ink),
                            Icon::Key(key) => keycap(ui, rect, key, ink),
                        }
                        ui.vertical(|ui| {
                            ui.label(controls::h3(item.title));
                            ui.add_space(controls::GAP_TIGHT);
                            ui.add(egui::Label::new(controls::muted(item.line, is_light)).wrap());
                            if !item.path.is_empty() {
                                ui.add_space(controls::GAP);
                                path(ui, &item.path, is_light);
                            }
                            if let Some(action) = item.action {
                                ui.add_space(controls::GAP);
                                if controls::button(ui, &action_label(state, action), Tone::Normal, true).clicked() {
                                    pressed = Some(action);
                                }
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
    if let Some(action) = pressed {
        run(state, action);
    }
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
