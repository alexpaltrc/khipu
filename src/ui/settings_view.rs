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


//! Ajustes: a dialog in the middle of the window, over the dimmed screen behind it. It is not a
//! screen, and it cannot be moved.
//!
//! **Two icons and one scroll.** The theme and the language are icons beside the cross: the theme
//! turns over with each click, the language opens its four. Under them Al escribir, Buscador and
//! Tus expansiones, each a card of rows the way Windows' own settings are laid out: a setting's
//! name, one quiet line on what it is for, and its control at the right edge. One line and no
//! more — the whole story of each lives in the guide (Consejos → Ajustes). A row of options too
//! wide to sit beside its name (the prefixes) goes under it, across the card.
//!
//! Not here on purpose: starting with Windows (always on; Windows has its own switch, see
//! [`crate::autostart`]) and favorites first (always).
//!
//! Everything on it is a call into `AppState`, which is what saves and reports; nothing is written
//! from here. What `AppState` says while the dialog is open comes back as
//! [`AppState::settings_note`] and the island says it over the dialog's foot, because the banner is
//! under the backdrop.
//!
//! It closes with Esc (handled in `App::logic`, after the folder picker it opens), with the cross,
//! or with a click on the dimmed window.

use crate::app::{text_tertiary, AppState, TransferDirection};
use crate::i18n::{fill, fill_count, Lang};
use crate::settings::PREFIX_SUGGESTIONS;
use crate::theme::ThemeMode;
use crate::ui::controls::{self, Tone};
use crate::ui::glyphs::{self, Glyph};
use crate::ui::island;
use crate::ui::keys;
use crate::ui::studio;

/// The widest the dialog's contents get. Enough for every theme in a row inside its card, and for
/// a setting's words beside the shortcut box; less on a small window, through
/// [`controls::dialog_width`].
const WIDEST: f32 = 480.0;

/// Between one card and the next group's name.
const GROUP_GAP: f32 = 20.0;

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    if !state.settings_open {
        // However it was closed, a half-typed prefix is not kept for next time: the field starts
        // again from the saved one.
        ctx.data_mut(|d| {
            d.remove::<String>(custom_prefix_id());
            d.remove::<bool>(prefix_other_id());
            d.insert_temp(fresh_id(), true);
        });
        island::forget_settings(ctx);
        return;
    }
    // Opened at the top every time. egui keeps a scroll position across frames and across runs,
    // and a dialog that opens half way down its own contents hides the first thing it offers.
    // Only the first frame finds the flag; every later one must leave the scroll to the wheel.
    let fresh = ctx.data_mut(|d| d.remove_temp::<bool>(fresh_id())).unwrap_or(false);
    let t = state.t();
    let lang = state.settings.lang;
    let mut close = false;

    let modal = controls::dialog(ctx, "settings").show(ctx, |ui| {
        ui.set_width(controls::dialog_width(ctx, WIDEST));
        // Everything under the title scrolls, so the smallest window still reaches every setting
        // and the dialog never runs past the window's edge. The title and the cross stay put.
        //
        // The room is set outright, before anything is laid out. A modal's area offers its contents
        // last frame's size, so a scroll area left to measure what is available never grows past
        // the height it opened at.
        let frame = 2.0 * f32::from(controls::PAGE_MARGIN);
        let room = (ctx.content_rect().height() - frame - 2.0 * controls::DIALOG_CLEARANCE).max(160.0);
        ui.set_max_height(room);

        ui.horizontal(|ui| {
            ui.label(controls::h3(t.settings_title));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tip = keys::tip(t.banner_close_tip, keys::BACK, lang);
                if controls::icon_button(ui, Glyph::Close, &tip, true).clicked() {
                    close = true;
                }
                language_button(ui, state);
                theme_button(ui, state);
            });
        });
        ui.add_space(controls::GAP_WIDE);

        // A bar that takes its own column, and only when the window is too short to need none:
        // floating over the contents, it sat on top of the controls at the right edge. In
        // that column it shows only while the pointer is over the page.
        ui.spacing_mut().scroll = controls::quiet_scroll();
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("settings")
            .max_height(room - TITLE_ROW)
            .auto_shrink([false, true]);
        if fresh {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        // When a note makes the contents taller than the dialog, the bar's column opens and
        // everything at the right edge makes way for it. egui eases that column in over the
        // app-wide animation time, a fifth of a second, on a curve that covers half the distance
        // in the first fifth of it: forty milliseconds, which reads as a jump. The scroll area
        // reads that time from the global style both before and after it lays out its contents,
        // so it is lengthened exactly there and put back for the contents in between: the
        // column opens slowly and the switches inside keep their own pace. Zero stays zero, so
        // Windows' animation effects turned off still means no motion here either.
        let normal = ctx.global_style().animation_time;
        let make_way = if normal > 0.0 { BAR_MAKES_WAY } else { 0.0 };
        ctx.global_style_mut(|s| s.animation_time = make_way);
        scroll.show(ui, |ui| {
            ctx.global_style_mut(|s| s.animation_time = normal);
            ui.set_width(ui.available_width());
            sections(ui, state);
            ctx.global_style_mut(|s| s.animation_time = make_way);
        });
        ctx.global_style_mut(|s| s.animation_time = normal);

        // What the last action said — an export's count, an import's result, a refusal — rises
        // over the foot of the dialog, wherever the page is scrolled to.
        let page = ui.min_rect();
        let foot = egui::pos2(page.center().x, page.bottom() - controls::GAP_WIDE);
        island::show(ui, state, island::Source::Settings(foot), page.width() - 2.0 * controls::GAP_WIDE, false);
    });

    if close || modal.backdrop_response.clicked() {
        state.close_settings();
    }
}

