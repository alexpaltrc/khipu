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

//! The one form: creating an expansion and editing one, which are the same screen with a different
//! title and a different verb.
//!
//! `EditState` is the whole form — every field the user can touch — and it lives in
//! `View::Edit`, so a half-written expansion survives switching screens and coming back.
//! `build_match` turns it into the `SimpleMatch` that gets saved, and `new_for_edit` turns a stored
//! one back into a form; those two have to stay each other's inverse, or reopening an expansion
//! shows something the user did not write.
//!
//! The trigger is split into prefix and body for editing and rejoined on save, so changing the
//! global prefix is a change to one setting and not to every trigger in the file.
//!
//! The date half of the form is a preset picker over [`crate::yaml::presets`] plus, when none of
//! them fits, the block builder in [`crate::ui::date_blocks`]. The month-language control appears
//! wherever a month is actually spelled out, presets and custom alike, because the version that
//! sat at the foot of the form read as missing.
//!
//! `shadowing_pair` warns about one relation only — one trigger being a prefix of another — and
//! deliberately does not warn about the pair the prefix box itself produces. Read its own comment
//! before adding a case to it; espanso's matcher makes most of the pairs that look dangerous safe.
//!
//! ## Two questions, in order
//!
//! Two numbered questions: **¿Qué vas a escribir?** and **¿Qué quieres que aparezca?**. The second
//! is drawn faint while the first is still empty, never locked: the order is a suggestion, and
//! anybody may fill them in any order. A question and the date are a
//! button each under the text, whose right-click menu cuts, copies and pastes. What was last
//! copied is still shown as a pill in texts that have it, but nothing offers it any more.
//! The star, the folder and the name change nothing about what an expansion does, so they are not
//! questions: they wait as quiet buttons at the foot, opposite Guardar (decided by Alex,
//! 2026-11-22). What the expansion will do is shown once, in the preview, and nothing on the form
//! repeats it.
//!
//! ## Leaving is asked about only when there is something to lose
//!
//! The form is copied as it stood when the screen was arrived at. Esc and Cancelar leave at once
//! while the draft still matches that copy, and ask «¿Descartar los cambios?» once it does not.
//!
//! ## Validation is a thing the form knows, not a thing saving discovers
//!
//! [`problem_with`] is worked out once per frame, before anything is drawn, and two places read it:
//! the message beside the field that causes it, and whether Guardar can be pressed at all. That is
//! the plan's rule — offer saving only when the form is valid — and it is the only arrangement that
//! makes the rule actionable, because a button that is dead for a reason printed somewhere else is
//! one nobody can act on. [`crate::app::AppState::save_edit`] still checks for itself: it is reached
//! from Ctrl+S and from tests as well as from this button.

use crate::app::{AppState, View};
use crate::datefmt;
use crate::i18n::{fill, Lang, Strings};
use crate::yaml::form::{self, FieldKind, FormField, FormSpec};
use crate::yaml::model::{SimpleMatch, VarEntry};
use crate::ui::controls::{self, Tone};
use crate::ui::glyphs::Glyph;
use crate::ui::keys;
use crate::ui::studio;
use crate::yaml::presets::{build_custom_date_var, month_lang_of, DatePreset};

const DATE_VAR_NAME: &str = "fecha";
/// How many lines of the replacement box are visible before it has anything in it.
///
/// It is a floor, not a ceiling: the box grows as the text does and the page scrolls. Six is about
/// an email signature — enough that it does not read as a one-line field, few enough that the
/// preview underneath is still on screen in an ordinary window.
const TEXT_AREA_ROWS: f32 = 6.0;
/// The tallest the preview of the result gets before it scrolls inside itself. Rounded down to a
/// whole number of lines where it is used, so the last line shown is a whole line.
const PREVIEW_MAX_HEIGHT: f32 = 168.0;
/// The widest the form is allowed to get, however wide the window is.
///
/// A replacement box eight hundred points across is not eight hundred points more useful: past a
/// point a line of text stops being comfortable to read, and the two boxes of step 1 start to look
/// like two unrelated things at opposite ends of the screen. Wider windows get margin, not wider
/// boxes — which is also the plan's rule about not scaling controls indiscriminately, read the
/// other way round.
const FORM_MAX_WIDTH: f32 = 880.0;
/// The preview's column, beside the form in a window wide enough for both. Standing apart and
/// still while the form scrolls, it reads as something to look at, not one more thing to fill in.
const SIDE_WIDTH: f32 = 300.0;
/// The narrowest the form gets to make room for that column. Any narrower and the preview goes back
/// under the text, where it was.
const SIDE_FORM_MIN: f32 = 520.0;
/// Width of the prefix box. Wide enough for the longest suggestion ("//") with the field padding
/// around it, and no wider — the prefix is one or two symbols, not a sentence.
/// Between the three parts of the form. With no card around each, the space is what tells them
/// apart, so it is wider than the gap between two things inside one part.
const PART_GAP: f32 = 28.0;
const PREFIX_FIELD_WIDTH: f32 = 72.0;

/// How the date a text holds as `{{fecha}}` is written. Kept while its pill is out of the text, so
/// cutting it and putting it back does not lose the format.
#[derive(Debug, Clone, PartialEq)]
pub struct DateChoice {
    /// The name inside its `{{...}}`.
    pub name: String,
    pub preset: Option<DatePreset>,
    pub custom_format: String,
    pub custom_tz: String,
    /// Which language a written-out month or weekday is spelled in, stored in the expansion itself
    /// so it reads the same on every machine it's shared with.
    pub month_lang: Lang,
}

impl Default for DateChoice {
    fn default() -> Self {
        let preset = DatePreset::IsoDateOnly;
        Self {
            name: DATE_VAR_NAME.to_string(),
            preset: Some(preset),
            custom_format: preset.format().to_string(),
            custom_tz: String::new(),
            month_lang: Lang::En,
        }
    }
}

impl DateChoice {
    fn of(var: &VarEntry) -> Self {
        Self {
            name: var.name.clone(),
            preset: DatePreset::detect(var),
            custom_format: var.param_str("format").unwrap_or("").to_string(),
            custom_tz: var.param_str("tz").unwrap_or("").to_string(),
            month_lang: month_lang_of(var),
        }
    }

    /// The `date` variable espanso reads.
    fn var(&self) -> VarEntry {
        match self.preset {
            Some(p) => p.build_var(&self.name, self.month_lang),
            None => build_custom_date_var(&self.name, &self.custom_format, Some(&self.custom_tz), self.month_lang),
        }
    }
}

/// Splits a stored trigger into its symbol prefix and the word after it.
///
/// The prefix is the leading run of non-alphanumeric characters, so `"::hola"` becomes
/// `("::", "hola")`. Keeping the two apart is what makes "apply this prefix everywhere" predictable:
/// the prefix is *replaced* rather than something new being stapled on to whatever was already
/// there, no matter whether the old one was one symbol or three.
///
/// A trigger with no letter or digit anywhere in it — `":--"`, `":_"`, `":—"` — is **all word and
/// no prefix**. Nothing in it says where a prefix would end, and calling the whole thing a prefix
/// left it with no word at all: the editor then refused to save it as an empty trigger, which is
/// why `":--" → "—"` could not be created. The prefix operations know the same rule and leave such
/// triggers alone; see [`crate::folders`].
pub fn split_trigger(trigger: &str) -> (String, String) {
    match trigger.char_indices().find(|(_, c)| c.is_alphanumeric()) {
        Some((word_start, _)) => (
            trigger[..word_start].to_string(),
            trigger[word_start..].to_string(),
        ),
        None => (String::new(), trigger.to_string()),
    }
}

/// How a worked example of a date is drawn: monospaced so it reads as literal output rather than as
/// prose, and muted so it supports the option it sits beside instead of competing with it.
fn sample_text(sample: &str) -> egui::RichText {
    egui::RichText::new(sample).monospace().weak()
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditState {
    /// `None` while creating a brand new expansion.
    pub editing_index: Option<usize>,
    /// Held apart from the word so the two can be edited independently; recombined on save.
    pub prefix: String,
    pub word: String,
    /// The text it writes, with its blanks, what was last copied and the date as markers.
    pub replace: String,
    /// What the user calls it, if anything. Written as espanso's `label`, which espanso itself
    /// shows in its own search bar; empty means no `label` at all.
    pub name: String,
    /// Optional folder this expansion belongs to. Entirely our own bookkeeping, never written
    /// into espanso's YAML.
    pub folder: Option<String>,
    pub choosing_new_folder: bool,
    pub new_folder_input: String,
    /// Whether it is a favourite. Ours, like the folder, and saved with Guardar like the rest of
    /// the form: a star changed here and then cancelled stays as it was.
    pub favorite: bool,
    /// What each blank in the text is: its kind, its starting value, a list's options. Keyed by
    /// name, and only for the names someone has set something on; a blank with no entry here is a
    /// short text that starts empty. Kept when its marker leaves the text, so cutting a marker and
    /// putting it back does not lose what was set on it. See [`EditState::form`].
    pub fields: Vec<FormField>,
    /// The name inside the `{{...}}` that stands for what was last copied: the one the stored
    /// expansion uses, or the word for it in the language it was first added in. Empty until then.
    pub copied: String,
    /// How the date is written, whether or not the text holds it.
    pub date: DateChoice,
}

impl EditState {
    pub fn new_for_create(default_prefix: &str) -> Self {
        Self {
            editing_index: None,
            prefix: default_prefix.to_string(),
            word: String::new(),
            replace: String::new(),
            name: String::new(),
            folder: None,
            choosing_new_folder: false,
            new_folder_input: String::new(),
            favorite: false,
            fields: Vec::new(),
            copied: String::new(),
            date: DateChoice::default(),
        }
    }

    pub fn new_for_edit(index: usize, m: &SimpleMatch, folder: Option<String>) -> Self {
        let form = m.as_form();
        // A form is edited as the text it fills in, markers and all.
        let replace = form.as_ref().map_or_else(|| m.replace.clone(), |f| f.template.clone());
        let date = form.as_ref().and_then(|f| f.date.as_ref()).or_else(|| m.as_date_var());
        let (prefix, word) = split_trigger(&m.trigger);
        Self {
            editing_index: Some(index),
            prefix,
            word,
            replace,
            name: m.label.clone().unwrap_or_default(),
            date: date.map(DateChoice::of).unwrap_or_default(),
            folder,
            choosing_new_folder: false,
            new_folder_input: String::new(),
            favorite: false,
            copied: form
                .as_ref()
                .and_then(|f| f.copied.clone())
                .or_else(|| m.as_copied().map(str::to_string))
                .unwrap_or_default(),
            fields: form.map(|f| f.fields).unwrap_or_default(),
        }
    }

    /// The form the text makes, if it has any `[[name]]` in it: one field per name, in the order the
    /// names first appear, each as set here or a short text if nothing was.
    pub fn form(&self, text: &str) -> Option<FormSpec> {
        let spec = self.as_spec(text);
        (!spec.fields.is_empty()).then_some(spec)
    }

    /// The text as a form would see it, blanks or not: what the preview shows.
    fn as_spec(&self, text: &str) -> FormSpec {
        FormSpec {
            template: text.to_string(),
            fields: form::names(text)
                .into_iter()
                .map(|name| {
                    self.fields
                        .iter()
                        .find(|f| f.name == name)
                        .cloned()
                        .unwrap_or(FormField { name, ..Default::default() })
                })
                .collect(),
            copied: self.copied_in(text).map(str::to_string),
            date: self.date_in(text).then(|| self.date.var()),
        }
    }

    /// The name of what was last copied, if the text has its marker in it.
    pub fn copied_in(&self, text: &str) -> Option<&str> {
        (!form::copied_markers(text, &self.copied).is_empty()).then_some(self.copied.as_str())
    }

    /// Whether the text has the date's marker in it.
    pub fn date_in(&self, text: &str) -> bool {
        !form::copied_markers(text, &self.date.name).is_empty()
    }

    /// A new blank's marker, under the first free name. A name is free once its marker is gone
    /// from the text, but what was set on that field is kept for a cut and paste, so a new blank
    /// that takes the name starts over instead of coming back as the old one's list or value.
    fn new_blank(&mut self, name: String) -> String {
        self.fields.retain(|f| f.name != name);
        format!("[[{name}]]")
    }

    /// The settings of the field called `name`, made on first touch.
    fn field_mut(&mut self, name: &str) -> &mut FormField {
        let at = match self.fields.iter().position(|f| f.name == name) {
            Some(at) => at,
            None => {
                self.fields.push(FormField { name: name.to_string(), ..Default::default() });
                self.fields.len() - 1
            }
        };
        &mut self.fields[at]
    }

    /// Resolves the effective folder, taking the "escribiendo una carpeta nueva" text box into
    /// account if that's the mode the user was in when they hit Guardar.
    pub fn resolved_folder(&self) -> Option<String> {
        if self.choosing_new_folder {
            let name = self.new_folder_input.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        } else {
            self.folder.clone()
        }
    }

    /// The trigger as espanso will see it: the prefix and the word joined back together.
    ///
    /// The word is stripped of any leading symbols first, by the same rule that splits a stored
    /// trigger into its two halves. Without that the two boxes fought each other: typing `:hola`
    /// into a box labelled "palabra", next to a prefix box already showing `:`, produced `::hola` —
    /// a trigger nobody asked for, and one that then had to be hunted down in Settings. The prefix
    /// box owns the prefix; anything typed in the word box is a word.
    ///
    /// Returns empty only when the word box is empty, so a prefix on its own is caught by the same
    /// check that catches an empty trigger rather than being saved as one. A word box holding
    /// nothing but symbols — `--`, `_`, `—` — is a word: [`split_trigger`] hands it back whole
    /// rather than eating it as a second prefix, so `:` and `--` make `:--`.
    pub fn trigger(&self) -> String {
        let (_, word) = split_trigger(self.word.trim());
        if word.is_empty() {
            return String::new();
        }
        format!("{}{}", self.prefix.trim(), word)
    }

    /// The name as it is written to `label`: trimmed, and absent rather than empty.
    fn label(&self) -> Option<String> {
        let name = self.name.trim();
        (!name.is_empty()).then(|| name.to_string())
    }

    pub fn build_match(&self) -> SimpleMatch {
        let spec = self.as_spec(&self.replace);
        // Any blank makes it a form: one variable that opens Khipu's window. See
        // [`crate::yaml::form`].
        if !spec.fields.is_empty() {
            let trigger = self.trigger();
            return SimpleMatch {
                vars: vec![spec.to_var(&trigger)],
                replace: format!("{{{{{}}}}}", form::VAR_NAME),
                trigger,
                label: self.label(),
            };
        }
        // What was last copied and the date, with no blanks, are espanso's own to fill in.
        let copied = spec.copied.map(|name| VarEntry {
            name,
            var_type: "clipboard".to_string(),
            ..Default::default()
        });
        SimpleMatch {
            trigger: self.trigger(),
            replace: self.replace.clone(),
            vars: copied.into_iter().chain(spec.date).collect(),
            label: self.label(),
        }
    }
}

/// Where the worked examples start, measured from the left edge of the option list.
///
/// The five options and their samples read as two columns, the way the trigger and its preview do
/// in the main list: every example begins on the same invisible line, so the eye compares four
/// dates instead of hunting for each one at a different indent.
///
/// This is not a constant, because the labels are translated — "Fecha y hora en UTC (ISO 8601)" is
/// a different width in each of the four languages, and a number picked for Spanish would either
/// crowd Hindi or waste half the row in English. It is the widest label plus the fixed chrome a
/// radio button puts around its text, and that chrome is *learned from a real one* (see
/// [`date_option`]) rather than reconstructed from egui's internal spacing, so a future change to
/// that spacing cannot quietly break the alignment.
fn option_column(ui: &egui::Ui, labels: &[&str]) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let widest = labels
        .iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap((*label).to_owned(), font.clone(), egui::Color32::PLACEHOLDER)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let chrome = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(radio_chrome_id()))
        .unwrap_or(48.0);
    widest + chrome + SAMPLE_GUTTER
}

/// The clear space between the longest option and where the examples begin.
///
/// Wider than the ordinary gap between two widgets, and deliberately so: this is the seam between
/// two columns, not the join between two things on one line. At a single gap the longest option and
/// its example touched, and the whole point of the alignment — four results read as a list — was
/// lost on the row that needed it most.
const SAMPLE_GUTTER: f32 = 30.0;

