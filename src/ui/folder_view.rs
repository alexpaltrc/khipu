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

//! Everything that can be done *to* a folder rather than inside it: rename it, switch its
//! expansions off or on, change the prefix of its triggers, export it, delete it.
//!
//! A screen and not a permanent panel. These are things a person does to a folder perhaps twice in
//! its life; a side panel showing them at all times spends a fifth of the window on them for ever,
//! and the library is what the window is *for*. Reached from a folder chip, one step back — and
//! the step back restores the filter, so the user lands where they came from.
//!
//! **Cards of rows, like Ajustes.** A row's name, one quiet line, and its control at the right
//! edge, in one centred column: the same reading order as the rest of the app, and no field
//! stretched across a wide window. Nothing asks to be saved. The name is kept when the field is
//! let go of (Esc takes it back), the switch acts at once, and only the prefix — the one change
//! that rewrites triggers — waits for its button.
//!
//! ## What this screen owes the user
//!
//! **The prefix change is shown before it happens.** Every trigger that would move is listed as
//! `antes → después`, and the Apply button is dead while any of them would collide with an
//! expansion that already exists. The plan is recomputed on every keystroke rather than on Apply,
//! which is the whole point: a conflict has to be visible *while* typing the prefix that causes it.
//!
//! **Deleting a folder deletes its expansions**, and that is said in the same row as the button
//! that does it, in the colour of danger. The semantics are not changed here — see
//! [`crate::app::AppState::request_delete_folder`] — only made impossible to miss.
//!
//! Nothing here writes to disk. Every action is a call into `AppState`.

use crate::app::{AppState, View};
use crate::ui::controls::{self, Tone};
use crate::ui::studio;

/// A prefix the user is trying out, together with what it would do.
///
/// The plan travels with the value instead of being recomputed while drawing, so the list of
/// changes and the error below it always describe the text that is actually in the field.
#[derive(Clone)]
struct PrefixDraft {
    value: String,
    plan: Result<Vec<crate::folders::Rename>, crate::folders::PrefixError>,
}

fn draft_key(folder: &str) -> egui::Id {
    egui::Id::new("folder-prefix-draft").with(folder)
}

/// The widest the column of cards gets. A name field, a sentence and a button fit in a row; wider,
/// the eye has to travel from a row's words across empty card to reach its control.
const COLUMN: f32 = 600.0;

/// Between the two cards: the folder's own settings, and deleting it.
const CARD_GAP: f32 = 20.0;

/// The name field in its row.
const NAME_FIELD: f32 = 220.0;

/// The prefix field in its row: a prefix is one to three characters.
const PREFIX_FIELD: f32 = 64.0;