/// Seconds the scroll bar's column takes to open or close. The curve is egui's, eased out, so
/// half the way is covered in the first tenth of a second and the rest settles gently.
const BAR_MAKES_WAY: f32 = 0.5;

/// The title row and the space under it.
const TITLE_ROW: f32 = controls::FIELD_HEIGHT + controls::GAP_WIDE;

/// How long the hidden theme is pointed out after it has been found.
pub const SPOTLIGHT: std::time::Duration = std::time::Duration::from_secs(4);

/// One beat of the halo while it is pointed out, in seconds.
const SPOTLIGHT_BEAT: f32 = 1.2;

/// Seconds one theme's icon takes to give way to the next.
const THEME_SWAP: f32 = 0.22;

/// A theme's name, for the button's tip and for a screen reader.
pub fn theme_name(t: &crate::i18n::Strings, mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => t.theme_system,
        ThemeMode::Light => t.theme_light,
        ThemeMode::Dark => t.theme_dark,
        ThemeMode::UltraDark => t.theme_ultra_dark,
        ThemeMode::Khipu => t.theme_khipu,
    }
}

/// What the button shows for each theme: the sun and the moon say light and dark without a word.
fn theme_glyph(mode: ThemeMode) -> Glyph {
    match mode {
        ThemeMode::System => Glyph::ThemeSystem,
        ThemeMode::Light => Glyph::Sun,
        ThemeMode::Dark => Glyph::Moon,
        ThemeMode::UltraDark => Glyph::MoonFilled,
        ThemeMode::Khipu => Glyph::Sparkle,
    }
}

/// The theme one click moves on to: sistema, claro, oscuro, ultra oscuro and, once found in
/// Consejos, «I got chiiills», then round again.
fn next_theme(mode: ThemeMode, found: bool) -> ThemeMode {
    let mut modes = ThemeMode::ALL.to_vec();
    if found {
        modes.push(ThemeMode::Khipu);
    }
    let at = modes.iter().position(|m| *m == mode).unwrap_or(0);
    modes[(at + 1) % modes.len()]
}

fn theme_swap_id() -> egui::Id {
    egui::Id::new("settings_theme_swap")
}