fn radio_chrome_id() -> egui::Id {
    egui::Id::new("radio_chrome")
}

/// One row of the date picker: the option, then the date it would produce right now, the latter
/// pushed out to `column` so all of them line up.
///
/// `sample` is optional because "Personalizado" has nothing to show until it is selected and has
/// blocks in it.
///
/// Returns `true` on the frame the option is clicked.
fn date_option(
    ui: &mut egui::Ui,
    selected: bool,
    label: &str,
    sample: Option<String>,
    column: f32,
) -> bool {
    let mut clicked = false;
    let stack_sample = sample.as_ref().is_some_and(|s| {
        let width = ui.painter().layout_no_wrap(s.clone(),egui::TextStyle::Monospace.resolve(ui.style()),egui::Color32::PLACEHOLDER).size().x;
        column + width > ui.available_width()
    });
    ui.horizontal(|ui| {
        let placed = ui.radio(selected, label);
        clicked = placed.clicked();

        // What a radio button adds around its own text — the circle, the gap, the padding. Taken
        // from one that has actually been laid out, and remembered, so the next frame can size the
        // column before drawing anything.
        //
        // Measured once and then left alone. It is the same number for every row and every frame —
        // it comes from the style, not the text — so re-deriving it meant laying every label out a
        // second time, five extra text layouts per frame, to arrive at the number already stored.
        if ui.ctx().data(|d| d.get_temp::<f32>(radio_chrome_id())).is_none() {
            let font = egui::TextStyle::Button.resolve(ui.style());
            let text_width = ui
                .painter()
                .layout_no_wrap(label.to_owned(), font, egui::Color32::PLACEHOLDER)
                .size()
                .x;
            let chrome = placed.rect.width() - text_width;
            if chrome > 0.0 {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(radio_chrome_id(), chrome));
            }
        }

        if let Some(sample) = sample.as_ref().filter(|_| !stack_sample) {
            let padding = column - placed.rect.width() - ui.spacing().item_spacing.x;
            if padding > 0.0 {
                ui.add_space(padding);
            }
            // On the option's baseline rather than centred beside it. See [`controls::baseline`].
            let drop = controls::baseline_drop(ui, egui::RichText::new("x"), egui::RichText::new("x").monospace());
            let line = egui::WidgetText::from(sample_text(sample)).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Body,
            );
            let (rect, _) = ui.allocate_exact_size(egui::vec2(line.size().x, placed.rect.height()), egui::Sense::hover());
            let top = rect.center().y - line.size().y * 0.5 + drop;
            ui.painter().galley(egui::pos2(rect.left(), top), line, ui.visuals().weak_text_color());
        }
    });
    if let Some(sample) = sample.as_ref().filter(|_| stack_sample) {
        ui.indent(egui::Id::new(label).with("sample"),|ui| { ui.add(egui::Label::new(sample_text(sample)).wrap()); });
    }
    clicked
}

/// The pair of triggers that cannot both work, if this one makes such a pair with an existing
/// expansion — shortest first.
///
/// Espanso fires a match the moment what you have typed ends with its trigger; it does not wait to
/// see whether you were going to type something longer, because it has no way of knowing when you
/// have stopped. So between `:test` and `:testing`, only `:test` can ever fire: by the fifth
/// character it has already replaced itself, and the `ing` lands in whatever it left behind.
///
/// This matters in both directions, which is why the answer is a pair rather than a yes. Adding
/// `:testing` next to an existing `:test` gives you an expansion that never works; adding `:test`
/// next to an existing `:testing` silently breaks the one you already had. Naming the short one and
/// the long one covers both of those directions without needing two different warnings.
///
/// Only the *prefix* relation is a conflict, and deliberately so. `:hola` and `::hola` look like
/// they collide — typing the long one does end with the short one — but espanso's rolling matcher
/// keeps one live path per position and returns on the first that completes, checking the paths
/// that started earliest first; the longest trigger is therefore always the one that wins, and both
/// expansions keep working. Warning about that pair would put a warning on exactly what the prefix
/// box and "apply this prefix everywhere" produce, which is the one thing here that is not a
/// mistake.
fn shadowing_pair(
    entries: &[crate::yaml::model::MatchEntry],
    editing: Option<usize>,
    trigger: &str,
) -> Option<(String, String)> {
    if trigger.is_empty() {
        return None;
    }
    entries.iter().enumerate().find_map(|(i, entry)| {
        if Some(i) == editing {
            return None;
        }
        let other = entry.trigger_str();
        // An exact match is a plain duplicate, and saving already refuses that one by name.
        if other.is_empty() || other == trigger {
            return None;
        }
        if trigger.starts_with(other) {
            Some((other.to_string(), trigger.to_string()))
        } else if other.starts_with(trigger) {
            Some((trigger.to_string(), other.to_string()))
        } else {
            None
        }
    })
}

/// The month a format spells out in words has to be spelled out in *some* language, and this is
/// where that is chosen.
///
/// It appears in two places — under the presets that write a month out, and inside the block
/// builder as soon as a "Mes (nombre)" block is in the row — so it lives in one function. The
/// alternative, one control at the foot of the form serving both, is what it used to be: for a
/// custom format it ended up below two palettes, nowhere near the block that needed it, and read
/// as missing entirely.
pub fn month_language(ui: &mut egui::Ui, t: &'static Strings, month_lang: &mut Lang) {
    field_label(ui, t.month_language_label);
    // Each language written in its own script, exactly as in the interface's own language picker
    // in Settings — and in the same control, so the two are recognisably the same question asked
    // twice rather than two different-looking widgets.
    let labels: Vec<&str> = Lang::ALL
        .iter()
        .map(|lang| lang.picker_label(ui.ctx()))
        .collect();
    let selected = Lang::ALL.iter().position(|l| l == month_lang);
    if let Some(picked) = controls::segmented(ui, "month_lang", &labels, selected) {
        *month_lang = Lang::ALL[picked];
    }
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(t.month_language_hint)
            .small()
            .color(crate::app::text_tertiary(!ui.visuals().dark_mode)),
    );
}

/// The caption above a control. Small and in the secondary colour, matching the section headings in
/// Settings and the folder names in the list — one register for "this names what is below it", used
/// everywhere, so the eye learns it once.
fn field_label(ui: &mut egui::Ui, text: &str) {
    let is_light = !ui.visuals().dark_mode;
    ui.label(
        egui::RichText::new(text)
            .strong()
            .color(crate::app::secondary_text(is_light)),
    );
    ui.add_space(4.0);
}

// --- What is wrong, if anything ----------------------------------------------------------------

/// Why the form cannot be saved yet. Three things, and no more than three: a rule that stops
/// somebody saving has to be one they can see the point of at the moment it stops them.
enum Problem {
    /// Nothing in the word box. A step not taken rather than a mistake made, and said as one.
    Empty,
    /// A letter or a digit in the prefix box.
    ///
    /// Worth blocking rather than quietly allowing, because [`split_trigger`] calls the *leading
    /// run of symbols* the prefix: `ab` and `hola` save as `abhola` and reopen as no prefix and the
    /// word `abhola`, and applying a folder prefix to it afterwards would eat the `ab`. By
    /// construction no stored expansion can arrive in this state, so this only ever catches
    /// something typed here a moment ago.
    PrefixHasLetters,
    /// Some other expansion already answers to this trigger. Espanso keeps the first match it finds
    /// for a trigger, so the second one would never fire.
    Duplicate(String),
    /// A blank set to be a list, with nothing in the list. The window would offer no choice at all.
    EmptyList(String),
}

fn problem_with(state: &AppState, edit: &EditState) -> Option<Problem> {
    let trigger = edit.trigger();
    if trigger.is_empty() {
        return Some(Problem::Empty);
    }
    if edit.prefix.trim().chars().any(char::is_alphanumeric) {
        return Some(Problem::PrefixHasLetters);
    }
    // Advanced matches count too, exactly as they do in `AppState::save_edit`. They are not in the
    // list, so a trigger colliding with one would look free here and then never fire.
    let taken = state
        .match_file
        .entries
        .iter()
        .enumerate()
        .any(|(i, e)| Some(i) != edit.editing_index && e.trigger_str() == trigger);
    if taken {
        return Some(Problem::Duplicate(trigger));
    }
    edit.form(&edit.replace)?
        .fields
        .into_iter()
        .find(|f| f.kind == FieldKind::Choice && f.options.iter().all(|o| o.trim().is_empty()))
        .map(|f| Problem::EmptyList(f.name))
}

fn problem_text(state: &AppState, t: &'static Strings, problem: &Problem) -> String {
    match problem {
        Problem::Empty => studio::text(
            state,
            "Escribe el activador que quieres usar.",
            "Type the trigger you want to use.",
            "I-type ang trigger na gusto mong gamitin.",
            "जो ट्रिगर चाहिए वह लिखें।",
        )
        .to_string(),
        Problem::PrefixHasLetters => studio::text(
            state,
            "El prefijo solo lleva símbolos, como : o //. Las letras van en el activador.",
            "The prefix takes symbols only, such as : or //. Letters belong in the trigger.",
            "Simbolo lang ang prefix, gaya ng : o //. Sa trigger napupunta ang mga letra.",
            "उपसर्ग में केवल चिह्न आते हैं, जैसे : या //। अक्षर ट्रिगर में लिखें।",
        )
        .to_string(),
        Problem::Duplicate(trigger) => fill(t.trigger_duplicate, &[("name", trigger)]),
        Problem::EmptyList(name) => fill(
            studio::text(
                state,
                "La lista «{name}» necesita al menos una opción.",
                "The list «{name}» needs at least one option.",
                "Kailangan ng listahang «{name}» ng kahit isang pagpipilian.",
                "सूची «{name}» में कम से कम एक विकल्प चाहिए।",
            ),
            &[("name", name)],
        ),
    }
}

// --- The screen ---------------------------------------------------------------------------------

/// The form as it stood when the screen was arrived at, kept to tell a draft that has been touched
/// from one that has not. Only a touched one is worth a question on the way out.
fn original_key() -> egui::Id {
    egui::Id::new("editor-original")
}

/// The frame the form was last drawn on, so arriving at it can be told apart from staying on it.
fn seen_key() -> egui::Id {
    egui::Id::new("editor-seen")
}

/// Set while «¿Descartar los cambios?» is on screen.
fn confirm_discard_key() -> egui::Id {
    egui::Id::new("editor-confirm-discard")
}

fn word_field_id() -> egui::Id {
    egui::Id::new("edit-word-field")
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let View::Edit(edit) = &state.view else {
        return;
    };
    let mut edit = edit.clone();
    let t = state.t();
    let lang = state.settings.lang;
    let is_light = !ui.visuals().dark_mode;
    let ctx = ui.ctx().clone();
    // Read before anything here draws a popup: the folder list closes itself on this frame's Esc,
    // and that Esc must not also close the form behind it.
    let popup_was_open = egui::Popup::is_any_open(&ctx);
    // A blank's bubble answers its own Esc, like the discard question.
    let bubble_open = ctx.data(|d| d.get_temp::<(String, u64)>(bubble_key())).is_some();

    let frame = ctx.cumulative_frame_nr();
    let arriving = ctx
        .data(|d| d.get_temp::<u64>(seen_key()))
        .is_none_or(|last| last + 1 < frame);
    ctx.data_mut(|d| d.insert_temp(seen_key(), frame));
    if arriving {
        ctx.data_mut(|d| {
            d.insert_temp(original_key(), edit.clone());
            d.remove::<bool>(confirm_discard_key());
            d.remove::<(String, u64)>(bubble_key());
            d.remove::<String>(naming_key());
            d.remove::<Vec<(String, String, String)>>(preview_answers_key());
            d.remove::<bool>(step_touched_key(2));
        });
        // A new expansion starts at its word: the box that has the caret is the first step, and
        // nobody has to be told where to begin.
        if edit.editing_index.is_none() {
            ctx.memory_mut(|m| m.request_focus(word_field_id()));
        }
    }
    let dirty = ctx
        .data(|d| d.get_temp::<EditState>(original_key()))
        .is_none_or(|original| original != edit);
    let confirming = ctx.data(|d| d.get_temp::<bool>(confirm_discard_key())).unwrap_or(false);

    // Worked out before a single widget is drawn, so that the message inside the form and the state
    // of the button at the foot are one answer rather than two that agree most of the time. Both
    // describe the form as the frame found it; a keystroke arriving this frame is answered on the
    // next one, which `show_form` asks for as soon as anything changes.
    let problem = problem_with(state, &edit);
    let can_save = problem.is_none();

    let mut save = false;
    let mut cancel = false;
    let mut delete = false;

    // The title, centred like the library's, above everything that scrolls.
    egui::Panel::top("editor-title")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::default().inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 0,
            bottom: controls::GAP_SECTION as i8,
        }))
        .show(ui, |ui| {
            let title = if edit.editing_index.is_none() {
                t.edit_title_new
            } else {
                t.edit_title_existing
            };
            ui.set_min_height(controls::FIELD_HEIGHT);
            ui.vertical_centered(|ui| {
                ui.add(egui::Label::new(controls::h1(title)).wrap());
            });
        });

    // The actions get a strip of their own at the foot of the window, in the same place whatever
    // the form's height: a form long enough to scroll must not be able to scroll its own Guardar
    // out of reach.
    egui::Panel::bottom("editor-actions")
        .resizable(false)
        .frame(egui::Frame::default().inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: controls::GAP_SECTION as i8,
            bottom: 2,
        }))
        .show(ui, |ui| {
            // Rule and buttons both keep to the form's own width, preview and all, so the strip
            // reads as the foot of the form rather than of the window.
            let full = ui.available_width();
            let (margin, span, _) = columns(full);
            // The rule sits on the panel's own top edge, which is the inner margin above where the
            // content begins — it separates the strip from the form, so it cannot be inside it.
            let edge = ui.max_rect();
            ui.painter().hline(
                (edge.left() + margin)..=(edge.left() + margin + span),
                edge.top() - controls::GAP_SECTION,
                egui::Stroke::new(1.0, crate::app::hairline(is_light)),
            );
            ui.horizontal(|ui| {
                ui.set_max_width(full - margin);
                ui.add_space(margin);
                ui.spacing_mut().item_spacing.x = controls::GAP_ROW;
                let gap = controls::GAP_ROW;
                // The buttons on the right are measured first, so the folder and the name on the
                // left know how much of the strip they may take.
                let mut right = controls::button_width(ui, t.save) + gap + controls::button_width(ui, t.cancel);
                if edit.editing_index.is_some() {
                    right += controls::GAP_SECTION + gap + controls::button_width(ui, t.delete);
                }
                let room = span - right - controls::FIELD_HEIGHT - 3.0 * gap - controls::GAP_SECTION;
                let each = (room * 0.5).clamp(controls::FIELD_HEIGHT, EXTRA_MAX_WIDTH);
                extras(ui, state, &mut edit, each);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let saving = controls::button(ui, t.save, Tone::Primary, can_save);
                    save = match &problem {
                        // Says why it cannot be pressed at the moment somebody tries to press it,
                        // which is a different moment from reading the form.
                        Some(problem) => saving.on_disabled_hover_text(problem_text(state, t, problem)),
                        None => saving.on_hover_text(keys::tip(t.save, keys::SAVE, lang)),
                    }
                    .clicked();
                    cancel = controls::button(ui, t.cancel, Tone::Normal, true)
                        .on_hover_text(keys::tip(t.cancel, keys::BACK, lang))
                        .clicked();
                    if edit.editing_index.is_some() {
                        // Set apart from Guardar: it is the one action here that cannot be taken
                        // back from this screen.
                        ui.add_space(controls::GAP_SECTION);
                        delete = controls::button(ui, t.delete, Tone::Danger, true)
                            .on_hover_text(t.delete_tip)
                            .clicked();
                    }
                });
            });
        });

    let body = ui.available_rect_before_wrap();
    let (margin, span, side) = columns(body.width());
    let width = side.map_or(span, |side| span - side - PART_GAP);
    // With the preview beside it, the form scrolls in a column of its own, scroll bar and all.
    let form_rect = match side {
        Some(_) => egui::Rect::from_min_max(body.min, egui::pos2(body.left() + margin + width, body.bottom())),
        None => body,
    };
    ui.scope_builder(egui::UiBuilder::new().max_rect(form_rect), |ui| {
        controls::page_scroll(ui, "editor-body", |ui| {
            ui.horizontal(|ui| {
                // The margin is the whole point of the horizontal, so nothing else may add to it.
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(margin);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    show_form(ui, state, edit, problem.as_ref(), side.is_none(), is_light);
                });
            });
        });
    });
    // Drawn after the form, so it already shows this frame's keystroke.
    if let (Some(side), View::Edit(edit)) = (side, &state.view) {
        let column = egui::Rect::from_min_size(
            egui::pos2(form_rect.right() + PART_GAP, body.top()),
            egui::vec2(side, body.height()),
        );
        let edit = edit.clone();
        ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
            // What the card holds around the box: its padding, the heading and the sentence under it.
            let around = 150.0;
            // A form's blanks follow the box, and may run past the window: they scroll, in a column
            // of their own like the form's.
            egui::ScrollArea::vertical().id_salt("editor-side").auto_shrink([false, true]).show(ui, |ui| {
                preview_pair(ui, state, &edit, (column.height() - around).max(PREVIEW_MAX_HEIGHT), is_light);
            });
        });
    }

    // Read on every frame so the key never falls through to another screen, and acted on only when
    // the button beside it could have been pressed: the shortcut is the button, not a way past it.
    // Not under a dialog either: under «Eliminar expansión» it saved the form and left the question
    // asking about a trigger that, renamed, no longer existed.
    let dialog_up = confirming || state.pending_confirm.is_some();
    let ctrl_s = keys::pressed(&ctx, keys::SAVE);
    save |= ctrl_s && can_save && !dialog_up;

    // Esc is Cancelar. Not while a list or a bubble is open over the form — that Esc closes it —
    // nor while a dialog is up, which answers its own Esc.
    let blocked = popup_was_open || bubble_open || dialog_up;
    if !blocked && keys::pressed(&ctx, keys::BACK) {
        cancel = true;
    }

    // Leaving a form that has been written in is asked about; leaving one that has not is not.
    // A question every time would teach people to answer it without reading.
    let mut leave = false;
    if cancel {
        if dirty {
            ctx.data_mut(|d| d.insert_temp(confirm_discard_key(), true));
        } else {
            leave = true;
        }
    }
    if confirming && discard_dialog(&ctx, state) {
        leave = true;
    }

    if let View::Edit(edit) = &state.view {
        let edit = edit.clone();
        if save {
            let trigger = edit.trigger();
            state.save_edit(&edit);
            // Only on the way out. A refused save leaves the form open with its banner, and the
            // library has nothing to point at.
            if matches!(state.view, View::List) {
                super::list_view::mark_saved(&ctx, &trigger);
            }
        } else if leave {
            ctx.data_mut(|d| d.remove::<bool>(confirm_discard_key()));
            state.view = View::List;
        } else if delete {
            if let Some(index) = edit.editing_index {
                state.request_delete(index);
            }
        }
    }
}