/// How many `antes → después` lines are shown before the rest are summed up.
const RENAMES_SHOWN: usize = 6;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let View::FolderOptions(name) = &state.view else {
        return;
    };
    let folder = name.clone();
    let is_light = !ui.visuals().dark_mode;
    let t = state.t();

    // Counts and prefixes are copied out before anything else happens: the cache handle borrows
    // nothing from `state`, but `info` borrows from the cache, and every action below needs
    // `&mut state`.
    let list = state.list();
    let known = list.folder_names.iter().any(|f| f == &folder);
    // A folder that has been declared but holds nothing is absent from `folders::summarize`, which
    // only ever sees folders through the expansions filed in them. That is not a missing folder,
    // it is an empty one, and this screen has to be able to rename and delete it.
    let (count, prefixes): (usize, Vec<String>) = match list.folders.get(&folder) {
        Some(info) => (info.count, info.prefixes.iter().cloned().collect()),
        None => (0, Vec::new()),
    };
    drop(list);

    // The folder was renamed or deleted out from under this screen. Nothing to offer.
    if !known {
        state.view = View::List;
        return;
    }

    // The name field holds the folder's name the moment the screen opens — this is the screen for
    // renaming, so asking the user to press "rename" first would be a click that buys nothing.
    if state
        .renaming_folder
        .as_ref()
        .is_none_or(|(old, _)| old != &folder)
    {
        state.start_rename_folder(&folder);
    }

    // The folder's name is the page's title, above the scroll like every other page's, and how
    // much is in it sits right under it.
    let go_back = controls::page_header(ui, t.back, &folder, state.settings.lang);
    ui.add_space(controls::GAP_TIGHT - controls::GAP_STACK);
    ui.vertical_centered(|ui| {
        ui.label(controls::muted(count_line(state, count), is_light));
    });
    ui.add_space(controls::GAP_STACK + controls::GAP);

    controls::page_scroll(ui, ("folder-options", &folder), |ui| {
        let width = ui.available_width().min(COLUMN);
        let side = ((ui.available_width() - width) * 0.5).max(0.0);
        ui.horizontal(|ui| {
            ui.add_space(side);
            ui.vertical(|ui| {
                ui.set_width(width);
                controls::card(ui, |ui| {
                    name_row(ui, state);
                    // An empty folder has nothing to switch, re-prefix or export: the count
                    // line already says so, and rows that could only say "nothing here" go.
                    if count > 0 {
                        controls::card_divider(ui);
                        switch_row(ui, state, &folder);
                        controls::card_divider(ui);
                        prefix_row(ui, state, &folder, &prefixes, is_light);
                        controls::card_divider(ui);
                        export_row(ui, state, &folder);
                    }
                });
                ui.add_space(CARD_GAP);
                controls::card(ui, |ui| delete_row(ui, state, &folder, count, is_light));
                ui.add_space(controls::GAP_STACK);
            });
        });
    });

    if go_back {
        // Leaving lets go of the field too: a name typed and not yet confirmed is kept, the same
        // as clicking anywhere else. The filter follows the folder under its new name.
        state.commit_rename_folder();
        state.cancel_rename_folder();
        let current = match &state.view {
            View::FolderOptions(name) => name.clone(),
            _ => folder,
        };
        studio::set_filter(ui.ctx(), state, Some(&current));
    }
}

/// «3 expansiones», under the title.
fn count_line(state: &AppState, count: usize) -> String {
    crate::i18n::fill(
        studio::plural(
            count,
            studio::text(state, "{n} expansión", "{n} expansion", "{n} expansion", "{n} विस्तार"),
            studio::text(state, "{n} expansiones", "{n} expansions", "{n} expansion", "{n} विस्तार"),
        ),
        &[("n", &count.to_string())],
    )
}

// ----------------------------------------------------------------------------------------------
// Name
// ----------------------------------------------------------------------------------------------

fn name_row(ui: &mut egui::Ui, state: &mut AppState) {
    let title = studio::text(state, "Nombre", "Name", "Pangalan", "नाम");
    let about = studio::text(
        state,
        "Cambiarlo no toca los activadores ni el texto.",
        "Renaming leaves the triggers and the text alone.",
        "Hindi nito binabago ang mga trigger o ang teksto.",
        "नाम बदलने से ट्रिगर या टेक्स्ट नहीं बदलते।",
    );
    let width = NAME_FIELD.min(ui.available_width() * 0.45);
    let left = controls::card_row(ui, title, about, width, |ui| {
        let (_, draft) = state.renaming_folder.as_mut()?;
        let field = ui.add(
            controls::text_field(draft)
                .id(egui::Id::new("folder-rename-field"))
                .desired_width(width),
        );
        controls::say_as(&field, title);
        Some(field.lost_focus())
    });
    if left == Some(true) {
        // Esc gives the name back; anything else that lets go of the field keeps it.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some((old, new)) = state.renaming_folder.as_mut() {
                *new = old.clone();
            }
        } else {
            state.commit_rename_folder();
        }
    }
}

// ----------------------------------------------------------------------------------------------
// On and off
// ----------------------------------------------------------------------------------------------