/// The theme, as one button beside the cross that shows the theme on and moves to the next with
/// each click. The icon on its way out shrinks and fades as the next one grows in past its size
/// and settles, so a click reads as the theme turning over rather than a picture being replaced.
///
/// Just after the hidden theme is found, the button is ringed in violet and holds the keyboard,
/// and its next click puts that theme on, wherever the turn would have gone: offered, not put on.
fn theme_button(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let mode = state.settings.theme_mode;
    let is_light = !ui.visuals().dark_mode;
    let spotlight = state.khipu_spotlight.filter(|at| at.elapsed() < SPOTLIGHT);
    if state.khipu_spotlight.is_some() && spotlight.is_none() {
        state.khipu_spotlight = None;
    }
    let tip = format!("{}: {}", t.appearance_section, theme_name(t, mode));

    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(controls::FIELD_HEIGHT), egui::Sense::click());
    let response = response.on_hover_text(&tip);
    controls::subtle_wash(ui, &response, true);

    // The keyboard is handed over once, as it is pointed out: Enter or Space then wears it. Asked
    // for on every frame it would hold the keyboard for the whole four seconds.
    if let Some(at) = spotlight {
        let focus_id = egui::Id::new("khipu-spotlight-focused");
        let first = ui.ctx().data_mut(|d| {
            let first = d.get_temp::<std::time::Instant>(focus_id) != Some(at);
            d.insert_temp(focus_id, at);
            first
        });
        if first {
            response.request_focus();
        }
    }

    let now = ui.input(|i| i.time);
    let animate = crate::theme::animations_enabled();
    let swap = ui.ctx().data(|d| d.get_temp::<(Glyph, f64)>(theme_swap_id()));
    let progress = swap.map_or(1.0, |(_, at)| ((now - at) as f32 / THEME_SWAP).clamp(0.0, 1.0));
    if progress < 1.0 {
        ui.ctx().request_repaint();
    }

    if ui.is_rect_visible(rect) {
        let ink = ui.visuals().text_color();
        let size = glyphs::SIZE;
        match swap.filter(|_| progress < 1.0) {
            Some((before, _)) => {
                // Out over the first half, eased in; in over the whole, eased out past its size.
                let out = (progress * 2.0).min(1.0);
                let gone = out * out;
                if gone < 1.0 {
                    glyphs::paint(ui, rect, before, size * (1.0 - 0.45 * gone), ink.gamma_multiply(1.0 - gone));
                }
                let grow = ease_out_back(progress);
                glyphs::paint(ui, rect, theme_glyph(mode), size * (0.55 + 0.45 * grow), ink.gamma_multiply(progress.min(1.0)));
            }
            None => glyphs::paint(ui, rect, theme_glyph(mode), size, ink),
        }

        if let Some(at) = spotlight {
            let e = at.elapsed().as_secs_f32();
            let fade = 1.0 - e / SPOTLIGHT.as_secs_f32();
            let glow = if animate {
                fade * (0.6 + 0.4 * (e / SPOTLIGHT_BEAT * std::f32::consts::TAU).cos())
            } else {
                // Nothing moves: the halo is simply there, and then it is not.
                1.0
            };
            ui.ctx().request_repaint();
            let outline = crate::app::khipu_outline(is_light);
            let radius = f32::from(controls::RADIUS_CONTROL);
            // Rings fading outward, strongest against the button.
            for step in 1..=3 {
                let k = step as f32;
                let alpha = glow * (1.0 - (k - 1.0) / 3.0) * 0.55;
                ui.painter().rect_stroke(
                    rect.expand(k),
                    radius + k,
                    egui::Stroke::new(1.0, outline.gamma_multiply(alpha)),
                    egui::StrokeKind::Inside,
                );
            }
            ui.painter().rect_stroke(rect, radius, egui::Stroke::new(1.5, outline), egui::StrokeKind::Inside);
        }
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &tip));
    controls::focus_ring(ui, &response, f32::from(controls::RADIUS_CONTROL));

    if response.clicked() {
        let next = if spotlight.is_some() {
            ThemeMode::Khipu
        } else {
            next_theme(mode, state.settings.khipu_theme_found)
        };
        state.khipu_spotlight = None;
        if animate {
            ui.ctx().data_mut(|d| d.insert_temp(theme_swap_id(), (theme_glyph(mode), now)));
        }
        state.set_theme_mode(next);
    }
}

/// Eased out past the end and back: 0 to a touch over 1, settling on 1.
fn ease_out_back(x: f32) -> f32 {
    const C: f32 = 1.4;
    let x = x - 1.0;
    1.0 + (C + 1.0) * x * x * x + C * x * x
}

/// The language, as one button beside the theme's that opens the four, each written in itself so
/// someone who cannot read the current one can still find theirs.
fn language_button(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let current = state.settings.lang;
    let tip = format!("{}: {}", t.language_section, current.picker_label(ui.ctx()));
    let button = controls::icon_button(ui, Glyph::Language, &tip, true);
    let by_keyboard = button.clicked() && button.has_focus();
    let mut picked = None;
    egui::Popup::menu(&button)
        .align(egui::RectAlign::BOTTOM_END)
        .gap(controls::GAP_TIGHT)
        .width(160.0)
        .show(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let first = ui.next_auto_id();
            for lang in Lang::ALL {
                if controls::menu_check_item(ui, lang.picker_label(ui.ctx()), None, lang == current).clicked() {
                    picked = Some(lang);
                }
            }
            if by_keyboard {
                ui.memory_mut(|m| m.request_focus(first));
            }
        });
    if let Some(lang) = picked {
        state.set_language(lang);
    }
}