/// Where the form goes in a body `full` wide: its left margin, the width of the form and the
/// preview beside it, and the preview's width when the window has room for it. Both are centred
/// as one block, the way the form alone was.
fn columns(full: f32) -> (f32, f32, Option<f32>) {
    let side = (full >= SIDE_FORM_MIN + PART_GAP + SIDE_WIDTH).then_some(SIDE_WIDTH);
    let span = match side {
        Some(side) => (full - PART_GAP - side).min(FORM_MAX_WIDTH) + PART_GAP + side,
        None => full.min(FORM_MAX_WIDTH),
    };
    (((full - span) * 0.5).max(0.0), span, side)
}

/// «¿Descartar los cambios?» Returns true once the answer is to throw them away; answering the
/// other way, pressing Esc or clicking beside the dialog puts the form back as it was.
fn discard_dialog(ctx: &egui::Context, state: &AppState) -> bool {
    let mut discard = false;
    let mut stay = false;
    let modal = controls::dialog(ctx, "editor-discard").show(ctx, |ui| {
        let is_light = !ui.visuals().dark_mode;
        ui.set_width(controls::dialog_width(ctx, 380.0));
        ui.label(controls::h3(studio::text(
            state,
            "¿Descartar los cambios?",
            "Discard your changes?",
            "Itapon ang mga pagbabago?",
            "बदलाव छोड़ दें?",
        )));
        ui.add_space(controls::GAP);
        ui.add(
            egui::Label::new(controls::muted(
                studio::text(
                    state,
                    "Lo que escribiste en este formulario no se guardará.",
                    "What you wrote in this form will not be saved.",
                    "Hindi mase-save ang isinulat mo sa form na ito.",
                    "इस फ़ॉर्म में जो लिखा है वह सहेजा नहीं जाएगा।",
                ),
                is_light,
            ))
            .wrap(),
        );
        ui.add_space(controls::GAP_STACK);
        controls::dialog_buttons(ui, |ui| {
            let keep = studio::text(
                state,
                "Seguir editando",
                "Keep editing",
                "Ituloy ang pag-edit",
                "संपादन जारी रखें",
            );
            if controls::button(ui, keep, Tone::Primary, true).clicked() {
                stay = true;
            }
            let drop = studio::text(state, "Descartar", "Discard", "Itapon", "छोड़ें");
            if controls::button(ui, drop, Tone::Danger, true).clicked() {
                discard = true;
            }
        });
    });
    if modal.should_close() || stay {
        ctx.data_mut(|d| d.remove::<bool>(confirm_discard_key()));
    }
    discard
}

/// The disc a step's number sits in, and how far the step's contents stand in from the left so
/// they start under its question rather than under its number.
const STEP_DOT: f32 = 26.0;
const STEP_INDENT: f32 = STEP_DOT + controls::GAP_WIDE;
/// How faint a step is while the one before it is still empty. Faint enough to read as «later»,
/// not so faint that it reads as switched off: it can still be filled in.
const STEP_FAINT: f32 = 0.45;

/// A step's number in its disc and its question beside it.
fn step_heading(ui: &mut egui::Ui, number: usize, question: &str) {
    let (fill, ink) = crate::app::accent_fill(ui.visuals());
    let mut job = egui::text::LayoutJob::default();
    controls::h3(question).append_to(&mut job, ui.style(), egui::FontSelection::Default, egui::Align::Center);
    job.wrap.max_width = (ui.available_width() - STEP_INDENT).max(1.0);
    let galley = ui.painter().layout_job(job);
    // The disc is centred on the question's first line, and a question that wraps hangs below it.
    let first = galley.rows.first().map_or(galley.size().y, |row| row.height());
    let line = first.max(STEP_DOT);
    let height = galley.size().y + (line - first);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    // Its own id rather than the next automatic one, which the faint scope around it can share with
    // the step before.
    let response = ui.interact(rect, egui::Id::new(("editor-step-heading", number)), egui::Sense::hover());
    let center = egui::pos2(rect.left() + STEP_DOT * 0.5, rect.top() + line * 0.5);
    ui.painter().circle_filled(center, STEP_DOT * 0.5, fill);
    ui.painter().text(
        center,
        egui::Align2::CENTER_CENTER,
        number.to_string(),
        egui::TextStyle::Body.resolve(ui.style()),
        ink,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + STEP_INDENT, center.y - first * 0.5),
        galley,
        ui.visuals().text_color(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, question));
}

/// What a step holds, set in under its question, and as faint as the step is.
fn step_body<R>(ui: &mut egui::Ui, opacity: f32, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let spacing = ui.spacing().item_spacing;
    ui.horizontal_top(|ui| {
        // The indent is the whole point of the horizontal, so nothing else may add to it.
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(STEP_INDENT);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = spacing;
            ui.set_width(ui.available_width());
            ui.set_opacity(opacity);
            body(ui)
        })
        .inner
    })
    .inner
}

/// How faint step `number` is this frame: eased between faint and full, so filling in the step
/// before it brings this one up rather than switching it on.
fn step_opacity(ui: &egui::Ui, number: usize, lit: bool) -> f32 {
    let t = ui.ctx().animate_bool_with_time(egui::Id::new(("editor-step-lit", number)), lit, 0.2);
    egui::lerp(STEP_FAINT..=1.0, t)
}

/// Set once someone has clicked into step `number` or moved the focus into it. From then on it is
/// lit whether the step before it is filled or not: a step somebody is working in is not «later».
fn step_touched_key(number: usize) -> egui::Id {
    egui::Id::new(("editor-step-touched", number))
}

/// Whether step `number` has been worked in since the editor opened.
fn step_touched(ui: &egui::Ui, number: usize) -> bool {
    ui.data(|d| d.get_temp::<bool>(step_touched_key(number))).unwrap_or(false)
}

/// Marks step `number` as worked in when a press lands in `area` — its question and all it holds —
/// or the focused widget sits in it, by mouse or by Tab.
fn note_step_touch(ui: &egui::Ui, number: usize, area: egui::Rect) {
    if step_touched(ui, number) {
        return;
    }
    let ctx = ui.ctx();
    let pressed = ctx.input(|i| i.pointer.any_pressed() && i.pointer.press_origin().is_some_and(|p| area.contains(p)));
    let focused = ctx
        .memory(|m| m.focused())
        .and_then(|id| ctx.read_response(id))
        .is_some_and(|r| area.contains(r.rect.center()));
    if pressed || focused {
        ui.data_mut(|d| d.insert_temp(step_touched_key(number), true));
        ctx.request_repaint();
    }
}

/// What you type: the two boxes that make the trigger, what they add up to, and anything wrong with it.
fn when_you_type(
    ui: &mut egui::Ui,
    state: &AppState,
    edit: &mut EditState,
    problem: Option<&Problem>,
    is_light: bool,
) {
    let t = state.t();
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        // Prefix and word are separate boxes so it is obvious which part is which — and so that
        // "apply this prefix everywhere" has an unambiguous piece to replace.
        //
        // Sized by their own margin rather than by `add_sized`. Handing `add_sized` a taller box
        // only stretches the frame: the text stays pinned to the top with the extra height below
        // it, which is exactly why the words in these two boxes once looked like they had floated
        // upward.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP;
            let prefix = ui.add(
                controls::text_field(&mut edit.prefix)
                    .desired_width(PREFIX_FIELD_WIDTH)
                    .hint_text(t.prefix_field_hint),
            );
            controls::say_as(&prefix, studio::text(state, "Prefijo", "Prefix", "Prefix", "उपसर्ग"));
            let word = ui.add(
                controls::text_field(&mut edit.word)
                    .id(word_field_id())
                    .desired_width(ui.available_width().max(90.0))
                    .hint_text(t.word_field_hint),
            );
            controls::say_as(&word, studio::text(state, "Activador", "Trigger", "Trigger", "ट्रिगर"));
            // Enter is «next question»: on to the text, the way Tab would go.
            if word.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                // The text box takes the focus within this same frame, further down: left in the
                // queue, this Enter would open it with an empty first line.
                ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                ui.memory_mut(|m| m.request_focus(replace_id()));
            }
        });

        if let Some(problem) = problem.filter(|p| !matches!(p, Problem::EmptyList(_))) {
            ui.add_space(controls::GAP_ROW);
            let message = problem_text(state, t, problem);
            match problem {
                // Not an error. It is the step nobody has taken yet, and a brand new form that
                // opens already scolding in red is telling the user off for arriving.
                Problem::Empty => {
                    ui.label(controls::muted(message, is_light));
                }
                _ => controls::notice(ui, &message, Tone::Danger),
            }
        }

        // Said here, while it is still being typed, rather than at save time. By the time you press
        // Guardar you have already decided; the useful moment to learn that a trigger cannot work is
        // the moment you are choosing it. It does not block saving — you may be about to delete the
        // other one, and the app has no business guessing.
        if let Some((short, long)) =
            shadowing_pair(&state.match_file.entries, edit.editing_index, &edit.trigger())
        {
            ui.add_space(controls::GAP_ROW);
            ui.label(
                egui::RichText::new(fill(
                    t.trigger_shadow,
                    &[("short", &short), ("long", &long)],
                ))
                .color(crate::app::caution(is_light)),
            );
        }
    });
}

/// What appears: the text, with its blanks, the date and what was last copied as pills, and what it
/// will actually produce.
fn appears_here(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, inline_preview: bool, is_light: bool) {
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        // Out of the draft while it is drawn, so the bubbles can reach the rest of the draft at the
        // same time.
        let mut text = std::mem::take(&mut edit.replace);
        text_with_blanks(ui, state, edit, &mut text, is_light);
        edit.replace = text;

        // Here only when the window has no room for it beside the form.
        if inline_preview {
            ui.add_space(controls::GAP_STACK);
            preview_pair(ui, state, edit, PREVIEW_MAX_HEIGHT, is_light);
        }
    });
}

// --- Blanks: a text with fields to fill in ------------------------------------------------------
//
// A blank is written `[[name]]`, what was last copied `{{copiado}}` and the date `{{fecha}}`,
// because that is what the file holds and what espanso reads. The box shows none of that: each
// marker is drawn as a pill with only its name in it, and an edit treats it as one character — it
// goes whole or not at all, nothing typed lands inside it, and the caret steps over it. A blank, or
// the date, is changed in the bubble a click on its pill opens, never by typing into its brackets.

fn replace_id() -> egui::Id {
    egui::Id::new("editor-replace")
}

/// How long a new pill takes to pop into place, and to fade in as it starts.
const POP_FOR: f32 = 0.45;
const POP_FADE: f32 = 0.08;

/// The markers in the text last frame, with that frame's number, and the pills still popping in
/// with the moment each appeared.
fn pills_seen_key() -> egui::Id {
    egui::Id::new("editor-pills-seen")
}

fn pill_pops_key() -> egui::Id {
    egui::Id::new("editor-pill-pops")
}

/// The pills popping in this frame. A pill pops when the text gains one: put in by a button,
/// pasted, made from a selection, or brought back by undo. Not when one is renamed or moved, where
/// the count stays the same, nor when the editor opens on a text that already has them.
fn pill_pops(ctx: &egui::Context, now: &[Pill]) -> Vec<(Marker, f64)> {
    let time = ctx.input(|i| i.time);
    let frame = ctx.cumulative_frame_nr();
    let current: Vec<Marker> = now.iter().map(|p| p.what.clone()).collect();
    let seen = ctx
        .data(|d| d.get_temp::<(Vec<Marker>, u64)>(pills_seen_key()))
        .filter(|(_, at)| at + 1 >= frame)
        .map(|(seen, _)| seen);
    let mut pops: Vec<(Marker, f64)> = ctx.data(|d| d.get_temp(pill_pops_key())).unwrap_or_default();
    if let Some(seen) = seen.filter(|seen| current.len() > seen.len()) {
        pops.extend(current.iter().filter(|m| !seen.contains(m)).map(|m| (m.clone(), time)));
    }
    pops.retain(|(what, at)| time - at < f64::from(POP_FOR) && current.contains(what));
    ctx.data_mut(|d| {
        d.insert_temp(pills_seen_key(), (current, frame));
        d.insert_temp(pill_pops_key(), pops.clone());
    });
    pops
}

/// How big a popping pill is `t` seconds in: from seven tenths of its size, past it by a few
/// percent, and back, the way a spring lets go.
fn pop_scale(t: f32) -> f32 {
    const OMEGA: f32 = 22.0;
    const DAMPING: f32 = 0.45;
    const FROM: f32 = 0.7;
    let damped = OMEGA * (1.0 - DAMPING * DAMPING).sqrt();
    1.0 - (1.0 - FROM) * (-DAMPING * OMEGA * t).exp() * (damped * t).cos()
}

/// The pill whose bubble is open, and the frame it was opened on: the click that opens it must not
/// also count as a click outside it.
fn bubble_key() -> egui::Id {
    egui::Id::new("editor-bubble")
}

/// The pill being dragged to somewhere else in the text, by the chars it covers.
fn drag_key() -> egui::Id {
    egui::Id::new("editor-drag")
}

/// A blank just made, until its bubble is drawn and its name box takes the caret.
fn naming_key() -> egui::Id {
    egui::Id::new("editor-naming")
}

/// Whether «Preguntar esto» was under the pointer on the last frame. Pressing it takes the
/// focus from the text, and the button has to outlive that press to be clicked.
fn convert_hover_key() -> egui::Id {
    egui::Id::new("editor-convert-hover")
}

/// The answers typed into the preview: each blank's name, the value it started with, and what is
/// in it now.
fn preview_answers_key() -> egui::Id {
    egui::Id::new("editor-preview-answers")
}

/// How wide the bubble is at least: room for a name and its starting value without wrapping.
const BUBBLE_MIN_WIDTH: f32 = 280.0;