/// Switches every expansion in the folder off, or back on. Not a state of the folder: a folder is
/// only the assignments of its triggers, so this does to all of them at once what the switch in
/// the detail does to one. On while any of them is on, so a folder with some of each says so
/// under the name and goes off with one click.
fn switch_row(ui: &mut egui::Ui, state: &mut AppState, folder: &str) {
    let triggers: Vec<String> = state
        .match_file
        .entries
        .iter()
        .filter_map(|e| match e {
            crate::yaml::model::MatchEntry::Simple(m)
                if state.settings.folder_of(&m.trigger) == Some(folder) =>
            {
                Some(m.trigger.clone())
            }
            _ => None,
        })
        .collect();
    let off = triggers.iter().filter(|t| state.match_file.is_off(t)).count();
    let on = triggers.len() - off;

    let title = studio::text(state, "Encendida", "On", "Nakabukas", "चालू");
    let about = if off > 0 && on > 0 {
        crate::i18n::fill(
            studio::text(
                state,
                "{k} de {n} apagadas.",
                "{k} of {n} off.",
                "{k} sa {n} ang nakapatay.",
                "{n} में से {k} बंद।",
            ),
            &[("k", &off.to_string()), ("n", &triggers.len().to_string())],
        )
    } else {
        studio::text(
            state,
            "Si la apagas, no se expande ni sale en el buscador.",
            "When off, it neither expands nor shows in search.",
            "Kapag nakapatay, hindi nag-e-expand o lumalabas sa paghahanap.",
            "बंद होने पर न विस्तार होता है, न खोज में दिखता है।",
        )
        .to_owned()
    };
    let mut lit = on > 0;
    let changed = controls::card_row(ui, title, &about, 40.0, |ui| {
        controls::toggle(ui, &mut lit, title).changed()
    });
    if changed {
        state.set_off(&triggers, !lit);
    }
}

// ----------------------------------------------------------------------------------------------
// Prefix
// ----------------------------------------------------------------------------------------------