fn sections(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let is_light = !ui.visuals().dark_mode;

    controls::card_heading(ui, studio::text(state, "Al escribir", "While you type", "Habang nagta-type", "टाइप करते समय"));
    controls::card(ui, |ui| {
        let about = studio::text(
            state,
            "Lo que escribes antes de la palabra, como \":\" en \":hola\".",
            "What you type before the word, like \":\" in \":hello\".",
            "Ang tine-type mo bago ang salita, gaya ng \":\" sa \":kumusta\".",
            "शब्द से पहले जो आप टाइप करते हैं, जैसे \":नमस्ते\" में \":\"।",
        );
        controls::card_stacked_row(ui, t.prefix_section, about, |ui| prefix_row(ui, state));
        controls::card_divider(ui);

        excluded_apps(ui, state);
    });

    // Only while this app owns the shortcut. With espanso's own bar in charge there is nothing of
    // ours for it to change.
    if state.hotkey.is_some() {
        ui.add_space(GROUP_GAP);
        controls::card_heading(ui, studio::text(state, "Buscador", "Search", "Paghahanap", "खोज"));
        controls::card(ui, |ui| {
            let width = shortcut_box_width(ui, state) + controls::GAP + controls::FIELD_HEIGHT;
            let about = studio::text(
                state,
                "Abre el buscador desde cualquier programa.",
                "Opens search from any program.",
                "Binubuksan ang paghahanap mula sa kahit anong program.",
                "किसी भी प्रोग्राम से खोज खोलता है।",
            );
            controls::card_row(ui, t.launcher_shortcut_section, about, width, |ui| launcher_shortcut(ui, state));
            if let Some(taken) = state.shortcut_taken {
                let lang = state.settings.lang;
                let text = fill(t.launcher_shortcut_taken, &[("keys", &taken.label(lang))]);
                ui.add(egui::Label::new(egui::RichText::new(text).color(crate::app::danger(is_light))).wrap());
                ui.add_space(controls::CARD_PAD_Y);
            }
        });
    }

    ui.add_space(GROUP_GAP);
    controls::card_heading(ui, studio::text(state, "Tus expansiones", "Your expansions", "Ang iyong mga expansion", "आपके विस्तार"));
    controls::card(ui, |ui| {
        let export = studio::text(state, "Exportar", "Export", "I-export", "निर्यात");
        let import = studio::text(state, "Importar", "Import", "I-import", "आयात");
        let width = controls::button_width(ui, export) + controls::GAP + controls::button_width(ui, import);
        let about = studio::text(
            state,
            "Guárdalas en un archivo, o trae las de otra computadora.",
            "Save them to a file, or bring in the ones from another computer.",
            "I-save sa isang file, o kunin ang mula sa ibang kompyuter.",
            "इन्हें फ़ाइल में सहेजें, या दूसरे कंप्यूटर से लाएँ।",
        );
        let (exporting, importing) = controls::card_row(ui, t.transfer_section, about, width, |ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP;
            // Right to left: the second button first.
            let importing = controls::button(ui, import, Tone::Normal, true).clicked();
            let exporting = controls::button(ui, export, Tone::Normal, true).clicked();
            (exporting, importing)
        });
        if exporting {
            state.export_expansions();
        }
        if importing {
            state.import_expansions();
        }
    });

    ui.add_space(GROUP_GAP);
    let quiet = text_tertiary(is_light);
    // Espanso's version, stated and left alone. No comparison against what this build was tested
    // with, and no nudge to update: in an office the right thing is usually to stay on a version
    // that works, and a badge implying otherwise would manufacture a worry nobody needs.
    // Three steps down in weight: Khipu and its version, who made it, and — set a little apart,
    // smallest and quietest — the engine under it.
    ui.vertical_centered(|ui| {
        let khipu = format!("Khipu {}", env!("CARGO_PKG_VERSION"));
        ui.add(egui::Label::new(controls::muted(khipu, is_light)).wrap());
        ui.add(egui::Label::new(controls::small_muted(t.made_by, is_light)).wrap());
        if let Some(espanso) = &state.espanso_version {
            ui.add_space(controls::GAP);
            ui.add(egui::Label::new(egui::RichText::new(format!("Espanso {espanso}")).small().color(quiet)).wrap());
        }
    });
}

fn open_apps_id() -> egui::Id {
    egui::Id::new("settings_open_apps")
}