/// What a pill stands for.
#[derive(Debug, Clone, PartialEq)]
enum Marker {
    /// A blank, by name.
    Blank(String),
    Copied,
    Date,
}

/// A marker in the text as the box shows it: one piece, as wide as its name.
#[derive(Debug, Clone, PartialEq)]
struct Pill {
    /// Where it sits, in chars.
    chars: std::ops::Range<usize>,
    what: Marker,
}

/// Every marker in the text, in order.
fn pills(text: &str, copied: &str, date: &str) -> Vec<Pill> {
    let mut at = 0;
    let mut found = Vec::new();
    for piece in form::pieces(text, Some(copied), Some(date)) {
        let (part, what) = match piece {
            form::Piece::Text(part) => {
                at += part.chars().count();
                continue;
            }
            form::Piece::Blank(name, part) => (part, Marker::Blank(name.to_string())),
            form::Piece::Copied(part) => (part, Marker::Copied),
            form::Piece::Date(part) => (part, Marker::Date),
        };
        let n = part.chars().count();
        found.push(Pill { chars: at..at + n, what });
        at += n;
    }
    found
}

/// The text with every marker's name in the accent, so what changes each time stands out from what
/// is fixed. A marker's brackets are laid out but not seen: the pill drawn over them is its edge,
/// and the caret still steps over them.
fn highlighted(ui: &egui::Ui, text: &str, copied: &str, date: &str, ink: egui::Color32, wrap: f32) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let blank = crate::app::accent(ui.visuals());
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut add = |part: &str, color| job.append(part, 0.0, egui::TextFormat::simple(font.clone(), color));
    for piece in form::pieces(text, Some(copied), Some(date)) {
        match piece {
            form::Piece::Text(part) => add(part, ink),
            // Every marker opens and closes with two ASCII characters.
            form::Piece::Blank(_, part) | form::Piece::Copied(part) | form::Piece::Date(part) => {
                let (open, rest) = part.split_at(2);
                let (name, close) = rest.split_at(rest.len() - 2);
                add(open, egui::Color32::TRANSPARENT);
                add(name, blank);
                add(close, egui::Color32::TRANSPARENT);
            }
        }
    }
    // An empty box still has a line, with the cursor the height of the text it is waiting for.
    if job.sections.is_empty() {
        job.append("", 0.0, egui::TextFormat::simple(font, ink));
    }
    job
}

/// The text an edit left, put right where it cut into a marker: a pill goes whole or not at all,
/// and nothing typed lands inside one. `None` when every marker came through whole.
///
/// The edit is found by comparing the two texts. Where repeated letters let it sit in more than
/// one place — a `[` typed just before `[[name]]` — the caret says which: after an edit it sits
/// right after what was typed, and where a deletion began.
fn whole_markers(before: &str, after: &str, caret: Option<usize>, pills: &[Pill]) -> Option<(String, usize)> {
    let old: Vec<char> = before.chars().collect();
    let new: Vec<char> = after.chars().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    let (mut start, mut end) = (prefix, old.len() - suffix);
    let mut typed = prefix..new.len() - suffix;
    if let Some(caret) = caret {
        let added = new.len().saturating_sub(old.len());
        let removed = old.len().saturating_sub(new.len());
        if let Some(at) = caret.checked_sub(added) {
            let fits = at + removed <= old.len() && caret <= new.len();
            if fits && old[..at] == new[..at] && old[at + removed..] == new[caret..] {
                (start, end, typed) = (at, at + removed, at..caret);
            }
        }
    }
    let mut moved = false;
    for pill in pills {
        let r = &pill.chars;
        if start == end {
            if r.start < start && start < r.end {
                (start, end) = (r.end, r.end);
                moved = true;
            }
        } else if start < r.end && r.start < end && !(start <= r.start && r.end <= end) {
            (start, end) = (start.min(r.start), end.max(r.end));
            moved = true;
        }
    }
    moved.then(|| {
        let text = old[..start].iter().chain(&new[typed.clone()]).chain(&old[end..]).collect();
        (text, start + typed.len())
    })
}

/// Where a caret or selection that landed inside a pill goes instead: to the pill's edges.
///
/// A selection grows outward, so it takes the whole pill. A caret moved by the keyboard carries
/// on through it, in the direction it was going (`from` is where it was); one put there by the
/// pointer goes to the nearer edge.
fn snapped(primary: usize, secondary: usize, from: usize, pointer: bool, pills: &[Pill]) -> (usize, usize) {
    let inside = |at: usize| pills.iter().find(|p| p.chars.start < at && at < p.chars.end).map(|p| p.chars.clone());
    let nearer = |at: usize, r: &std::ops::Range<usize>| if at - r.start < r.end - at { r.start } else { r.end };
    let caret = |at: usize| match inside(at) {
        None => at,
        Some(r) if pointer => nearer(at, &r),
        Some(r) if from <= r.start => r.end,
        Some(r) if from >= r.end => r.start,
        Some(r) => nearer(at, &r),
    };
    if primary == secondary {
        let at = caret(primary);
        return (at, at);
    }
    let away = |at: usize, up: bool| inside(at).map_or(at, |r| if up { r.end } else { r.start });
    let primary = if pointer { away(primary, primary > secondary) } else { caret(primary) };
    (primary, away(secondary, secondary > primary))
}

/// What a new question is called before it is renamed: «Pregunta 1», «Pregunta 2»… Never a name in
/// `taken`, which is every name this editor has seen: one freed by a deleted question still has
/// its answer in the preview and its draft in the name box, and a new question under it read as
/// the old one come back.
fn new_blank_name(state: &AppState, taken: &[String]) -> String {
    let word = studio::text(state, "Pregunta", "Question", "Tanong", "सवाल");
    (1..)
        .map(|n| format!("{word} {n}"))
        .find(|name| !taken.contains(name))
        .unwrap_or_default()
}

/// The byte where char `at` begins, or the end of the text.
fn byte_at(text: &str, at: usize) -> usize {
    text.char_indices().nth(at).map_or(text.len(), |(b, _)| b)
}

/// The text with the pill over `held` taken out and put in at char `at` of the text as it was, and
/// where the caret goes: right after the pill.
fn moved(text: &str, held: std::ops::Range<usize>, at: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let pill = &chars[held.clone()];
    let mut rest: Vec<char> = chars[..held.start].iter().chain(&chars[held.end..]).copied().collect();
    let at = if at > held.end { at - pill.len() } else { at };
    rest.splice(at..at, pill.iter().copied());
    (rest.into_iter().collect(), at + pill.len())
}

/// Puts `what` over `chars` in the text, leaves the caret after it and the focus in the box.
fn put(ctx: &egui::Context, text: &mut String, chars: std::ops::Range<usize>, what: &str) {
    let range = byte_at(text, chars.start)..byte_at(text, chars.end);
    text.replace_range(range, what);
    let mut field = egui::TextEdit::load_state(ctx, replace_id()).unwrap_or_default();
    let after = egui::text::CCursor::new(chars.start + what.chars().count());
    field.cursor.set_char_range(Some(egui::text::CCursorRange::one(after)));
    field.store(ctx, replace_id());
    ctx.memory_mut(|m| m.request_focus(replace_id()));
}

/// The selection the box last had, as chars in order, or where its caret was; the end of the text
/// if it never had one.
fn selection(ctx: &egui::Context, text: &str) -> std::ops::Range<usize> {
    let chars = text.chars().count();
    egui::TextEdit::load_state(ctx, replace_id())
        .and_then(|field| field.cursor.char_range())
        .map_or(chars..chars, |range| {
            let (a, b) = (range.primary.index.0.min(chars), range.secondary.index.0.min(chars));
            a.min(b)..a.max(b)
        })
}

/// The question a selection most likely answers, when it plainly says: a weekday asks «¿Qué
/// día?», an hour «¿A qué hora?», an address «¿Qué correo?». Every option of a list has to agree.
/// `None` when it does not plainly say, and the selection names the question itself, as before.
fn guessed_question(picked: &[&str], lang: Lang) -> Option<&'static str> {
    let kinds: Vec<Option<Guess>> = picked.iter().map(|p| guess_of(p)).collect();
    let first = kinds.first().copied().flatten()?;
    if !kinds.iter().all(|k| *k == Some(first)) {
        return None;
    }
    let ask = |es, en, fil, hi| studio::in_lang(lang, es, en, fil, hi);
    Some(match first {
        Guess::Day => ask("¿Qué día?", "Which day?", "Anong araw?", "कौन-सा दिन?"),
        Guess::Month => ask("¿Qué mes?", "Which month?", "Anong buwan?", "कौन-सा महीना?"),
        Guess::Date => ask("¿Qué fecha?", "Which date?", "Anong petsa?", "कौन-सी तारीख?"),
        Guess::Time => ask("¿A qué hora?", "What time?", "Anong oras?", "किस समय?"),
        Guess::Email => ask("¿Qué correo?", "Which email?", "Anong email?", "कौन-सा ईमेल?"),
        Guess::Link => ask("¿Qué enlace?", "Which link?", "Anong link?", "कौन-सा लिंक?"),
        Guess::Phone => ask("¿Qué teléfono?", "Which phone number?", "Anong numero ng telepono?", "कौन-सा फ़ोन नंबर?"),
        Guess::Money => ask("¿Cuánto?", "How much?", "Magkano?", "कितना?"),
        Guess::Count => ask("¿Cuántos?", "How many?", "Ilan?", "कितने?"),
        Guess::Greeting => ask("¿Qué saludo?", "Which greeting?", "Anong pagbati?", "कौन-सा अभिवादन?"),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Guess {
    Day,
    Month,
    Date,
    Time,
    Email,
    Link,
    Phone,
    Money,
    Count,
    Greeting,
}

/// What kind of thing one piece of text is, in any of the four languages, if it is plainly one.
fn guess_of(text: &str) -> Option<Guess> {
    const DAYS: &[&str] = &[
        "lunes", "martes", "miércoles", "miercoles", "jueves", "viernes", "sábado", "sabado", "domingo",
        "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
        "lunes", "martes", "miyerkules", "huwebes", "biyernes", "sabado", "linggo",
        "सोमवार", "मंगलवार", "बुधवार", "गुरुवार", "शुक्रवार", "शनिवार", "रविवार",
        "hoy", "mañana", "manana", "today", "tomorrow", "ngayon", "bukas", "आज", "कल",
    ];
    const MONTHS: &[&str] = &[
        "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre",
        "setiembre", "octubre", "noviembre", "diciembre", "january", "february", "march", "april",
        "may", "june", "july", "august", "september", "october", "november", "december", "enero",
        "pebrero", "marso", "abril", "mayo", "hunyo", "hulyo", "agosto", "setyembre", "oktubre",
        "nobyembre", "disyembre", "जनवरी", "फ़रवरी", "फरवरी", "मार्च", "अप्रैल", "मई", "जून", "जुलाई",
        "अगस्त", "सितंबर", "अक्टूबर", "नवंबर", "दिसंबर",
    ];
    const GREETINGS: &[&str] = &[
        "buenos días", "buenos dias", "buenas tardes", "buenas noches", "hola", "good morning",
        "good afternoon", "good evening", "hello", "hi", "magandang umaga", "magandang hapon",
        "magandang gabi", "नमस्ते", "सुप्रभात", "शुभ संध्या",
    ];
    let t = text.trim().trim_end_matches(['.', ',', '!', '¡', ';', ':']).trim_start_matches('¡').trim();
    let lower = t.to_lowercase();
    if lower.is_empty() || lower.contains('\n') {
        return None;
    }
    if DAYS.contains(&lower.as_str()) {
        return Some(Guess::Day);
    }
    if MONTHS.contains(&lower.as_str()) {
        return Some(Guess::Month);
    }
    if GREETINGS.contains(&lower.as_str()) {
        return Some(Guess::Greeting);
    }
    if lower.contains(char::is_whitespace) {
        return None;
    }
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www.") {
        return Some(Guess::Link);
    }
    if let Some((user, host)) = lower.split_once('@') {
        return (!user.is_empty() && host.contains('.') && !host.starts_with('.')).then_some(Guess::Email);
    }
    let digits = lower.chars().filter(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let only = |allowed: &str| lower.chars().all(|c| c.is_ascii_digit() || allowed.contains(c));
    let parts = |sep: char| lower.split(sep).count();
    if only(":") && parts(':') == 2 || only(":apm.") && lower.contains(':') && (lower.ends_with("am") || lower.ends_with("pm")) {
        return Some(Guess::Time);
    }
    if (only("/") && parts('/') == 3) || (only("-") && parts('-') == 3 && digits <= 8) {
        return Some(Guess::Date);
    }
    if lower.starts_with(['$', '€', '£', '₱', '₹']) && only("$€£₱₹.,") {
        return Some(Guess::Money);
    }
    if only("+-().") && digits >= 7 {
        return Some(Guess::Phone);
    }
    if only("") && digits <= 4 {
        return Some(Guess::Count);
    }
    None
}

/// Fills an empty box with a short text that asks two questions, one typed and one picked from
/// options, in the interface's language: the quickest way to see what a question is.
fn write_example(ctx: &egui::Context, state: &AppState, edit: &mut EditState, text: &mut String) {
    let lang = state.settings.lang;
    let ask = |es, en, fil, hi| studio::in_lang(lang, es, en, fil, hi);
    let who = ask("¿A quién?", "Who to?", "Para kanino?", "किसे?").to_string();
    let day = ask("¿Qué día?", "Which day?", "Anong araw?", "कौन-सा दिन?").to_string();
    let days: Vec<String> = ask(
        "lunes / martes / miércoles / jueves / viernes",
        "Monday / Tuesday / Wednesday / Thursday / Friday",
        "Lunes / Martes / Miyerkules / Huwebes / Biyernes",
        "सोमवार / मंगलवार / बुधवार / गुरुवार / शुक्रवार",
    )
    .split(" / ")
    .map(String::from)
    .collect();
    let template = ask(
        "Hola {who}, te confirmo nuestra reunión del {day}. ¡Saludos!",
        "Hi {who}, confirming our meeting on {day}. Best!",
        "Hi {who}, kinukumpirma ko ang meeting natin sa {day}. Salamat!",
        "नमस्ते {who}, {day} की हमारी मीटिंग पक्की है। धन्यवाद!",
    );
    let who_marker = edit.new_blank(who.clone());
    let day_marker = edit.new_blank(day.clone());
    *edit.field_mut(&who) = FormField { name: who.clone(), ..Default::default() };
    *edit.field_mut(&day) = FormField { name: day.clone(), kind: FieldKind::Choice, default: days[0].clone(), options: days };
    let written = template.replace("{who}", &who_marker).replace("{day}", &day_marker);
    put(ctx, text, 0..text.chars().count(), &written);
}

/// What a selection turned into a blank is called, and what kind of blank it makes.
///
/// The name is the question the selection plainly answers ([`guessed_question`]), or else its first
/// line, so the pill still reads like the text it replaced until it is renamed. Several lines make a long text; options split by « / » make a list, the
/// first one chosen; anything else is a short text that starts as what was selected. A list is
/// not named after its options, which would read as one more of them: it gets «Pregunta 1» unless
/// they all say what they answer.
fn blank_from(state: &AppState, taken: &[String], picked: &str) -> FormField {
    let options: Vec<String> = picked.split(" / ").map(|o| o.trim().to_string()).collect();
    let is_list = !picked.contains('\n') && options.len() > 1 && options.iter().all(|o| !o.is_empty());
    let first = if is_list { "" } else { picked.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("") };
    let asked: Vec<&str> = if is_list { options.iter().map(String::as_str).collect() } else { vec![picked] };
    let first = guessed_question(&asked, state.settings.lang).unwrap_or(first);
    let cleaned: String = first.chars().filter(|c| !matches!(c, '[' | ']')).take(form::NAME_MAX).collect();
    let base = cleaned.trim();
    let name = if base.is_empty() {
        None
    } else if !taken.iter().any(|n| n == base) {
        Some(base.to_string())
    } else {
        (2..100)
            .map(|n| format!("{base} {n}"))
            .find(|n| n.chars().count() <= form::NAME_MAX && !taken.contains(n))
    };
    let name = name.unwrap_or_else(|| new_blank_name(state, taken));
    if picked.contains('\n') {
        FormField { name, kind: FieldKind::Long, default: picked.to_string(), options: Vec::new() }
    } else if is_list {
        FormField { name, kind: FieldKind::Choice, default: options[0].clone(), options }
    } else {
        FormField { name, kind: FieldKind::Short, default: picked.trim().to_string(), options: Vec::new() }
    }
}

/// Turns the chars in `chars` into a blank and opens its bubble with the name ready to type over.
fn make_blank(ctx: &egui::Context, state: &AppState, edit: &mut EditState, text: &mut String, chars: std::ops::Range<usize>) {
    let picked = text[byte_at(text, chars.start)..byte_at(text, chars.end)].to_string();
    let mut taken = form::names(text);
    taken.extend(edit.fields.iter().map(|f| f.name.clone()));
    let field = if picked.trim().is_empty() {
        FormField { name: new_blank_name(state, &taken), ..Default::default() }
    } else {
        blank_from(state, &taken, &picked)
    };
    let marker = edit.new_blank(field.name.clone());
    let name = field.name.clone();
    *edit.field_mut(&name) = field;
    // An empty selection is a caret: the blank goes in beside it, and takes nothing with it.
    let over = if picked.trim().is_empty() { chars.end..chars.end } else { chars };
    put(ctx, text, over, &marker);
    open_bubble(ctx, Marker::Blank(name.clone()));
    ctx.data_mut(|d| d.insert_temp(naming_key(), name));
}

fn open_bubble(ctx: &egui::Context, what: Marker) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(bubble_key(), (what, frame)));
}