fn prefix_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    folder: &str,
    prefixes: &[String],
    is_light: bool,
) {
    let t = state.t();
    let title = studio::text(state, "Prefijo", "Prefix", "Prefix", "उपसर्ग");

    let none_label = studio::text(state, "sin prefijo", "no prefix", "walang prefix", "कोई उपसर्ग नहीं");
    let shown: Vec<String> = prefixes
        .iter()
        .map(|p| if p.is_empty() { none_label.to_owned() } else { format!("«{p}»") })
        .collect();
    let about = if prefixes.len() > 1 {
        crate::i18n::fill(
            studio::text(
                state,
                "Usa varios: {list}. Uno nuevo los une a todos.",
                "It uses several: {list}. A new one joins them all.",
                "Iba-iba ang gamit: {list}. Pag-iisahin sila ng bago.",
                "कई उपयोग में हैं: {list}। नया सबको एक कर देगा।",
            ),
            &[("list", &shown.join(" "))],
        )
    } else {
        // One prefix is already in the field beside the words; they only say what it is.
        studio::text(
            state,
            "Lo que va antes de cada activador.",
            "What comes before each trigger.",
            "Ang nauuna sa bawat trigger.",
            "हर ट्रिगर से पहले आने वाला।",
        )
        .to_owned()
    };

    let key = draft_key(folder);
    let mut draft = ui.ctx().data(|d| d.get_temp::<PrefixDraft>(key)).unwrap_or_else(|| {
        let value = prefixes
            .first()
            .cloned()
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| state.settings.prefix.clone());
        let plan = crate::folders::plan(&state.match_file.entries, &state.settings, folder, &value);
        PrefixDraft { value, plan }
    });

    let changed = controls::card_row(ui, title, &about, PREFIX_FIELD, |ui| {
        let field = ui.add(
            controls::text_field(&mut draft.value)
                .id(egui::Id::new(("folder-prefix-field", folder)))
                .desired_width(PREFIX_FIELD),
        );
        controls::say_as(&field, title);
        field.changed()
    });
    if changed {
        draft.plan = crate::folders::plan(&state.match_file.entries, &state.settings, folder, &draft.value);
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, draft.clone()));

    // What the new prefix would do, under its row and inside the same card, once there is
    // something to show.
    match &draft.plan {
        Ok(changes) if changes.is_empty() => {}
        Ok(changes) => {
            controls::inset_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for change in changes.iter().take(RENAMES_SHOWN) {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("{}  →  {}", change.old, change.new))
                                .monospace()
                                .size(13.0),
                        )
                        .wrap(),
                    );
                }
                if changes.len() > RENAMES_SHOWN {
                    ui.add_space(controls::GAP_TIGHT);
                    ui.label(controls::small_muted(
                        crate::i18n::fill(t.see_more, &[("n", &(changes.len() - RENAMES_SHOWN).to_string())]),
                        is_light,
                    ));
                }
            });
            let label = format!("{}  ·  {}", apply_label(state), changes.len());
            let note = studio::text(
                state,
                "Solo cambian los activadores. El texto se conserva.",
                "Only the triggers change. The text stays.",
                "Mga trigger lang ang magbabago. Pareho ang teksto.",
                "केवल ट्रिगर बदलेंगे। टेक्स्ट वही रहेगा।",
            );
            let apply = controls::card_row(ui, "", note, controls::button_width(ui, &label), |ui| {
                controls::button(ui, &label, Tone::Primary, true).clicked()
            });
            if apply {
                let value = draft.value.clone();
                state.apply_folder_prefix(folder, &value);
                ui.ctx().data_mut(|d| d.remove::<PrefixDraft>(key));
            }
        }
        Err(error) => {
            // Present, visible and dead: the button has to stay on screen or the explanation for
            // why nothing happens leaves with it.
            controls::notice(ui, &studio::prefix_error(state, error), Tone::Danger);
            let label = apply_label(state);
            controls::card_row(ui, "", "", controls::button_width(ui, label), |ui| {
                controls::button(ui, label, Tone::Primary, false);
            });
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Export and delete
// ----------------------------------------------------------------------------------------------

fn export_row(ui: &mut egui::Ui, state: &mut AppState, folder: &str) {
    let title = studio::text(state, "Exportar", "Export", "I-export", "निर्यात");
    let about = studio::text(
        state,
        "Guárdala en un archivo para llevarla a otra computadora.",
        "Save it to a file to take it to another computer.",
        "I-save sa file para madala sa ibang kompyuter.",
        "दूसरे कंप्यूटर पर ले जाने के लिए फ़ाइल में सहेजें।",
    );
    let label = studio::text(state, "Exportar", "Export", "I-export", "निर्यात");
    let export = controls::card_row(ui, title, about, controls::button_width(ui, label), |ui| {
        controls::button(ui, label, Tone::Normal, true).clicked()
    });
    if export {
        state.export_folder(folder);
    }
}

fn delete_row(ui: &mut egui::Ui, state: &mut AppState, folder: &str, count: usize, is_light: bool) {
    let title = studio::text(state, "Eliminar carpeta", "Delete folder", "Tanggalin ang folder", "फ़ोल्डर हटाएँ");
    // The one sentence this screen exists to make unmissable, beside the button it is about. An
    // empty folder loses nothing, and an alarm for nothing teaches people to read past the one
    // that matters, so only a folder with something in it says it in red.
    let about = if count == 0 {
        controls::small_muted(
            studio::text(
                state,
                "Está vacía: no se elimina ninguna expansión.",
                "It is empty: no expansion is deleted.",
                "Walang laman: walang expansion na matatanggal.",
                "यह खाली है: कोई विस्तार नहीं हटेगा।",
            ),
            is_light,
        )
    } else {
        let line = crate::i18n::fill(
            studio::plural(
                count,
                studio::text(
                    state,
                    "Su {n} expansión se elimina con ella.",
                    "Its {n} expansion goes with it.",
                    "Matatanggal din ang {n} expansion nito.",
                    "इसका {n} विस्तार भी हट जाएगा।",
                ),
                studio::text(
                    state,
                    "Sus {n} expansiones se eliminan con ella.",
                    "Its {n} expansions go with it.",
                    "Matatanggal din ang {n} expansion nito.",
                    "इसके {n} विस्तार भी हट जाएँगे।",
                ),
            ),
            &[("n", &count.to_string())],
        );
        egui::RichText::new(line).small().color(crate::app::danger(is_light))
    };
    let label = state.t().delete;
    let delete = controls::card_row_with(ui, title, about, controls::button_width(ui, label), |ui| {
        controls::button(ui, label, Tone::Danger, true).clicked()
    });
    if delete {
        state.request_delete_folder(folder);
    }
}

/// The button under a new prefix's changes. Short: the list above already says what it applies.
fn apply_label(state: &AppState) -> &'static str {
    studio::text(state, "Aplicar", "Apply", "Ilapat", "लागू करें")
}