/// «No expandir en»: «Añadir», which offers the programs open right now, and under it the ones
/// already chosen, each with its × to take it back off. See [`crate::exclusions`].
fn excluded_apps(ui: &mut egui::Ui, state: &mut AppState) {
    let is_light = !ui.visuals().dark_mode;
    let add = studio::text(state, "Añadir", "Add", "Magdagdag", "जोड़ें");
    let width = controls::subtle_button_width(ui, true, add, true);
    let mut picked = None;
    controls::card_row(
        ui,
        studio::text(state, "No expandir en", "Don't expand in", "Huwag mag-expand sa", "इनमें विस्तार न करें"),
        studio::text(
            state,
            "Programas donde tus expansiones no se activan. Un navegador cuenta entero, con todas sus páginas.",
            "Programs where your expansions don't work. A browser counts whole, every page in it.",
            "Mga program kung saan hindi gagana ang mga expansion mo. Buo ang browser, kasama ang lahat ng page nito.",
            "वे प्रोग्राम जिनमें आपके विस्तार काम नहीं करते। ब्राउज़र पूरा गिना जाता है, उसके सभी पेज समेत।",
        ),
        width,
        |ui| {
            let menu = controls::subtle_button(ui, Some(Glyph::Add), add, Some(Glyph::ChevronDown));
            // Asked once, as it opens: walking every window on every frame it stays open would be
            // work for nothing.
            if menu.clicked() {
                let chosen = &state.settings.excluded_apps;
                let open: Vec<_> = crate::exclusions::open_apps()
                    .into_iter()
                    .filter(|app| !chosen.iter().any(|c| c.exe.eq_ignore_ascii_case(&app.exe)))
                    .collect();
                ui.data_mut(|d| d.insert_temp(open_apps_id(), std::sync::Arc::new(open)));
            }
            let by_keyboard = menu.clicked() && menu.has_focus();
            let open = ui
                .data(|d| d.get_temp::<std::sync::Arc<Vec<crate::exclusions::ExcludedApp>>>(open_apps_id()))
                .unwrap_or_default();
            let none = studio::text(
                state,
                "No hay otros programas abiertos",
                "No other programs are open",
                "Walang ibang bukas na program",
                "कोई अन्य प्रोग्राम खुला नहीं है",
            );
            egui::Popup::menu(&menu)
                .align(egui::RectAlign::BOTTOM_END)
                .gap(controls::GAP_TIGHT)
                .width(240.0)
                .show(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if open.is_empty() {
                        controls::menu_item(ui, None, none, None, Tone::Normal, false);
                        return;
                    }
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        let first = ui.next_auto_id();
                        for app in open.iter() {
                            if controls::menu_item(ui, None, &app.name, Some(&app.exe), Tone::Normal, true).clicked() {
                                picked = Some(app.clone());
                            }
                        }
                        if by_keyboard {
                            ui.memory_mut(|m| m.request_focus(first));
                        }
                    });
                });
        },
    );
    if let Some(app) = picked {
        state.exclude_app(app);
    }

    if state.settings.excluded_apps.is_empty() {
        return;
    }
    let remove = studio::text(state, "Quitar", "Remove", "Alisin", "हटाएँ");
    let mut removed = None;
    for app in &state.settings.excluded_apps {
        ui.horizontal(|ui| {
            ui.set_min_height(controls::FIELD_HEIGHT);
            ui.spacing_mut().item_spacing.x = controls::GAP;
            // One size for both, so the two words stand on one line.
            ui.add(egui::Label::new(app.name.as_str()).truncate());
            ui.add(egui::Label::new(controls::muted(app.exe.as_str(), is_light)).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if controls::icon_button(ui, Glyph::Close, remove, true).clicked() {
                    removed = Some(app.exe.clone());
                }
            });
        });
    }
    ui.add_space(controls::CARD_PAD_Y);
    if let Some(exe) = removed {
        state.include_app(&exe);
    }
}

fn prefix_other_id() -> egui::Id {
    egui::Id::new("settings_prefix_other")
}