/// The text box with its pills and its right-click menu, the buttons that put a marker in it, and
/// the bubble of the blank, or of the date, being changed.
fn text_with_blanks(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, text: &mut String, is_light: bool) {
    let ctx = ui.ctx().clone();
    // A floor, not a box: the field gets at least this much and grows past it as the text does,
    // with the page scrolling rather than the words being cut off.
    let height = ui.text_style_height(&egui::TextStyle::Body) * TEXT_AREA_ROWS + 16.0;
    let ink = ui.visuals().widgets.inactive.text_color();
    let copied = edit.copied.clone();
    let date = edit.date.name.clone();
    let before = text.clone();
    let before_pills = pills(&before, &copied, &date);
    let last = egui::TextEdit::load_state(&ctx, replace_id()).and_then(|field| field.cursor.char_range());
    let from = last.map(|range| range.primary.index.0);
    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap: f32| {
        ui.painter().layout_job(highlighted(ui, buffer.as_str(), &copied, &date, ink, wrap))
    };
    let carrying = ctx.data(|d| d.get_temp::<std::ops::Range<usize>>(drag_key())).is_some();
    let layout = egui::Layout::centered_and_justified(ui.layout().main_dir());
    let mut output = ui
        .allocate_ui_with_layout(egui::vec2(ui.available_width(), height), layout, |ui| {
            controls::multiline_field(text).id(replace_id()).layouter(&mut layouter).interactive(!carrying).show(ui)
        })
        .inner;
    let response = output.response.response.clone();
    controls::say_as(&response, studio::text(state, "Texto", "Text", "Teksto", "टेक्स्ट"));

    // A right click inside the selection keeps it, as in every Windows text box, so the menu it
    // opens acts on it; the box itself would have dropped it for a caret under the pointer.
    let right_press = response.hovered() && ctx.input(|i| i.pointer.secondary_pressed());
    if let (true, Some(last), Some(at)) = (right_press, last, ctx.input(|i| i.pointer.interact_pos())) {
        let (a, b) = (last.primary.index.0, last.secondary.index.0);
        let under = output.galley.cursor_from_pos(at - output.galley_pos).index.0;
        if a != b && (a.min(b)..=a.max(b)).contains(&under) {
            output.state.cursor.set_char_range(Some(last));
            output.state.clone().store(&ctx, replace_id());
        }
    }

    // The galley is this frame's only while nothing below has to change the text it was laid out
    // from; when something does, the pills wait a frame for the new one.
    let mut fresh = true;
    if *text != before {
        let caret = output
            .state
            .cursor
            .char_range()
            .filter(|r| r.primary == r.secondary)
            .map(|r| r.primary.index.0);
        if let Some((kept, caret)) = whole_markers(&before, text, caret, &before_pills) {
            *text = kept;
            let at = egui::text::CCursor::new(caret);
            output.state.cursor.set_char_range(Some(egui::text::CCursorRange::one(at)));
            output.state.clone().store(&ctx, replace_id());
            fresh = false;
        }
    }
    let now = pills(text, &copied, &date);
    if let Some(range) = output.state.cursor.char_range() {
        let (primary, secondary) = (range.primary.index.0, range.secondary.index.0);
        let pointer = response.is_pointer_button_down_on() || response.clicked() || response.drag_stopped();
        let (p, s) = snapped(primary, secondary, from.unwrap_or(primary), pointer, &now);
        if (p, s) != (primary, secondary) {
            let range = egui::text::CCursorRange::two(egui::text::CCursor::new(s), egui::text::CCursor::new(p));
            output.state.cursor.set_char_range(Some(range));
            output.state.clone().store(&ctx, replace_id());
            ctx.request_repaint();
        }
    }
    if !fresh {
        ctx.request_repaint();
    }

    let mut open = ctx.data(|d| d.get_temp::<(Marker, u64)>(bubble_key()));
    let mut anchor = None;
    let mut convert = None;
    if fresh {
        let painter = ui.painter_at(output.text_clip_rect);
        let hover = response.hover_pos();
        let accent = crate::app::accent(ui.visuals());
        let mut on_pill = false;
        let pops = pill_pops(&ctx, &now);
        let time = ctx.input(|i| i.time);
        for pill in &now {
            let rects = controls::pill_rects(&output.galley, output.galley_pos, &pill.chars);
            let opens = pill.what != Marker::Copied;
            let hovered = opens && hover.is_some_and(|p| rects.iter().any(|r| r.contains(p)));
            if hovered && response.clicked() {
                let opened = (pill.what.clone(), ctx.cumulative_frame_nr());
                ctx.data_mut(|d| d.insert_temp(bubble_key(), opened.clone()));
                open = Some(opened);
            }
            let is_open = open.as_ref().is_some_and(|(what, _)| *what == pill.what);
            if is_open && anchor.is_none() {
                anchor = rects.first().copied();
            }
            on_pill |= hovered;
            let strength = match (is_open || hovered, is_light) {
                (false, true) => 0.14,
                (false, false) => 0.24,
                (true, true) => 0.24,
                (true, false) => 0.36,
            };
            let popped = pops.iter().find(|(what, _)| *what == pill.what).map(|(_, at)| (time - at) as f32);
            for rect in rects {
                let mut rect = rect.shrink2(egui::vec2(0.0, 1.0));
                let mut fill = accent.gamma_multiply(strength);
                if let Some(t) = popped {
                    rect = egui::Rect::from_center_size(rect.center(), rect.size() * pop_scale(t));
                    fill = fill.gamma_multiply((t / POP_FADE).min(1.0));
                }
                painter.rect_filled(rect, rect.height() * 0.5, fill);
            }
        }
        if !pops.is_empty() {
            ctx.request_repaint();
        }
        if on_pill && !carrying {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // A pill can be picked up and put somewhere else: dragged, it leaves a caret where it
        // would land, and goes there when let go. Meanwhile the box stops answering the pointer,
        // so the drag does not also paint a selection across the text.
        let mut held = ctx.data(|d| d.get_temp::<std::ops::Range<usize>>(drag_key()));
        if response.drag_started() {
            let origin = ctx.input(|i| i.pointer.press_origin());
            held = origin.and_then(|origin| {
                now.iter()
                    .find(|p| controls::pill_rects(&output.galley, output.galley_pos, &p.chars).iter().any(|r| r.contains(origin)))
                    .map(|p| p.chars.clone())
            });
            if held.is_some() {
                ctx.data_mut(|d| d.remove::<(Marker, u64)>(bubble_key()));
                open = None;
            }
        }
        if let Some(chars) = held.filter(|c| c.end <= text.chars().count()) {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            let drop = ctx.input(|i| i.pointer.latest_pos()).map(|p| {
                let at = output.galley.cursor_from_pos(p - output.galley_pos).index.0;
                match now.iter().find(|q| q.chars.start < at && at < q.chars.end) {
                    Some(q) if at - q.chars.start < q.chars.end - at => q.chars.start,
                    Some(q) => q.chars.end,
                    None => at,
                }
            });
            let drop = drop.filter(|at| *at < chars.start || *at > chars.end);
            if let Some(at) = drop {
                let caret = output.galley.pos_from_cursor(egui::text::CCursor::new(at));
                let caret = caret.translate(output.galley_pos.to_vec2());
                painter.vline(caret.left(), caret.y_range(), egui::Stroke::new(2.0, accent));
            }
            if ctx.input(|i| i.pointer.any_down()) {
                ctx.data_mut(|d| d.insert_temp(drag_key(), chars));
                ctx.request_repaint();
            } else {
                ctx.data_mut(|d| d.remove::<std::ops::Range<usize>>(drag_key()));
                let (moved_text, caret) = match drop {
                    Some(at) => moved(text, chars, at),
                    None => (text.clone(), chars.end),
                };
                *text = moved_text;
                output.state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(caret))));
                output.state.clone().store(&ctx, replace_id());
                ctx.memory_mut(|m| m.request_focus(replace_id()));
                ctx.request_repaint();
            }
        }

        // A selection can become a blank where it stands. The button floats over its first line
        // while the text has the focus, and survives the press that takes the focus away.
        let hovered_before = ctx.data(|d| d.get_temp::<bool>(convert_hover_key())).unwrap_or(false);
        let mut hovered_now = false;
        if let Some(range) = output.state.cursor.char_range().filter(|_| (response.has_focus() || hovered_before) && !response.context_menu_opened()) {
            let (a, b) = (range.primary.index.0, range.secondary.index.0);
            let chars = a.min(b)..a.max(b);
            let whole = !now.iter().any(|p| p.chars.start < chars.end && chars.start < p.chars.end);
            if !chars.is_empty() && whole {
                let first = output.galley.pos_from_cursor(egui::text::CCursor::new(chars.start));
                let at = output.galley_pos + first.left_top().to_vec2() - egui::vec2(0.0, controls::GAP_TIGHT);
                let label = studio::text(state, "Preguntar esto", "Ask for this", "Itanong ito", "यह पूछें");
                let button = egui::Area::new(egui::Id::new("editor-convert"))
                    .order(egui::Order::Foreground)
                    .pivot(egui::Align2::LEFT_BOTTOM)
                    .fixed_pos(at)
                    .show(&ctx, |ui| {
                        egui::Frame::popup(ui.style())
                            .show(ui, |ui| controls::subtle_button(ui, Some(Glyph::Rename), label, None))
                            .inner
                    })
                    .inner;
                hovered_now = button.hovered();
                if button.clicked() {
                    convert = Some(chars);
                }
            }
        }
        ctx.data_mut(|d| d.insert_temp(convert_hover_key(), hovered_now));
    }

    let mut add_blank = false;
    let mut add_date = false;
    // The right-click menu: the box's own commands, and a selection made a question right there.
    let mut cut = false;
    let mut copy = false;
    let mut paste = false;
    let mut example = false;
    let lang = state.settings.lang;
    let picked = selection(&ctx, text);
    let picked_whole = !now.iter().any(|p| p.chars.start < picked.end && picked.start < p.chars.end);
    // As wide as the list's own flyouts.
    egui::Popup::context_menu(&response).width(240.0).show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let some = !picked.is_empty();
        let label = studio::text(state, "Cortar", "Cut", "I-cut", "काटें");
        let keys_text = keys::text(keys::CUT, lang);
        cut = controls::menu_item(ui, Some(Glyph::Cut), label, Some(&keys_text), Tone::Normal, some).clicked();
        let label = studio::text(state, "Copiar", "Copy", "Kopyahin", "कॉपी करें");
        let keys_text = keys::text(keys::COPY, lang);
        copy = controls::menu_item(ui, Some(Glyph::Copy), label, Some(&keys_text), Tone::Normal, some).clicked();
        let label = studio::text(state, "Pegar", "Paste", "I-paste", "चिपकाएँ");
        let keys_text = keys::text(keys::PASTE, lang);
        paste = controls::menu_item(ui, Some(Glyph::Paste), label, Some(&keys_text), Tone::Normal, true).clicked();
        if some && picked_whole {
            controls::menu_separator(ui);
            let label = studio::text(state, "Preguntar esto", "Ask for this", "Itanong ito", "यह पूछें");
            if controls::menu_item(ui, Some(Glyph::Rename), label, None, Tone::Normal, true).clicked() {
                convert = Some(picked.clone());
            }
        }
    });

    // What can go into the text, each one button away.
    ui.add_space(controls::GAP);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = controls::GAP;
        let blank = studio::text(state, "Pregunta", "Question", "Tanong", "सवाल");
        let blank_tip = studio::text(
            state,
            "Khipu te la hace cada vez que usas la expansión, y tu respuesta se escribe aquí. Si hay texto seleccionado, se convierte en la pregunta.",
            "Khipu asks it each time you use the expansion, and your answer is written here. Selected text becomes the question.",
            "Itatanong ito ng Khipu tuwing gagamitin mo ang expansion, at dito isusulat ang sagot mo. Ang napiling teksto ang magiging tanong.",
            "हर बार विस्तार उपयोग करने पर Khipu यह पूछता है, और आपका जवाब यहाँ लिखा जाता है। चुना हुआ टेक्स्ट सवाल बन जाता है।",
        );
        add_blank = controls::subtle_button(ui, Some(Glyph::Add), blank, None).on_hover_text(blank_tip).clicked();
        let date = studio::text(state, "Fecha", "Date", "Petsa", "दिनांक");
        let date_tip = studio::text(
            state,
            "La fecha o la hora del momento en que uses la expansión",
            "The date or time at the moment you use the expansion",
            "Ang petsa o oras sa sandaling gamitin mo ang expansion",
            "विस्तार उपयोग करते समय की दिनांक या समय",
        );
        add_date = controls::subtle_button(ui, Some(Glyph::Calendar), date, None).on_hover_text(date_tip).clicked();
        // With nothing written yet, a whole example shows what the buttons are for.
        if text.trim().is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = studio::text(state, "Ver un ejemplo", "See an example", "Tingnan ang halimbawa", "उदाहरण देखें");
                example = controls::subtle_button(ui, None, label, None).clicked();
            });
        }
    });
    if example {
        write_example(&ctx, state, edit, text);
    }
    // «Pregunta» does what «Preguntar esto» does to a selection, and makes an
    // empty blank at the caret when there is none.
    if add_blank {
        convert = Some(if picked_whole { picked.clone() } else { picked.end..picked.end });
    }
    if let Some(chars) = convert {
        make_blank(&ctx, state, edit, text, chars);
    }
    // The date goes in at the caret with its bubble open, so how it is written is chosen right
    // there. One written in words starts out in the interface's language.
    if add_date {
        if !edit.date_in(text) && edit.date == DateChoice::default() {
            edit.date.month_lang = state.settings.lang;
        }
        let at = selection(&ctx, text).end;
        put(&ctx, text, at..at, &format!("{{{{{}}}}}", edit.date.name));
        open_bubble(&ctx, Marker::Date);
    }
    if cut || copy {
        ctx.copy_text(text[byte_at(text, picked.start)..byte_at(text, picked.end)].to_string());
        if cut {
            put(&ctx, text, picked.clone(), "");
        }
    }
    if paste {
        let pasted = crate::paste::copied_text();
        if !pasted.is_empty() {
            put(&ctx, text, picked.clone(), &pasted);
        }
    }

    match (open, anchor) {
        (Some((what, opened)), Some(anchor)) => {
            let fresh_click = opened == ctx.cumulative_frame_nr();
            let kept = match what {
                Marker::Blank(name) => {
                    bubble(ui, state, edit, text, &name, anchor, fresh_click, is_light).map(Marker::Blank)
                }
                Marker::Date => date_bubble(ui, state, edit, anchor, fresh_click).then_some(Marker::Date),
                Marker::Copied => None,
            };
            ctx.data_mut(|d| match kept {
                Some(what) => {
                    d.insert_temp(bubble_key(), (what, opened));
                }
                None => d.remove::<(Marker, u64)>(bubble_key()),
            });
        }
        // Its pill went from the text, or is waiting a frame for the new layout.
        (Some(_), None) if fresh => {
            ctx.data_mut(|d| d.remove::<(Marker, u64)>(bubble_key()));
        }
        _ => {}
    }
}

/// The bubble under a question's pill: what is asked, whether the answer is typed or picked from
/// options, and what it starts as. Returns the name the question goes by now, or `None` once the
/// bubble is closed.
#[allow(clippy::too_many_arguments)]
fn bubble(
    ui: &mut egui::Ui,
    state: &AppState,
    edit: &mut EditState,
    text: &mut String,
    name: &str,
    anchor: egui::Rect,
    fresh_click: bool,
    is_light: bool,
) -> Option<String> {
    let ctx = ui.ctx().clone();
    let field = edit.form(text)?.fields.into_iter().find(|f| f.name == name)?;
    // A short and a long answer are one kind now: the box in the form grows with what is typed.
    let kinds = [
        studio::text(state, "Respuesta escrita", "Typed answer", "Isusulat na sagot", "लिखा हुआ जवाब"),
        studio::text(state, "Opciones", "Options", "Mga pagpipilian", "विकल्प"),
    ];
    let width = controls::segmented_natural_width(ui, &kinds).max(BUBBLE_MIN_WIDTH);
    let mut kept = Some(name.to_string());
    let shown = egui::Popup::new(egui::Id::new("editor-bubble-popup"), ctx.clone(), anchor, ui.layer_id())
        .open(true)
        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
        .align(egui::RectAlign::BOTTOM_START)
        .align_alternatives(&[egui::RectAlign::TOP_START])
        .gap(controls::GAP_TIGHT)
        // A menu's 4-point padding is for rows that carry their own; boxes need room around them.
        .frame(egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(controls::GAP_WIDE as i8)))
        .show(|ui| {
            ui.set_width(width);
            let is_choice = field.kind == FieldKind::Choice;

            // The question is typed into a copy and written back only while it is a name the text
            // can hold: not empty, not another question's, nothing that would end the marker
            // early. The copy is kept with the name it was typed over, and dropped as soon as the
            // box lets go of the focus.
            let name_id = egui::Id::new("editor-blank-name");
            let draft_id = name_id.with("draft");
            let mut draft = if ctx.memory(|m| m.has_focus(name_id)) {
                ctx.data(|d| d.get_temp::<(String, String)>(draft_id))
                    .filter(|(over, _)| *over == field.name)
                    .map_or_else(|| field.name.clone(), |(_, draft)| draft)
            } else {
                ctx.data_mut(|d| d.remove::<(String, String)>(draft_id));
                field.name.clone()
            };
            // A question just made goes straight to its words, chosen whole, so the first thing
            // typed is the question: «Pregunta 1», or the words it was made from, are a starting
            // point. The box starts over, too: its undo must not bring back an earlier question.
            // Not on the bubble's first, unseen pass: what is drawn there cannot take the focus.
            let naming = !ui.is_sizing_pass()
                && ctx.data(|d| d.get_temp::<String>(naming_key())).is_some_and(|n| n == field.name);
            if naming {
                ctx.data_mut(|d| {
                    d.remove::<String>(naming_key());
                    d.remove::<(String, String)>(draft_id);
                });
                draft = field.name.clone();
                let mut name_box = egui::text_edit::TextEditState::default();
                let all = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(draft.chars().count()),
                );
                name_box.cursor.set_char_range(Some(all));
                name_box.store(&ctx, name_id);
                ctx.memory_mut(|m| m.request_focus(name_id));
            }
            let hint = studio::text(state, "Qué preguntar", "What to ask", "Ano ang itatanong", "क्या पूछना है");
            ui.label(controls::small_muted(hint, is_light));
            ui.add_space(controls::GAP_TIGHT);
            let example = studio::text(state, "¿Qué día?", "Which day?", "Anong araw?", "कौन-सा दिन?");
            let named = ui.add(
                controls::text_field(&mut draft)
                    .id(name_id)
                    .hint_text(example)
                    .desired_width(f32::INFINITY),
            );
            if named.changed() {
                let new = draft.trim();
                let valid = !new.is_empty()
                    && new.chars().count() <= form::NAME_MAX
                    && !new.contains(['[', ']', '\n', '\r'])
                    && !form::names(text).iter().any(|n| n == new);
                let over = if valid { new.to_string() } else { field.name.clone() };
                ctx.data_mut(|d| d.insert_temp(draft_id, (over, draft.clone())));
                if valid {
                    *text = form::rename(text, &field.name, new);
                    edit.fields.retain(|f| f.name != new);
                    if let Some(f) = edit.fields.iter_mut().find(|f| f.name == field.name) {
                        f.name = new.to_string();
                    }
                    kept = Some(new.to_string());
                }
            }
            // Enter is done: back to the text, where the caret was.
            if named.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                kept = None;
                ctx.memory_mut(|m| m.request_focus(replace_id()));
            }
            let current = kept.clone().unwrap_or_else(|| field.name.clone());

            ui.add_space(controls::GAP);
            if let Some(picked) = controls::segmented(ui, "editor-blank-kind", &kinds, Some(usize::from(is_choice))) {
                let f = edit.field_mut(&current);
                if picked == 1 && f.kind != FieldKind::Choice {
                    f.kind = FieldKind::Choice;
                    // Options start from the answer it already had, or from one empty row that
                    // takes the caret, so there is never a blank list to wonder about.
                    if f.options.iter().all(|o| o.trim().is_empty()) {
                        f.options = vec![std::mem::take(&mut f.default)];
                        let row = egui::Id::new(("editor-option", 0usize));
                        ctx.memory_mut(|m| m.request_focus(row));
                    }
                } else if picked == 0 && f.kind == FieldKind::Choice {
                    f.kind = FieldKind::Short;
                }
            }

            ui.add_space(controls::GAP);
            if is_choice {
                options(ui, state, edit, &current, &field.options, is_light);
            } else {
                let mut value = field.default.clone();
                let hint = studio::text(
                    state,
                    "Respuesta inicial (opcional)",
                    "Starting answer (optional)",
                    "Panimulang sagot (opsyonal)",
                    "शुरुआती जवाब (वैकल्पिक)",
                );
                ui.label(controls::small_muted(hint, is_light));
                ui.add_space(controls::GAP_TIGHT);
                let input = controls::multiline_field(&mut value).desired_rows(1).desired_width(f32::INFINITY);
                if ui.add(input).changed() {
                    edit.field_mut(&current).default = value;
                }
            }
        })?;

    // The bubble answers Esc and a click anywhere outside it — but not the click that opened it,
    // which a click on another pill also is.
    let outside = !fresh_click && shown.response.clicked_elsewhere();
    if shown.response.should_close() || outside {
        kept = None;
    }
    kept
}

/// A list's options, one box each with a way to take it out, and a way to add one more. Enter in a
/// box adds the next one below it, so a list can be typed straight through.
fn options(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, name: &str, current: &[String], is_light: bool) {
    let ctx = ui.ctx().clone();
    let row_id = |i: usize| egui::Id::new(("editor-option", i));
    let hint = studio::text(
        state,
        "La primera sale elegida",
        "The first one starts selected",
        "Nakapili na ang una",
        "पहला पहले से चुना रहता है",
    );
    ui.label(controls::small_muted(hint, is_light));
    ui.add_space(controls::GAP_TIGHT);
    let remove_tip = studio::text(state, "Quitar opción", "Remove option", "Alisin ang pagpipilian", "विकल्प हटाएँ");
    let mut list = current.to_vec();
    if list.is_empty() {
        list.push(String::new());
    }
    let mut changed = false;
    let mut remove = None;
    let mut insert_after = None;
    for i in 0..list.len() {
        if i > 0 {
            ui.add_space(controls::GAP_TIGHT);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP_TIGHT;
            let room = ui.available_width() - controls::FIELD_HEIGHT - controls::GAP_TIGHT;
            let row = ui.add(controls::text_field(&mut list[i]).id(row_id(i)).desired_width(room));
            changed |= row.changed();
            if row.lost_focus() && ui.input(|inp| inp.key_pressed(egui::Key::Enter)) && !list[i].trim().is_empty() {
                insert_after = Some(i);
            }
            // The last box cannot go: a list always has a row to type into.
            if controls::icon_button(ui, Glyph::Close, remove_tip, list.len() > 1).clicked() {
                remove = Some(i);
            }
        });
    }
    ui.add_space(controls::GAP_TIGHT);
    let add = studio::text(state, "Añadir opción", "Add option", "Magdagdag ng pagpipilian", "विकल्प जोड़ें");
    if controls::subtle_button(ui, Some(Glyph::Add), add, None).clicked() {
        insert_after = Some(list.len() - 1);
    }
    if let Some(i) = remove {
        list.remove(i);
        changed = true;
    }
    if let Some(i) = insert_after {
        list.insert(i + 1, String::new());
        ctx.memory_mut(|m| m.request_focus(row_id(i + 1)));
        changed = true;
    }
    if changed {
        edit.field_mut(name).options = list;
    }
    if current.iter().all(|o| o.trim().is_empty()) {
        ui.add_space(controls::GAP_TIGHT);
        let missing = studio::text(
            state,
            "Escribe al menos una opción para poder guardar.",
            "Write at least one option to be able to save.",
            "Sumulat ng kahit isang pagpipilian para makapag-save.",
            "सहेजने के लिए कम से कम एक विकल्प लिखें।",
        );
        ui.label(egui::RichText::new(missing).color(crate::app::caution(is_light)));
    }
}

/// The bubble under the date's pill: how the date is written. Every date pill in the text is the
/// same date, so it holds for all of them. Returns whether it is still open.
fn date_bubble(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, anchor: egui::Rect, fresh_click: bool) -> bool {
    let ctx = ui.ctx().clone();
    let date = &mut edit.date;
    let Some(shown) = egui::Popup::new(egui::Id::new("editor-bubble-popup"), ctx.clone(), anchor, ui.layer_id())
        .open(true)
        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
        .align(egui::RectAlign::BOTTOM_START)
        .align_alternatives(&[egui::RectAlign::TOP_START])
        .gap(controls::GAP_TIGHT)
        .frame(egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(controls::GAP_WIDE as i8)))
        .show(|ui| {
            ui.set_min_width(BUBBLE_MIN_WIDTH);
            date_options(ui, state.t(), &mut date.preset, &mut date.custom_format, &mut date.custom_tz, &mut date.month_lang);
        })
    else {
        return false;
    };
    // Esc, or a click anywhere outside it — but not the click that opened it.
    let outside = !fresh_click && shown.response.clicked_elsewhere();
    !(shown.response.should_close() || outside)
}

/// The preview's answers for `spec`, each kept under its blank's name, and following the value the
/// blank starts with until somebody types over it.
fn preview_answers(ctx: &egui::Context, spec: &FormSpec) -> Vec<String> {
    let kept: Vec<(String, String, String)> = ctx.data(|d| d.get_temp(preview_answers_key())).unwrap_or_default();
    spec.fields
        .iter()
        .map(|field| {
            let initial = field.initial();
            match kept.iter().find(|(name, ..)| *name == field.name) {
                Some((_, was, answer)) if answer != was => answer.clone(),
                _ => initial,
            }
        })
        .collect()
}

fn keep_preview_answers(ctx: &egui::Context, spec: &FormSpec, answers: &[String]) {
    let kept: Vec<(String, String, String)> = spec
        .fields
        .iter()
        .zip(answers)
        .map(|(field, answer)| (field.name.clone(), field.initial(), answer.clone()))
        .collect();
    ctx.data_mut(|d| d.insert_temp(preview_answers_key(), kept));
}

/// The four real formats, the way into building a fifth, and the way out for one the blocks cannot
/// express. Unchanged by the renovation except for where it sits: what a format *means* is not a
/// thing a new coat of paint may touch.
fn date_options(
    ui: &mut egui::Ui,
    t: &'static Strings,
    preset: &mut Option<DatePreset>,
    custom_format: &mut String,
    custom_tz: &mut String,
    month_lang: &mut Lang,
) {
    // Every option carries the date it would produce *right now*, right next to it. That is the
    // whole explanation most people need: you pick the one that looks like what you want, instead
    // of decoding "%B %-d, %Y" to find out.
    let mut labels: Vec<&str> = DatePreset::ALL.iter().map(|p| p.label(t)).collect();
    labels.push(t.preset_custom);
    let column = option_column(ui, &labels);

    for p in DatePreset::ALL {
        if date_option(
            ui,
            *preset == Some(p),
            p.label(t),
            Some(p.sample(*month_lang)),
            column,
        ) {
            *preset = Some(p);
            *custom_format = p.format().to_string();
            *custom_tz = p.tz().unwrap_or("").to_string();
        }
    }

    {
        let sample = preset.is_none().then(|| {
            let tz = (!custom_tz.trim().is_empty()).then(|| custom_tz.trim());
            datefmt::sample(custom_format, tz, *month_lang)
        });
        if date_option(ui, preset.is_none(), t.preset_custom, sample, column) && preset.is_some() {
            // Arriving at "Personalizado" starts from nothing.
            //
            // It used to inherit whatever preset was selected a moment ago, which meant the block
            // builder only appeared if that preset happened to be one the blocks can express —
            // coming from the ISO date it showed up, coming from any of the other three it silently
            // did not. Starting empty removes that entirely, and it is the better first impression
            // anyway: an empty row that says where to drop things explains itself, a row pre-filled
            // with somebody else's format does not.
            *preset = None;
            custom_format.clear();
            custom_tz.clear();
        }
    }

    if preset.is_none() {
        ui.indent("custom_date", |ui| {
            // The builder is the whole of it now. The strftime field, the time-zone box and the
            // "Ejemplo: %Y-%m-%d" line that used to sit underneath are gone: they were the old way
            // of doing this, kept as a safety net, and leaving them there just asked people to
            // learn the notation the blocks exist to replace. The time zone is no longer typed
            // either — it follows from whether an "Hora (UTC)" block is in the row.
            //
            // Only formats the blocks can carry *back out* go to the builder. The blocks hold one
            // time zone between them — UTC or the machine's own — and when they are rebuilt they
            // write the whole `tz` field from that. So a match carrying `tz: Europe/Madrid` handed
            // to the builder would come back with its time zone deleted the first time any block
            // was touched: the same silent loss the advanced field below exists to prevent, which
            // is where such a format goes.
            //
            // The comparison ignores case for the same reason the preview does: espanso accepts
            // `tz: utc`, and the two must not disagree about what it means.
            let tz = custom_tz.trim();
            let tz_is_utc = tz.eq_ignore_ascii_case("UTC");
            let expressible = tz.is_empty() || tz_is_utc;
            match expressible
                .then(|| crate::ui::date_blocks::parse(custom_format, tz_is_utc))
                .flatten()
            {
                Some(blocks) => {
                    if let Some(rebuilt) = crate::ui::date_blocks::show(ui, t, &blocks, month_lang) {
                        *custom_format = crate::ui::date_blocks::to_format(&rebuilt);
                        *custom_tz = crate::ui::date_blocks::timezone_for(&rebuilt)
                            .unwrap_or("")
                            .to_string();
                    }
                }
                None => {
                    // An imported or hand-edited format the blocks cannot express. Shown rather
                    // than silently rewritten, since rewriting it would lose part of what the
                    // author meant.
                    let is_light = !ui.visuals().dark_mode;
                    ui.label(controls::muted(t.blocks_advanced, is_light));
                    ui.add_space(controls::GAP);
                    ui.horizontal(|ui| {
                        ui.label(controls::field_label(t.format_label));
                        ui.add(controls::text_field(custom_format).desired_width(220.0));
                    });
                    // The third place this question has to be asked, and the one it was missing
                    // from. A format the blocks cannot express is the *only* kind that can carry
                    // `%A`, `%b` or `%h`, so this branch is where a written month or weekday is
                    // most likely to turn up — and saving pins a `locale:` for it either way.
                    // Without the picker an imported `%A %-d %B %Y` that named no language quietly
                    // became English, with nothing on screen to say so or to say otherwise.
                    if datefmt::needs_locale(custom_format) {
                        ui.add_space(controls::GAP_SECTION);
                        month_language(ui, t, month_lang);
                    }
                }
            }
        });
    }

    // Under the presets, the same question the block builder asks about its own row: a format that
    // writes the month in words has to say which words.
    if (*preset).is_some_and(|p| datefmt::needs_locale(p.format())) {
        ui.add_space(controls::GAP_SECTION);
        month_language(ui, t, month_lang);
    }
}