/// The suggested prefixes and «Otro», which opens a field beside them for any other. Wrapped, so on
/// a narrow window the field drops under the row instead of running off the card.
fn prefix_row(ui: &mut egui::Ui, state: &mut AppState) {
    let mut labels: Vec<String> = PREFIX_SUGGESTIONS.iter().map(|p| (*p).to_owned()).collect();
    labels.push(other_label(state).to_string());
    let other = PREFIX_SUGGESTIONS.len();
    let chosen = PREFIX_SUGGESTIONS.iter().position(|p| *p == state.settings.prefix);
    let asked = ui.data(|d| d.get_temp::<bool>(prefix_other_id())).unwrap_or(false);
    // A prefix typed by hand is «Otro» by definition; so is the moment after «Otro» was clicked,
    // while the field still holds a suggestion.
    let selected = if asked || chosen.is_none() { other } else { chosen.unwrap_or(other) };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(controls::GAP_WIDE, controls::GAP);
        if let Some(picked) = controls::segmented_codes(ui, "prefix", &labels, other, Some(selected)) {
            if picked == other {
                // The field takes the keyboard once the click is over; see below.
                ui.data_mut(|d| {
                    d.insert_temp(prefix_other_id(), true);
                    d.insert_temp(focus_other_id(), true);
                });
                ui.ctx().request_repaint();
            } else {
                ui.data_mut(|d| d.remove::<bool>(prefix_other_id()));
                state.set_prefix(PREFIX_SUGGESTIONS[picked].to_string());
                state.offer_prefix_rename();
            }
        }
        let open = ui.data(|d| d.get_temp::<bool>(prefix_other_id())).unwrap_or(false) || chosen.is_none();
        if open {
            let pending = ui.data(|d| d.get_temp::<bool>(focus_other_id())).unwrap_or(false);
            // Not while the click that chose «Otro» is still in the input: a field handed the keyboard
            // in that frame gives it up again, to the click it was not under.
            if pending && !ui.input(|i| i.pointer.any_down() || i.pointer.any_released()) {
                ui.data_mut(|d| d.remove::<bool>(focus_other_id()));
                ui.memory_mut(|m| m.request_focus(custom_prefix_field_id()));
            }
            custom_prefix(ui, state);
        }
    });
}

fn focus_other_id() -> egui::Id {
    egui::Id::new("settings_prefix_other_focus")
}

/// «Otro», the segment for a prefix none of the suggestions offers.
fn other_label(state: &AppState) -> &'static str {
    studio::text(state, "Otro", "Other", "Iba", "अन्य")
}

/// Set while the dialog is closed, so the frame that opens it knows it is the first.
fn fresh_id() -> egui::Id {
    egui::Id::new("settings_fresh")
}

fn custom_prefix_id() -> egui::Id {
    egui::Id::new("settings_custom_prefix")
}

fn custom_prefix_field_id() -> egui::Id {
    egui::Id::new("settings_custom_prefix_field")
}

/// The field for a prefix none of the suggestions offers, opened by «Otro».
fn custom_prefix(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        // The field keeps its own buffer while it is being typed in, instead of being re-cloned
        // from the committed prefix on every frame.
        //
        // `set_prefix` refuses an empty value — rightly, since every trigger needs one — and
        // the old field then redrew itself from the prefix that was still stored, while egui's
        // cursor stayed at the start of what the user had just cleared. Backspacing ":" away
        // and typing ";" put the new character *in front of* the old one: the prefix became
        // ";:", saved without a word, and every expansion created afterwards took it.
        //
        // Nothing is committed until it is usable, so the empty moment in the middle of an edit
        // now stays on screen, which is what it always looked like it was doing.
        let buffer_id = custom_prefix_id();
        let mut custom = ui
            .data(|d| d.get_temp::<String>(buffer_id))
            .unwrap_or_else(|| state.settings.prefix.clone());
        let response = ui.add(controls::text_field(&mut custom).id(custom_prefix_field_id()).desired_width(64.0));
        if response.changed() {
            ui.data_mut(|d| d.insert_temp(buffer_id, custom.clone()));
            if !custom.trim().is_empty() {
                // Still one save per keystroke, and deliberately so: a prefix is one to three
                // characters typed once in a blue moon, and deferring the commit to `lost_focus`
                // would silently drop the edit for anyone who closes this dialog with Esc.
                state.set_prefix(custom);
            }
        }
        // Dropped as soon as the field is done with, so it goes back to following the stored
        // prefix — including when that was changed from the row of suggestions just beside it.
        if response.lost_focus() {
            ui.data_mut(|d| d.remove::<String>(buffer_id));
            // Asked once the prefix is finished, not at every key of it.
            state.offer_prefix_rename();
        }
    });
}

/// The shortcut box is never narrower than this, so it does not jump in width between the prompt,
/// the keys being held and the shortcut.
const SHORTCUT_BOX_MIN: f32 = 200.0;