/// What the expansion will actually produce, on a card of its own beside the form.
///
/// Everything to the left is the expansion being made; this card only shows it, so it is drawn as
/// something else: a raised card, like the one beside the list, headed «Vista previa» with the
/// trigger said in a sentence, and holding nothing to count or tune. For a date it is the only place
/// the result can be seen at all — nobody reads `%-d de %B de %Y` and sees a date. The sample comes
/// from [`crate::datefmt`], which is the code that runs when the expansion is used, so what is on
/// screen is what espanso will write.
///
/// A text with questions can be tried out right in it: each answer is clicked, an option to the next
/// one and a typed answer to a box where another is typed. None of it is kept.
fn preview_pair(ui: &mut egui::Ui, state: &AppState, edit: &EditState, max_height: f32, is_light: bool) {
    let trigger = edit.trigger();
    let shown = edit.as_spec(&edit.replace);
    let spec = (!shown.fields.is_empty()).then(|| shown.clone());
    let mut answers = spec.as_ref().map(|spec| preview_answers(ui.ctx(), spec));
    let result = &edit.replace;
    let quiet = crate::app::secondary_text(is_light);

    controls::section_frame(is_light).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP;
            crate::ui::glyphs::show(ui, Glyph::View, crate::ui::glyphs::SIZE, quiet);
            ui.label(controls::h3(studio::text(state, "Vista previa", "Preview", "Preview", "पूर्वावलोकन")));
        });
        ui.add_space(controls::GAP_TIGHT);
        // The trigger in the sentence, in the accent the way a trigger is always written.
        let body = egui::TextStyle::Body.resolve(ui.style());
        let mut job = egui::text::LayoutJob::default();
        if trigger.is_empty() {
            let said = studio::text(
                state,
                "Así quedará tu texto.",
                "How your text will look.",
                "Ganito lalabas ang teksto mo.",
                "आपका टेक्स्ट ऐसा दिखेगा।",
            );
            job.append(said, 0.0, egui::TextFormat::simple(body.clone(), quiet));
        } else {
            let said = studio::text(
                state,
                "Así quedará al escribir {trigger}.",
                "How it looks when you type {trigger}.",
                "Ganito ito lalabas kapag tinype mo ang {trigger}.",
                "{trigger} लिखने पर ऐसा दिखेगा।",
            );
            let (before, after) = said.split_once("{trigger}").unwrap_or((said, ""));
            job.append(before, 0.0, egui::TextFormat::simple(body.clone(), quiet));
            job.append(&trigger, 0.0, controls::trigger_format(ui, &body, crate::app::accent(ui.visuals())));
            job.append(after, 0.0, egui::TextFormat::simple(body.clone(), quiet));
        }
        ui.add(egui::Label::new(job).wrap().selectable(false));
        ui.add_space(controls::GAP_ROW);

        controls::inset_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Rounded down to whole lines, so the box never shows the top half of a line of text.
            let line = ui.text_style_height(&egui::TextStyle::Body);
            let cap = (max_height / line).floor().max(3.0) * line;
            egui::ScrollArea::vertical()
                .id_salt("editor-preview")
                .max_height(cap)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if result.is_empty() {
                        ui.label(controls::muted(
                            studio::text(
                                state,
                                "Tu texto aparecerá aquí…",
                                "Your text will appear here…",
                                "Lalabas ang teksto mo rito…",
                                "आपका टेक्स्ट यहाँ दिखेगा…",
                            ),
                            is_light,
                        ));
                    } else {
                        // The same job the form window draws, so the date reads as today's and what
                        // was last copied sits in the accent, just as it will once typed.
                        let lang = state.settings.lang;
                        let mut none = Vec::new();
                        let answers = answers.as_mut().unwrap_or(&mut none);
                        try_answers(ui, &shown, answers, lang, is_light);
                    }
                });
        });

        // Said out loud because the preview shows *today*, and a date expansion that wrote today's
        // date for ever would be a bug rather than a feature.
        if edit.replace.trim() == format!("{{{{{}}}}}", edit.date.name) {
            ui.add_space(controls::GAP);
            ui.label(controls::small_muted(
                studio::text(
                    state,
                    "La fecha se calcula cada vez que escribes el activador.",
                    "The date is worked out each time you type the trigger.",
                    "Kinakalkula ang petsa sa tuwing ita-type mo ang trigger.",
                    "हर बार ट्रिगर लिखने पर दिनांक की गणना होती है।",
                ),
                is_light,
            ));
        }

        if let (Some(spec), Some(answers)) = (&spec, &answers) {
            ui.add_space(controls::GAP);
            ui.label(controls::small_muted(
                studio::text(
                    state,
                    "Haz clic en una respuesta para probar otra",
                    "Click an answer to try another",
                    "I-click ang isang sagot para sumubok ng iba",
                    "दूसरा आज़माने के लिए किसी जवाब पर क्लिक करें",
                ),
                is_light,
            ));
            keep_preview_answers(ui.ctx(), spec, answers);
        }
    });
}

fn preview_typing_key() -> egui::Id {
    egui::Id::new("editor-preview-typing")
}

/// The preview's text with each answer in the accent and ready to be clicked: an option gives way
/// to the next one, a typed answer opens a box beside it to type another. The answer under the
/// pointer, or being typed, wears the same pill as the markers in the text box.
fn try_answers(ui: &mut egui::Ui, spec: &FormSpec, answers: &mut [String], lang: Lang, is_light: bool) {
    let ctx = ui.ctx().clone();
    let (job, spans) = crate::ui::form_view::result_job_with_answers(ui, spec, answers, None, lang, ui.available_width());
    let galley = ui.painter().layout_job(job);
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::click());
    let typing = ctx
        .data(|d| d.get_temp::<(String, u64)>(preview_typing_key()))
        .and_then(|(name, opened)| spec.fields.iter().position(|f| f.name == name).map(|i| (i, opened)));
    let hover = response.hover_pos();
    let fill = controls::pill_fill(ui.visuals(), is_light);
    // An answer has no brackets to lend its pill an edge, so the pill reaches a little past it.
    let edge = egui::vec2(3.0, 0.0);
    let mut over = None;
    let mut typing_at = None;
    for (chars, index) in &spans {
        let rects: Vec<egui::Rect> = controls::pill_rects(&galley, rect.min, chars).iter().map(|r| r.expand2(edge)).collect();
        let hovered = hover.is_some_and(|p| rects.iter().any(|r| r.contains(p)));
        let is_typed = typing.is_some_and(|(i, _)| i == *index);
        if hovered || is_typed {
            for r in &rects {
                let r = r.shrink2(egui::vec2(0.0, 1.0));
                ui.painter().rect_filled(r, r.height() * 0.5, fill);
            }
        }
        if hovered && over.is_none() {
            over = Some(*index);
        }
        if is_typed && typing_at.is_none() {
            typing_at = rects.first().copied();
        }
    }
    ui.painter().galley(rect.min, galley, ui.visuals().text_color());
    if over.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let frame = ctx.cumulative_frame_nr();
    let mut open = typing;
    if let Some(index) = over.filter(|_| response.clicked()) {
        let field = &spec.fields[index];
        if field.kind == FieldKind::Choice && !field.options.is_empty() {
            let now = field.options.iter().position(|o| *o == answers[index]);
            let next = now.map_or(0, |i| (i + 1) % field.options.len());
            answers[index] = field.options[next].clone();
            open = None;
        } else if typing.is_some_and(|(i, _)| i == index) {
            open = None;
        } else {
            open = Some((index, frame));
            typing_at = None;
        }
        ctx.request_repaint();
    }

    // The box for a typed answer, under it, for as long as nothing is clicked away from it.
    if let (Some((index, opened)), Some(anchor)) = (open, typing_at) {
        let id = egui::Id::new("editor-preview-answer");
        let shown = egui::Popup::new(id.with("popup"), ctx.clone(), anchor, ui.layer_id())
            .open(true)
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
            .align(egui::RectAlign::BOTTOM_START)
            .align_alternatives(&[egui::RectAlign::TOP_START])
            .gap(controls::GAP_TIGHT)
            .frame(egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(controls::GAP_WIDE as i8)))
            .show(|ui| {
                ui.set_width(BUBBLE_MIN_WIDTH);
                let field = &spec.fields[index];
                let answer = &mut answers[index];
                let box_ = if field.kind == FieldKind::Long {
                    controls::multiline_field(answer).desired_rows(2)
                } else {
                    controls::text_field(answer)
                };
                let typed = ui.add(box_.id(id).desired_width(f32::INFINITY));
                if opened == frame {
                    ctx.memory_mut(|m| m.request_focus(id));
                }
                if typed.changed() {
                    ctx.request_repaint();
                }
                let done = field.kind != FieldKind::Long && typed.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                done
            });
        let away = frame > opened
            && ctx.input(|i| i.pointer.any_click())
            && shown.as_ref().is_some_and(|s| !s.response.contains_pointer());
        let done = shown.as_ref().is_some_and(|s| s.inner);
        let name = spec.fields[index].name.clone();
        ctx.data_mut(|d| {
            if away || done {
                d.remove::<(String, u64)>(preview_typing_key());
            } else {
                d.insert_temp(preview_typing_key(), (name, opened));
            }
        });
    } else if open.is_none() {
        ctx.data_mut(|d| d.remove::<(String, u64)>(preview_typing_key()));
    } else if let Some((index, opened)) = open {
        // Opened this frame: its anchor is found on the next one.
        let name = spec.fields[index].name.clone();
        ctx.data_mut(|d| d.insert_temp(preview_typing_key(), (name, opened)));
        ctx.request_repaint();
    }
}

/// The widest a folder's or a name's words get at the foot before they are cut with «…».
const EXTRA_MAX_WIDTH: f32 = 180.0;
/// How wide the folder and name flyouts open.
const EXTRA_MENU_WIDTH: f32 = 260.0;
/// How tall the folders in the folder flyout grow before they scroll.
const EXTRA_FOLDERS_MAX_HEIGHT: f32 = 240.0;

/// The box for a new folder's name, so «Nueva carpeta…» can put the cursor in it.
fn new_folder_field_id() -> egui::Id {
    egui::Id::new("edit-new-folder-name")
}

/// The star, the folder and the name: the three things an expansion may have that change nothing
/// about what it does. They wait at the foot, opposite Guardar, as quiet buttons, so making an
/// expansion asks for two answers and not five; what each one is for is said on hover. Each button
/// shows what it holds, so an expansion that has a folder or a name says so without being opened up.
/// `each` is the most the folder and the name may take, so the row never runs into Guardar.
fn extras(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, each: f32) {
    let star_tip = studio::text(
        state,
        "Favorita: sale primero en la lista y en el buscador",
        "Favorite: comes first in the list and in search",
        "Paborito: nauuna sa listahan at sa paghahanap",
        "पसंदीदा: सूची और खोज में सबसे पहले आता है",
    );
    if controls::star_button(ui, edit.favorite, star_tip).clicked() {
        edit.favorite = !edit.favorite;
    }

    let folder_tip = studio::text(
        state,
        "Carpeta: para encontrarla aquí. La expansión funciona igual en cualquier carpeta",
        "Folder: for finding it here. The expansion works the same in any folder",
        "Folder: para mahanap ito rito. Pareho ang gana ng expansion sa kahit anong folder",
        "फ़ोल्डर: इसे यहाँ खोजने के लिए। विस्तार किसी भी फ़ोल्डर में एक जैसा काम करता है",
    );
    let folder_word = studio::text(state, "Carpeta", "Folder", "Folder", "फ़ोल्डर");
    let shown = edit.folder.clone().unwrap_or_else(|| folder_word.to_owned());
    let button = controls::tool_button(ui, Glyph::Folder, &shown, edit.folder.is_some(), each, folder_tip);
    folder_menu(ui, state, edit, &button);

    let name_tip = studio::text(
        state,
        "Nombre: para reconocerla en la lista y encontrarla en el buscador",
        "Name: to recognise it in the list and find it in search",
        "Pangalan: para makilala ito sa listahan at mahanap sa paghahanap",
        "नाम: ताकि सूची में इसे पहचान सकें और खोज में ढूँढ सकें",
    );
    let named = !edit.name.trim().is_empty();
    let shown = if named {
        edit.name.trim().to_owned()
    } else {
        studio::text(state, "Nombre", "Name", "Pangalan", "नाम").to_owned()
    };
    let button = controls::tool_button(ui, Glyph::Tag, &shown, named, each, name_tip);
    name_menu(state, edit, &button);
}

/// The folder flyout: no folder, every folder, and a new one named in place. A folder named here
/// exists only in this draft until Guardar, like any other change on the form.
fn folder_menu(ui: &egui::Ui, state: &AppState, edit: &mut EditState, button: &egui::Response) {
    let t = state.t();
    let ctx = ui.ctx();
    // Closed with a name half typed, the flyout opens next time on the list again.
    if edit.choosing_new_folder && !egui::Popup::is_id_open(ctx, egui::Popup::default_response_id(button)) && !button.clicked() {
        edit.choosing_new_folder = false;
        edit.new_folder_input.clear();
    }
    let mut folder_names = state.settings.all_folder_names();
    // A folder named in this draft is not in the settings yet, but it is the one chosen, so the
    // list has to be able to show it as chosen.
    if let Some(pending) = edit.folder.as_ref().filter(|f| !folder_names.contains(f)) {
        folder_names.push(pending.clone());
        folder_names.sort_by_key(|f| f.to_lowercase());
    }
    let new_folder = format!("{}…", t.add_new_folder.trim_start_matches(['+', ' ']).trim_end_matches('…'));
    let by_keyboard = button.clicked() && button.has_focus();
    egui::Popup::menu(button)
        // Choosing a folder closes it (see [`controls::menu_check_item`]); typing a new one must not.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .align(egui::RectAlign::TOP_START)
        .gap(controls::GAP_TIGHT)
        .width(EXTRA_MENU_WIDTH)
        .show(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let first = ui.next_auto_id();
            if controls::menu_check_item(ui, t.no_folder, None, edit.folder.is_none()).clicked() {
                edit.folder = None;
            }
            if by_keyboard {
                ui.memory_mut(|m| m.request_focus(first));
            }
            egui::ScrollArea::vertical()
                .id_salt("editor-folder-menu")
                .max_height(EXTRA_FOLDERS_MAX_HEIGHT)
                .show(ui, |ui| {
                    for name in &folder_names {
                        let chosen = edit.folder.as_deref() == Some(name.as_str());
                        if controls::menu_check_item(ui, &crate::app::truncate(name, 32), None, chosen).clicked() {
                            edit.folder = Some(name.clone());
                        }
                    }
                });
            controls::menu_separator(ui);
            if edit.choosing_new_folder {
                // The line becomes the box for the name, where it was.
                let field = menu_field(
                    ui,
                    controls::text_field(&mut edit.new_folder_input)
                        .id(new_folder_field_id())
                        .hint_text(t.new_folder_placeholder),
                );
                let name = edit.new_folder_input.trim().to_string();
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty() {
                    // An existing name typed again is that folder, chosen.
                    let existing = folder_names.iter().find(|f| f.to_lowercase() == name.to_lowercase());
                    edit.folder = Some(existing.cloned().unwrap_or(name));
                    edit.choosing_new_folder = false;
                    edit.new_folder_input.clear();
                    ui.close();
                }
            } else if controls::menu_item_staying(ui, Some(Glyph::Add), &new_folder).clicked() {
                edit.choosing_new_folder = true;
                edit.new_folder_input.clear();
                ui.data_mut(|d| d.insert_temp(focus_due_key(), true));
            }
        });
}

/// Set when a flyout's box should take the cursor, and cleared once it has it. Asked for on the
/// frame of the click, the focus is lost again to that same click, so the box keeps asking until
/// it holds it.
fn focus_due_key() -> egui::Id {
    egui::Id::new("editor-flyout-focus-due")
}

/// A box inside a flyout, drawn as boxes are everywhere else — a menu takes the borders off the
/// widgets in it — and as wide as the flyout.
fn menu_field(ui: &mut egui::Ui, field: egui::TextEdit<'_>) -> egui::Response {
    let response = ui
        .scope(|ui| {
            ui.style_mut().visuals.widgets = ui.ctx().global_style().visuals.widgets.clone();
            ui.add(field.desired_width(ui.available_width()))
        })
        .inner;
    if ui.data(|d| d.get_temp::<bool>(focus_due_key())).unwrap_or(false) {
        if response.has_focus() {
            ui.data_mut(|d| d.remove::<bool>(focus_due_key()));
        } else {
            ui.memory_mut(|m| m.request_focus(response.id));
        }
    }
    response
}

/// The name flyout: one box, with the cursor already in it. Enter puts it away.
fn name_menu(state: &AppState, edit: &mut EditState, button: &egui::Response) {
    if button.clicked() {
        button.ctx.data_mut(|d| d.insert_temp(focus_due_key(), true));
    }
    egui::Popup::menu(button)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .align(egui::RectAlign::TOP_START)
        .gap(controls::GAP_TIGHT)
        .width(EXTRA_MENU_WIDTH)
        .show(|ui| {
            let field = menu_field(
                ui,
                controls::text_field(&mut edit.name)
                    .hint_text(studio::text(
                        state,
                        "Por ejemplo: Firma del correo",
                        "For example: Email signature",
                        "Halimbawa: Pirma sa email",
                        "उदाहरण: ईमेल हस्ताक्षर",
                    )),
            );
            controls::say_as(&field, studio::text(state, "Nombre", "Name", "Pangalan", "नाम"));
            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                ui.close();
            }
        });
}

/// The form itself: two numbered questions — what you type and what appears. The title sits above
/// it, outside the scroll, and the star, the folder and the name wait at the foot.
///
/// It takes `edit` by value and puts it back at the end, which is what lets the parts hold
/// `&mut EditState` without the borrow checker having to reason about `state.view` at the same
/// time. The draft is one thing throughout the frame, and it is written back once.
fn show_form(
    ui: &mut egui::Ui,
    state: &mut AppState,
    mut edit: EditState,
    problem: Option<&Problem>,
    inline_preview: bool,
    is_light: bool,
) {
    let first = studio::text(state, "¿Qué vas a escribir?", "What will you type?", "Ano ang ita-type mo?", "आप क्या लिखेंगे?");
    step_heading(ui, 1, first);
    ui.add_space(controls::GAP_ROW);
    step_body(ui, 1.0, |ui| when_you_type(ui, state, &mut edit, problem, is_light));
    ui.add_space(PART_GAP);

    // Each later step is faint until the one before it has something in it — never locked: the
    // order is a suggestion, and someone who starts with the text is not stopped from doing so.
    let second = studio::text(
        state,
        "¿Qué quieres que aparezca?",
        "What do you want to appear?",
        "Ano ang gusto mong lumabas?",
        "आप क्या दिखाना चाहते हैं?",
    );
    let lit = step_opacity(ui, 2, !edit.word.trim().is_empty() || step_touched(ui, 2));
    let top = ui.cursor().top();
    ui.scope(|ui| {
        ui.set_opacity(lit);
        step_heading(ui, 2, second);
    });
    ui.add_space(controls::GAP_ROW);
    step_body(ui, lit, |ui| appears_here(ui, state, &mut edit, inline_preview, is_light));
    note_step_touch(ui, 2, egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top()));
    ui.add_space(controls::GAP);

    // The draft is its own state, and a change to it has to show up on the next frame rather than
    // whenever something else happens to ask for one.
    if matches!(&state.view, View::Edit(current) if current != &edit) {
        ui.ctx().request_repaint();
    }
    state.view = View::Edit(edit);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window opened at its default size shows the preview beside the form, not under it: that
    /// is what the default is for, and a smaller one would put a newcomer's first expansion in
    /// the cramped layout.
    #[test]
    fn the_default_window_has_room_for_the_preview_beside_the_form() {
        let body = crate::display::DEFAULT_SIZE[0] - 2.0 * f32::from(controls::PAGE_MARGIN);
        assert!(columns(body).2.is_some());
    }

    #[test]
    fn a_selection_that_plainly_answers_a_question_is_named_after_it() {
        let es = |p: &[&str]| guessed_question(p, Lang::Es);
        assert_eq!(es(&["lunes"]), Some("¿Qué día?"));
        assert_eq!(es(&["Friday."]), Some("¿Qué día?"));
        assert_eq!(es(&["lunes", "martes", "viernes"]), Some("¿Qué día?"));
        assert_eq!(es(&["Marzo"]), Some("¿Qué mes?"));
        assert_eq!(es(&["10:30"]), Some("¿A qué hora?"));
        assert_eq!(es(&["14/11/2026"]), Some("¿Qué fecha?"));
        assert_eq!(es(&["ana@correo.com"]), Some("¿Qué correo?"));
        assert_eq!(es(&["https://khipu.app"]), Some("¿Qué enlace?"));
        assert_eq!(es(&["+52-55-1234-5678"]), Some("¿Qué teléfono?"));
        assert_eq!(es(&["$250.00"]), Some("¿Cuánto?"));
        assert_eq!(es(&["3"]), Some("¿Cuántos?"));
        assert_eq!(es(&["Buenos días", "Buenas tardes"]), Some("¿Qué saludo?"));
        assert_eq!(guessed_question(&["lunes"], Lang::En), Some("Which day?"));
        // Not plainly anything: the selection keeps naming itself.
        assert_eq!(es(&["Juan"]), None);
        assert_eq!(es(&["la reunión del lunes"]), None);
        assert_eq!(es(&["lunes", "Juan"]), None);
        assert_eq!(es(&["12345"]), None);
    }
    use crate::yaml::model::{MatchEntry, SimpleMatch};

    fn stored(triggers: &[&str]) -> Vec<MatchEntry> {
        triggers
            .iter()
            .map(|t| {
                MatchEntry::Simple(SimpleMatch {
                    trigger: (*t).to_string(),
                    replace: "x".to_string(),
                    vars: Vec::new(),
                    label: None,
                })
            })
            .collect()
    }

    #[test]
    fn the_short_trigger_is_named_first_whichever_one_is_being_added() {
        let existing = stored(&[":test"]);
        assert_eq!(
            shadowing_pair(&existing, None, ":testing"),
            Some((":test".to_string(), ":testing".to_string()))
        );
        let existing = stored(&[":testing"]);
        assert_eq!(
            shadowing_pair(&existing, None, ":test"),
            Some((":test".to_string(), ":testing".to_string()))
        );
    }

    #[test]
    fn the_same_word_under_a_different_prefix_is_not_a_warning() {
        // espanso's matcher completes the longest live path first, so `::hola` wins over `:hola`
        // and both keep working. This is what the prefix box produces on purpose; warning about it
        // would be warning about the feature.
        assert_eq!(shadowing_pair(&stored(&[":hola"]), None, "::hola"), None);
        assert_eq!(shadowing_pair(&stored(&["::hola"]), None, ":hola"), None);
        assert_eq!(shadowing_pair(&stored(&[":hola"]), None, "x:hola"), None);
    }

    #[test]
    fn an_exact_duplicate_is_left_to_the_check_that_names_it() {
        assert_eq!(shadowing_pair(&stored(&[":test"]), None, ":test"), None);
    }

    #[test]
    fn the_row_being_edited_never_shadows_itself() {
        let existing = stored(&[":test", ":other"]);
        assert_eq!(shadowing_pair(&existing, Some(0), ":test"), None);
    }

    fn typed(prefix: &str, word: &str) -> String {
        EditState {
            prefix: prefix.to_string(),
            word: word.to_string(),
            ..EditState::new_for_create("")
        }
        .trigger()
    }

    /// `:--` → `—` was impossible: a word box holding only symbols was read as a second prefix,
    /// which left no word, which the save check reported as an empty trigger.
    #[test]
    fn a_word_made_only_of_symbols_is_a_word() {
        assert_eq!(typed(":", "--"), ":--");
        assert_eq!(typed(":", "_"), ":_");
        assert_eq!(typed(":", "—"), ":—");
        assert_eq!(typed("::", "--"), "::--");
        // Whole thing typed into the word box: no prefix to find, so it is kept entire.
        assert_eq!(split_trigger(":--"), (String::new(), ":--".to_string()));
    }

    /// The two reasons the split exists in the first place, neither of which may move.
    #[test]
    fn the_prefix_box_still_owns_the_prefix_and_an_empty_word_is_still_empty() {
        assert_eq!(typed(":", ":hola"), ":hola");
        assert_eq!(typed(":", "hola"), ":hola");
        assert_eq!(split_trigger("::hola"), ("::".to_string(), "hola".to_string()));
        assert_eq!(typed(":", ""), "");
        assert_eq!(typed(":", "   "), "");
    }

    /// Opening a symbol-only trigger for editing and pressing Guardar has to give it back
    /// unchanged; otherwise the expansion could be created and then never edited again.
    #[test]
    fn a_symbol_only_trigger_survives_a_round_trip_through_the_editor() {
        for trigger in [":--", ":_", ":—", "//", ";;"] {
            let stored = SimpleMatch {
                trigger: trigger.to_string(),
                replace: "—".to_string(),
                vars: Vec::new(),
                label: None,
            };
            let edit = EditState::new_for_edit(0, &stored, None);
            assert_eq!(edit.trigger(), trigger);
            assert_eq!(edit.build_match().trigger, trigger);
        }
    }

    /// A date pill is espanso's own date variable when nothing needs filling in, the form's `date`
    /// when something does, and either way reopens as the same text with the same format.
    #[test]
    fn a_date_pill_survives_the_editor_alone_and_in_a_form() {
        for text in ["Hoy es {{fecha}}.", "{{fecha}} — Hola [[cliente]]", "{{fecha}}"] {
            let mut edit = EditState::new_for_create(":");
            edit.word = "hoy".into();
            edit.date.preset = Some(DatePreset::DayMonthYear);
            edit.date.month_lang = Lang::Es;
            edit.replace = text.into();
            let m = edit.build_match();
            let is_form = text.contains("[[");
            assert_eq!(m.as_form().is_some(), is_form);
            if is_form {
                assert_eq!(m.as_form().unwrap().date.unwrap().param_str("format"), Some("%-d %B %Y"));
            } else {
                assert_eq!(m.replace, text);
                assert_eq!(m.as_date_var().map(|v| v.var_type.as_str()), Some("date"));
            }
            assert!(MatchEntry::Simple(m.clone()).is_safely_editable());
            let back = EditState::new_for_edit(0, &m, None);
            assert_eq!(back.replace, text);
            assert_eq!(back.date.preset, Some(DatePreset::DayMonthYear));
            assert_eq!(back.build_match(), m);
        }
    }

    /// What was last copied is espanso's clipboard variable on its own, Khipu's to fill in inside
    /// a form, and either way comes back from the file as the same text with the same marker.
    #[test]
    fn what_was_copied_survives_the_editor_alone_and_in_a_form() {
        for text in ["Te reenvío: {{copiado}}", "Hola [[cliente]], te reenvío: {{ copiado }}"] {
            let mut edit = EditState::new_for_create(":");
            edit.word = "fw".into();
            edit.copied = "copiado".into();
            edit.replace = text.into();
            let m = edit.build_match();
            let is_form = text.contains("[[");
            assert_eq!(m.as_form().is_some(), is_form);
            if !is_form {
                assert_eq!(m.vars[0].var_type, "clipboard");
                assert_eq!(m.as_copied(), Some("copiado"));
            }
            assert!(MatchEntry::Simple(m.clone()).is_safely_editable());
            let back = EditState::new_for_edit(0, &m, None);
            assert_eq!(back.copied, "copiado");
            assert_eq!(back.replace, text);
            assert_eq!(back.build_match(), m);
        }
        // Written by hand in espanso's way, with a parameter Khipu does not know: left alone.
        let mut m = SimpleMatch {
            trigger: ":c".into(),
            replace: "{{clipboard}}".into(),
            vars: vec![VarEntry { name: "clipboard".into(), var_type: "clipboard".into(), ..Default::default() }],
            label: None,
        };
        assert!(MatchEntry::Simple(m.clone()).is_safely_editable());
        m.vars[0].params.insert("x".into(), 1.into());
        assert!(!MatchEntry::Simple(m).is_safely_editable());
    }

    /// A field deleted from the text and a new one added in its place: the new one takes the
    /// freed name, and used to take the deleted one's list and options with it.
    #[test]
    fn a_new_field_does_not_inherit_a_deleted_one() {
        let mut edit = EditState::new_for_create(":");
        edit.field_mut("campo 1").kind = FieldKind::Choice;
        edit.field_mut("campo 1").options = vec!["a".into(), "b".into()];
        let text = edit.new_blank("campo 1".into());
        let spec = edit.form(&text).unwrap();
        assert_eq!(spec.fields, vec![FormField { name: "campo 1".into(), ..Default::default() }]);

        // Cut and pasted back without a new field in between, it is still what it was.
        edit.field_mut("campo 2").kind = FieldKind::Long;
        assert_eq!(edit.form("[[campo 2]]").unwrap().fields[0].kind, FieldKind::Long);
    }

    /// Editing used to write `label: None` whatever the file said, so saving any change to an
    /// expansion with a name threw the name away.
    #[test]
    fn a_name_survives_the_editor_and_an_empty_one_is_not_written() {
        let stored = SimpleMatch {
            trigger: ";firma".to_string(),
            replace: "Saludos".to_string(),
            vars: Vec::new(),
            label: Some("Firma del correo".to_string()),
        };
        let edit = EditState::new_for_edit(0, &stored, None);
        assert_eq!(edit.build_match(), stored);

        let mut cleared = edit.clone();
        cleared.name = "   ".to_string();
        assert_eq!(cleared.build_match().label, None);

        let mut padded = edit;
        padded.name = "  Firma  ".to_string();
        assert_eq!(padded.build_match().label.as_deref(), Some("Firma"));
    }

    fn edited(before: &str, after: &str, caret: usize) -> Option<(String, usize)> {
        whole_markers(before, after, Some(caret), &pills(before, "copiado", "fecha"))
    }

    #[test]
    fn a_marker_goes_whole_or_not_at_all() {
        // Backspace right after a pill takes the pill.
        assert_eq!(edited("Hola [[cliente]]", "Hola [[cliente]", 15), Some(("Hola ".to_string(), 5)));
        // Delete right before one does too.
        assert_eq!(edited("[[a]] y", "[a]] y", 0), Some((" y".to_string(), 0)));
        // A selection that ends inside a pill takes the whole pill with it.
        assert_eq!(edited("ab [[nombre]] cd", "ab bre]] cd", 3), Some(("ab  cd".to_string(), 3)));
        // Ordinary typing beside a pill is left alone, even of the bracket it starts with.
        assert_eq!(edited("[[x]]", "[[[x]]", 1), None);
        assert_eq!(edited("[[x]]", "[[x]]]", 6), None);
        assert_eq!(edited("Hola [[x]]", "Hola, [[x]]", 5), None);
        // So is what was last copied.
        assert_eq!(edited("{{copiado}}!", "{{copiado}!", 10), Some(("!".to_string(), 0)));
    }

    #[test]
    fn nothing_typed_lands_inside_a_pill() {
        assert_eq!(edited("[[ab]]", "[[aZb]]", 4), Some(("[[ab]]Z".to_string(), 7)));
        // A paste that brings a whole marker of its own is a second pill, not a broken one.
        assert_eq!(edited("Hola ", "Hola [[x]]", 10), None);
    }

    #[test]
    fn the_caret_steps_over_a_pill() {
        let text = "ab [[nombre]] cd";
        let p = pills(text, "", "");
        assert_eq!(p[0].chars, 3..13);
        // Moving right into it carries on to its end; moving left, to its start.
        assert_eq!(snapped(4, 4, 3, false, &p), (13, 13));
        assert_eq!(snapped(12, 12, 13, false, &p), (3, 3));
        // A click goes to the nearer edge.
        assert_eq!(snapped(5, 5, 0, true, &p), (3, 3));
        assert_eq!(snapped(11, 11, 0, true, &p), (13, 13));
        // A selection grows to take the whole pill, whichever way it was dragged.
        assert_eq!(snapped(6, 1, 1, true, &p), (13, 1));
        assert_eq!(snapped(6, 15, 15, true, &p), (3, 15));
        // Shift+arrow back out of a pill it had taken lets it go.
        assert_eq!(snapped(12, 1, 13, false, &p), (3, 1));
        // Outside every pill, nothing moves.
        assert_eq!(snapped(1, 15, 1, true, &p), (1, 15));
    }
    /// A pill dragged somewhere else: taken out, put in where it was dropped, the caret after it.
    #[test]
    fn a_pill_moves_where_it_is_dropped() {
        // Forward: the drop point counts in the text as it was.
        assert_eq!(moved("[[a]] uno dos", 0..5, 9), (" uno[[a]] dos".to_string(), 9));
        // Backward.
        assert_eq!(moved("uno [[a]]", 4..9, 0), ("[[a]]uno ".to_string(), 5));
    }
}