/// Room inside the shortcut box on each side of its words, the same as a text field's.
const SHORTCUT_BOX_PAD: f32 = 12.0;

/// The shortcut that opens the search window: a box that listens when clicked, as a game's key
/// settings do, and the button that puts back the default.
///
/// While listening, every key goes to the box and nowhere else (see [`crate::hotkey`]): the
/// modifiers show as they are held, the first other key or extra mouse button ends it, and Esc,
/// a click anywhere else or leaving the window abandons it.
fn launcher_shortcut(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let lang = state.settings.lang;
    let capturing = state.shortcut_capture;
    let default = crate::hotkey::DEFAULT;

    // Laid out right to left, from the card's edge: the reset button there, the box before it.
    ui.spacing_mut().item_spacing.x = controls::GAP;
    let tip = fill(t.launcher_shortcut_reset_tip, &[("keys", &default.label(lang))]);
    // Greyed rather than gone when there is nothing to put back, like every action here.
    let enabled = state.search_shortcut != default;
    let reset = controls::icon_button(ui, Glyph::Undo, &tip, enabled).clicked();
    let field = shortcut_box(ui, state);

    if capturing.is_some() {
        let elsewhere = ui.input(|i| i.pointer.any_pressed()) && !field.hovered();
        let left = !ui.input(|i| i.focused);
        if elsewhere || left {
            state.cancel_shortcut_capture();
        }
    } else if field.clicked() {
        state.begin_shortcut_capture();
    }
    if reset {
        state.set_launcher_shortcut(default);
    }
}

/// How wide the shortcut box is: room for the shortcut and for the prompt, whichever is longer.
fn shortcut_box_width(ui: &egui::Ui, state: &AppState) -> f32 {
    let t = state.t();
    let lang = state.settings.lang;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let measure = |s: String| ui.painter().layout_no_wrap(s, font.clone(), egui::Color32::PLACEHOLDER).size().x;
    let natural = measure(state.search_shortcut.label(lang)).max(measure(t.launcher_shortcut_prompt.into()));
    (natural + 2.0 * SHORTCUT_BOX_PAD).max(SHORTCUT_BOX_MIN)
}

/// The box itself, drawn as a text field is: the same fill, border and accent line while it has
/// the keyboard — which, while it listens, it has more completely than any field.
fn shortcut_box(ui: &mut egui::Ui, state: &AppState) -> egui::Response {
    let t = state.t();
    let lang = state.settings.lang;
    let is_light = !ui.visuals().dark_mode;
    let capturing = state.shortcut_capture;
    let ink = ui.visuals().text_color();
    let (text, color) = match capturing {
        Some(0) => (t.launcher_shortcut_prompt.to_string(), text_tertiary(is_light)),
        Some(held) => (format!("{}…", crate::hotkey::mods_text(held, lang)), ink),
        None => (state.search_shortcut.label(lang), ink),
    };

    let width = shortcut_box_width(ui, state);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, controls::FIELD_HEIGHT), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        let stroke = if capturing.is_some() {
            visuals.selection.stroke
        } else {
            ui.style().interact(&response).bg_stroke
        };
        let radius = f32::from(controls::RADIUS_CONTROL);
        ui.painter()
            .rect(rect, radius, visuals.text_edit_bg_color(), stroke, egui::StrokeKind::Inside);
        let galley = egui::WidgetText::from(egui::RichText::new(text).color(color)).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            width - 2.0 * SHORTCUT_BOX_PAD,
            egui::TextStyle::Body,
        );
        let at = egui::pos2(rect.left() + SHORTCUT_BOX_PAD, rect.center().y - galley.size().y * 0.5);
        ui.painter().galley(at, galley, color);
        if capturing.is_some() {
            controls::field_underline(ui, rect);
        }
    }
    if capturing.is_none() && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, state.search_shortcut.label(lang))
    });
    controls::focus_ring(ui, &response, f32::from(controls::RADIUS_CONTROL));
    response
}

/// How much of a folder name a line shows. Folder names come from the user and nothing stops one
/// being a whole sentence; a line does not wrap, so a name left whole would run off the dialog.
const FOLDER_NAME_MAX_CHARS: usize = 32;

/// The folder picker that stands in front of an export or an import.
///
/// Drawn from `App::ui` right after [`show`], not from inside it: it has to be the modal on top of
/// Ajustes, and the document it is holding has to outlive the frame the button was pressed in.
///
/// One line per folder, ticked or not, in the same line the folder menu and «Mover a una carpeta»
/// use — so a folder is chosen the same way everywhere, and the check says what is included
/// without a sentence explaining how to include it. The running total underneath moves as lines
/// are ticked, which is what confirms the click did something.
pub fn show_folder_picker(ctx: &egui::Context, state: &mut AppState) {
    let t = state.settings.t();
    let Some(pending) = &mut state.pending_transfer else {
        return;
    };
    let is_export = matches!(pending.direction, TransferDirection::Export);
    let show_bulk = pending.groups.len() > 3;

    let mut confirm = false;
    let mut cancel = false;

    // `pending` stays borrowed for as long as the lines are on screen, so the two buttons only
    // record what was pressed. The transfer itself starts below, once that borrow is over.
    let modal = controls::dialog(ctx, "transfer_folders").show(ctx, |ui| {
        let is_light = !ui.visuals().dark_mode;
        ui.set_width(controls::dialog_width(ctx, 460.0));
        ui.add(
            egui::Label::new(controls::h3(if is_export {
                t.transfer_pick_export_title
            } else {
                t.transfer_pick_import_title
            }))
            .wrap(),
        );

        // Only worth offering once clicking them off one at a time is real work. With two or three
        // folders these buttons would be two more things to read for nothing. Quiet, so they read
        // as shortcuts over the list rather than as two more folders.
        if show_bulk {
            ui.add_space(controls::GAP);
            controls::quiet_row(ui, |ui| {
                if controls::button(ui, t.transfer_pick_all, Tone::Quiet, true).clicked() {
                    for group in pending.groups.iter_mut() {
                        group.selected = true;
                    }
                }
                if controls::button(ui, t.transfer_pick_none, Tone::Quiet, true).clicked() {
                    for group in pending.groups.iter_mut() {
                        group.selected = false;
                    }
                }
            });
        }

        ui.add_space(controls::GAP_WIDE);
        // Capped and scrolling, so thirty folders cannot push the buttons off the bottom of the
        // window — the one thing that would leave this modal with no way out but Esc.
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                for group in pending.groups.iter_mut() {
                    let name = group.folder.as_deref().unwrap_or(t.no_folder);
                    let name = crate::app::truncate(name, FOLDER_NAME_MAX_CHARS);
                    let count = group.count.to_string();
                    if controls::check_line(ui, &name, Some(&count), group.selected).clicked() {
                        group.selected = !group.selected;
                    }
                }
            });

        let chosen: usize = pending
            .groups
            .iter()
            .filter(|group| group.selected)
            .map(|group| group.count)
            .sum();

        ui.add_space(controls::GAP_WIDE);
        ui.add(
            egui::Label::new(controls::muted(
                fill_count(
                    if is_export {
                        t.transfer_pick_export_total
                    } else {
                        t.transfer_pick_import_total
                    },
                    chosen,
                    &[],
                ),
                is_light,
            ))
            .wrap(),
        );

        ui.add_space(controls::GAP_STACK);
        controls::dialog_buttons(ui, |ui| {
            if controls::button(ui, t.cancel, Tone::Normal, true).clicked() {
                cancel = true;
            }
            // Nothing chosen is not an error to explain afterwards: the way out is that the action
            // cannot be taken, with the total above already saying why.
            let go = if is_export {
                t.transfer_pick_export_go
            } else {
                t.transfer_pick_import_go
            };
            if controls::button(ui, go, Tone::Primary, chosen > 0).clicked() {
                confirm = true;
            }
        });
    });

    if modal.backdrop_response.clicked() {
        cancel = true;
    }
    if confirm {
        state.confirm_pending_transfer();
    } else if cancel {
        state.cancel_pending_transfer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One click at a time, the button visits every theme once and comes back to the first; the
    /// hidden one only once it has been found.
    #[test]
    fn the_theme_button_walks_every_theme_and_back() {
        for found in [false, true] {
            let mut mode = ThemeMode::System;
            let mut seen = Vec::new();
            loop {
                seen.push(mode);
                mode = next_theme(mode, found);
                if mode == ThemeMode::System {
                    break;
                }
            }
            assert_eq!(seen.len(), if found { 5 } else { 4 });
            assert_eq!(seen.contains(&ThemeMode::Khipu), found);
        }
        // Wearing the hidden theme after it was forgotten still leads somewhere.
        assert_eq!(next_theme(ThemeMode::Khipu, false), ThemeMode::Light);
    }
}
