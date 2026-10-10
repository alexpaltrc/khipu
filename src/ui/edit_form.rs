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
//! ## Four parts, no cards and no numbers
//!
//! The form reads **Cuando escribes**, **Aparece este texto**, then the name and the folder, both
//! optional, in that order, each under a plain heading on the window itself. They were once
//! numbered cards; the order already says which comes first, and a card around every part is a
//! frame around nothing.
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
/// Width of the prefix box. Wide enough for the longest suggestion ("//") with the field padding
/// around it, and no wider — the prefix is one or two symbols, not a sentence.
/// Between the three parts of the form. With no card around each, the space is what tells them
/// apart, so it is wider than the gap between two things inside one part.
const PART_GAP: f32 = 28.0;
const PREFIX_FIELD_WIDTH: f32 = 72.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Text {
        replace: String,
    },
    Date {
        preset: Option<DatePreset>,
        custom_format: String,
        custom_tz: String,
        /// Which language a written-out month or weekday is spelled in. English by default, to
        /// match the interface's own default, and stored in the expansion itself so it reads the
        /// same on every machine it's shared with.
        month_lang: Lang,
    },
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
    pub kind: Kind,
    /// What the user calls it, if anything. Written as espanso's `label`, which espanso itself
    /// shows in its own search bar; empty means no `label` at all.
    pub name: String,
    /// The half of the Texto/Fecha switch that is not on screen, kept so that crossing the switch
    /// cannot destroy work. Until Guardar is pressed, the replacement text — or a date format built
    /// block by block — exists nowhere else, and one mis-click used to be enough to lose it with no
    /// way back. Scratch state only: `build_match` never reads it, so the YAML written is the same
    /// as it always was.
    pub stashed_kind: Option<Kind>,
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
}

impl EditState {
    pub fn new_for_create(default_prefix: &str) -> Self {
        Self {
            editing_index: None,
            prefix: default_prefix.to_string(),
            word: String::new(),
            kind: Kind::Text {
                replace: String::new(),
            },
            name: String::new(),
            stashed_kind: None,
            folder: None,
            choosing_new_folder: false,
            new_folder_input: String::new(),
            favorite: false,
            fields: Vec::new(),
        }
    }

    pub fn new_for_edit(index: usize, m: &SimpleMatch, folder: Option<String>) -> Self {
        let form = m.as_form();
        let kind = if let Some(form) = &form {
            // A form is edited as the text it fills in, markers and all.
            Kind::Text {
                replace: form.template.clone(),
            }
        } else if let Some(var) = m.as_date_var() {
            Kind::Date {
                preset: DatePreset::detect(var),
                custom_format: var.param_str("format").unwrap_or("").to_string(),
                custom_tz: var.param_str("tz").unwrap_or("").to_string(),
                month_lang: month_lang_of(var),
            }
        } else {
            Kind::Text {
                replace: m.replace.clone(),
            }
        };
        let (prefix, word) = split_trigger(&m.trigger);
        Self {
            editing_index: Some(index),
            prefix,
            word,
            kind,
            name: m.label.clone().unwrap_or_default(),
            stashed_kind: None,
            folder,
            choosing_new_folder: false,
            new_folder_input: String::new(),
            favorite: false,
            fields: form.map(|f| f.fields).unwrap_or_default(),
        }
    }

    /// The form the text makes, if it has any `[[name]]` in it: one field per name, in the order the
    /// names first appear, each as set here or a short text if nothing was.
    pub fn form(&self, text: &str) -> Option<FormSpec> {
        let names = form::names(text);
        (!names.is_empty()).then(|| FormSpec {
            template: text.to_string(),
            fields: names
                .into_iter()
                .map(|name| {
                    self.fields
                        .iter()
                        .find(|f| f.name == name)
                        .cloned()
                        .unwrap_or(FormField { name, ..Default::default() })
                })
                .collect(),
        })
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
        match &self.kind {
            Kind::Text { replace } => match self.form(replace) {
                // Any blank makes it a form: one variable that opens Khipu's window. See
                // [`crate::yaml::form`].
                Some(spec) => {
                    let trigger = self.trigger();
                    SimpleMatch {
                        vars: vec![spec.to_var(&trigger)],
                        replace: format!("{{{{{}}}}}", form::VAR_NAME),
                        trigger,
                        label: self.label(),
                    }
                }
                None => SimpleMatch {
                    trigger: self.trigger(),
                    replace: replace.clone(),
                    vars: Vec::new(),
                    label: self.label(),
                },
            },
            Kind::Date {
                preset,
                custom_format,
                custom_tz,
                month_lang,
            } => {
                let var: VarEntry = match preset {
                    Some(p) => p.build_var(DATE_VAR_NAME, *month_lang),
                    None => build_custom_date_var(
                        DATE_VAR_NAME,
                        custom_format,
                        Some(custom_tz.as_str()),
                        *month_lang,
                    ),
                };
                SimpleMatch {
                    trigger: self.trigger(),
                    replace: format!("{{{{{DATE_VAR_NAME}}}}}"),
                    vars: vec![var],
                    label: self.label(),
                }
            }
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
            ui.label(sample_text(sample));
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
    let Kind::Text { replace } = &edit.kind else {
        return None;
    };
    edit.form(replace)?
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
    // «Probar» answers its own Esc, like the discard question.
    let trying = ctx.data(|d| d.get_temp::<bool>(trying_key())).unwrap_or(false);

    let frame = ctx.cumulative_frame_nr();
    let arriving = ctx
        .data(|d| d.get_temp::<u64>(seen_key()))
        .is_none_or(|last| last + 1 < frame);
    ctx.data_mut(|d| d.insert_temp(seen_key(), frame));
    if arriving {
        ctx.data_mut(|d| {
            d.insert_temp(original_key(), edit.clone());
            d.remove::<bool>(confirm_discard_key());
            d.remove::<bool>(trying_key());
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
            // The star, at the right edge of the form's own column, level with the title.
            let full = ui.max_rect();
            let margin = ((full.width() - full.width().min(FORM_MAX_WIDTH)) * 0.5).max(0.0);
            let star = egui::Rect::from_min_size(
                egui::pos2(full.right() - margin - controls::FIELD_HEIGHT, full.top()),
                egui::Vec2::splat(controls::FIELD_HEIGHT),
            );
            let tip = if edit.favorite {
                studio::text(state, "Quitar de favoritos", "Remove from favorites", "Alisin sa mga paborito", "पसंदीदा से हटाएँ")
            } else {
                studio::text(state, "Añadir a favoritos", "Add to favorites", "Idagdag sa mga paborito", "पसंदीदा में जोड़ें")
            };
            let on = edit.favorite;
            if ui
                .scope_builder(egui::UiBuilder::new().max_rect(star), |ui| {
                    controls::star_button(ui, on, tip)
                })
                .inner
                .clicked()
            {
                edit.favorite = !on;
            }
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
            // Rule and buttons both keep to the form's own width, so the strip reads as the foot
            // of the form rather than of the window.
            let full = ui.available_width();
            let margin = ((full - full.min(FORM_MAX_WIDTH)) * 0.5).max(0.0);
            // The rule sits on the panel's own top edge, which is the inner margin above where the
            // content begins — it separates the strip from the form, so it cannot be inside it.
            let edge = ui.max_rect();
            ui.painter().hline(
                (edge.left() + margin)..=(edge.right() - margin),
                edge.top() - controls::GAP_SECTION,
                egui::Stroke::new(1.0, crate::app::hairline(is_light)),
            );
            ui.horizontal(|ui| {
                ui.set_max_width(full - margin);
                ui.add_space(margin);
                ui.spacing_mut().item_spacing.x = controls::GAP_ROW;
                let saving = controls::button(ui, t.save, Tone::Primary, can_save);
                save = match &problem {
                    // Says why it cannot be pressed at the moment somebody tries to press it, which
                    // is a different moment from reading the form.
                    Some(problem) => saving.on_disabled_hover_text(problem_text(state, t, problem)),
                    None => saving.on_hover_text(keys::tip(t.save, keys::SAVE, lang)),
                }
                .clicked();
                cancel = controls::button(ui, t.cancel, Tone::Normal, true)
                    .on_hover_text(keys::tip(t.cancel, keys::BACK, lang))
                    .clicked();
                if edit.editing_index.is_some() {
                    // Across the strip from Guardar rather than beside it. It is the one action
                    // here that cannot be taken back from this screen.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        delete = controls::button(ui, t.delete, Tone::Danger, true)
                            .on_hover_text(t.delete_tip)
                            .clicked();
                    });
                }
            });
        });

    controls::page_scroll(ui, "editor-body", |ui| {
        let full = ui.available_width();
        let width = full.min(FORM_MAX_WIDTH);
        ui.horizontal(|ui| {
            // The margin is the whole point of the horizontal, so nothing else may add to it.
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(((full - width) * 0.5).max(0.0));
            ui.vertical(|ui| {
                ui.set_width(width);
                show_form(ui, state, edit, problem.as_ref(), is_light);
            });
        });
    });

    // Read on every frame so the key never falls through to another screen, and acted on only when
    // the button beside it could have been pressed: the shortcut is the button, not a way past it.
    let ctrl_s = keys::pressed(&ctx, keys::SAVE);
    save |= ctrl_s && can_save && !confirming && !trying;

    // Esc is Cancelar. Not while a list is open over the form — that Esc closes the list — nor
    // while a dialog is up, which answers its own Esc.
    let blocked = popup_was_open || confirming || trying || state.pending_confirm.is_some();
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

/// The name of one part of the form. Plain words: the parts come in the order they are filled in,
/// and a number beside each would only say so again.
fn section_heading(ui: &mut egui::Ui, title: &str) {
    ui.add(egui::Label::new(controls::h3(title)).wrap());
}

/// "Escribirás: `:firma`" — the two boxes read back as the one thing espanso will watch for.
///
/// Drawn in pieces rather than as one filled-in sentence, so the trigger itself carries the accent
/// and the monospace: it is a literal string to type, and the words around it are prose. The
/// template is split at its own placeholder, which leaves each of the four languages its own word
/// order — and falls back to filling the template whole if a translation ever loses the placeholder.
fn trigger_line(ui: &mut egui::Ui, t: &'static Strings, trigger: &str, is_light: bool) {
    let shown = if trigger.is_empty() { "…" } else { trigger };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = controls::GAP_TIGHT;
        match t.trigger_preview.split_once("{trigger}") {
            Some((before, after)) => {
                let (before, after) = (before.trim_end(), after.trim_start());
                if !before.is_empty() {
                    ui.label(controls::muted(before, is_light));
                }
                ui.label(controls::code(shown, ui.visuals()).strong());
                if !after.is_empty() {
                    ui.label(controls::muted(after, is_light));
                }
            }
            None => {
                ui.label(controls::muted(
                    fill(t.trigger_preview, &[("trigger", shown)]),
                    is_light,
                ));
            }
        }
    });
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
        section_heading(
            ui,
            studio::text(
                state,
                "Cuando escribes",
                "When you type",
                "Kapag tina-type mo",
                "जब आप लिखते हैं",
            ),
        );
        ui.add_space(controls::GAP_WIDE);

        // Prefix and word are separate boxes so it is obvious which part is which — and so that
        // "apply this prefix everywhere" has an unambiguous piece to replace.
        //
        // Sized by their own margin rather than by `add_sized`. Handing `add_sized` a taller box
        // only stretches the frame: the text stays pinned to the top with the extra height below
        // it, which is exactly why the words in these two boxes once looked like they had floated
        // upward.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP;
            ui.add(
                controls::text_field(&mut edit.prefix)
                    .desired_width(PREFIX_FIELD_WIDTH)
                    .hint_text(t.prefix_field_hint),
            );
            ui.add(
                controls::text_field(&mut edit.word)
                    .id(word_field_id())
                    .desired_width(ui.available_width().max(90.0))
                    .hint_text(t.word_field_hint),
            );
        });

        ui.add_space(controls::GAP_ROW);
        trigger_line(ui, t, &edit.trigger(), is_light);

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

/// What appears: text or date, whichever this is, and what it will actually produce.
fn appears_here(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, is_light: bool) {
    let t = state.t();
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        section_heading(
            ui,
            studio::text(
                state,
                "Aparece este texto",
                "This text appears",
                "Lalabas ang tekstong ito",
                "यह टेक्स्ट दिखता है",
            ),
        );
        ui.add_space(controls::GAP_WIDE);

        let was_date = matches!(edit.kind, Kind::Date { .. });
        let mut is_date_mode = was_date;
        // Two positions of one switch, so drawn as one.
        let kinds = [t.kind_text, t.kind_date];
        if let Some(picked) = controls::segmented(ui, "kind", &kinds, Some(is_date_mode as usize)) {
            is_date_mode = picked == 1;
        }

        // Crossing the switch puts the half being left behind into `stashed_kind` and brings back
        // whatever was waiting there. It used to throw the old half away and start the new one from
        // its defaults, which meant a mis-click on a two-position control emptied the replacement
        // box of an expansion being edited, and clicking straight back gave an empty box — the text
        // was gone, and pressing Guardar wrote that emptiness over the stored version.
        //
        // Coming to the switch for the first time still starts from the defaults, so nothing looks
        // any different until you cross back.
        if is_date_mode != was_date {
            let fresh = if is_date_mode {
                Kind::Date {
                    preset: Some(DatePreset::IsoDateOnly),
                    custom_format: DatePreset::IsoDateOnly.format().to_string(),
                    custom_tz: String::new(),
                    month_lang: Lang::En,
                }
            } else {
                Kind::Text {
                    replace: String::new(),
                }
            };
            let restored = match edit.stashed_kind.take() {
                Some(kind) if matches!(&kind, Kind::Date { .. }) == is_date_mode => kind,
                _ => fresh,
            };
            edit.stashed_kind = Some(std::mem::replace(&mut edit.kind, restored));
        }

        ui.add_space(controls::GAP_WIDE);
        match &mut edit.kind {
            Kind::Text { replace } => {
                // Out of the draft while it is drawn, so the field list below can reach the rest
                // of the draft at the same time.
                let mut text = std::mem::take(replace);
                text_with_blanks(ui, state, edit, &mut text, is_light);
                if let Kind::Text { replace } = &mut edit.kind {
                    *replace = text;
                }
            }
            Kind::Date {
                preset,
                custom_format,
                custom_tz,
                month_lang,
            } => date_options(ui, t, preset, custom_format, custom_tz, month_lang),
        }

        ui.add_space(controls::GAP_STACK);
        preview_pair(ui, state, edit, is_light);
    });
}

// --- Blanks: a text with fields to fill in ------------------------------------------------------

fn replace_id() -> egui::Id {
    egui::Id::new("editor-replace")
}

/// Set while «Probar» is on screen, with the answers typed into it.
fn trying_key() -> egui::Id {
    egui::Id::new("editor-try")
}

fn try_answers_key() -> egui::Id {
    egui::Id::new("editor-try-answers")
}

/// The text with every `[[name]]` in the accent, so the blanks stand out from what is fixed.
fn highlighted(ui: &egui::Ui, text: &str, ink: egui::Color32, wrap: f32) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let blank = crate::app::accent(ui.visuals());
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut last = 0;
    for (range, _) in form::markers(text) {
        job.append(&text[last..range.start], 0.0, egui::TextFormat::simple(font.clone(), ink));
        job.append(&text[range.clone()], 0.0, egui::TextFormat::simple(font.clone(), blank));
        last = range.end;
    }
    job.append(&text[last..], 0.0, egui::TextFormat::simple(font, ink));
    job
}

/// What a new blank is called before it is renamed: «campo 1», «campo 2»…
fn new_blank_name(state: &AppState, text: &str) -> String {
    let word = studio::text(state, "campo", "field", "field", "फ़ील्ड");
    let taken = form::names(text);
    (1..)
        .map(|n| format!("{word} {n}"))
        .find(|name| !taken.contains(name))
        .unwrap_or_default()
}

/// Puts a new `[[campo N]]` where the cursor was in the text, or at its end if it never had one,
/// and leaves the cursor after it.
fn insert_blank(ctx: &egui::Context, state: &AppState, text: &mut String) {
    let marker = format!("[[{}]]", new_blank_name(state, text));
    let mut field = egui::TextEdit::load_state(ctx, replace_id()).unwrap_or_default();
    let chars = text.chars().count();
    let at = field
        .cursor
        .char_range()
        .map_or(chars, |range| range.primary.index.0.max(range.secondary.index.0).min(chars));
    let byte = text.char_indices().nth(at).map_or(text.len(), |(b, _)| b);
    text.insert_str(byte, &marker);
    let after = egui::text::CCursor::new(at + marker.chars().count());
    field.cursor.set_char_range(Some(egui::text::CCursorRange::one(after)));
    field.store(ctx, replace_id());
    ctx.memory_mut(|m| m.request_focus(replace_id()));
}

/// The text box, the buttons that add a blank and try the form out, and one row per blank.
fn text_with_blanks(
    ui: &mut egui::Ui,
    state: &AppState,
    edit: &mut EditState,
    text: &mut String,
    is_light: bool,
) {
    // A floor, not a box: `add_sized` gives the field at least this much and the field grows past
    // it as the text does, with the page scrolling rather than the words being cut off.
    let height = ui.text_style_height(&egui::TextStyle::Body) * TEXT_AREA_ROWS + 16.0;
    let ink = ui.visuals().widgets.inactive.text_color();
    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap: f32| {
        ui.painter().layout_job(highlighted(ui, buffer.as_str(), ink, wrap))
    };
    ui.add_sized(
        egui::vec2(ui.available_width(), height),
        controls::multiline_field(text).id(replace_id()).layouter(&mut layouter),
    );

    ui.add_space(controls::GAP);
    let spec = edit.form(text);
    let mut open_try = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = controls::GAP;
        let label = studio::text(state, "Insertar campo", "Insert field", "Magdagdag ng field", "फ़ील्ड जोड़ें");
        let tip = studio::text(
            state,
            "Un hueco que rellenas al usar la expansión",
            "A blank you fill in each time you use it",
            "Isang patlang na pupunan mo tuwing gagamitin ito",
            "एक खाली जगह जिसे हर बार उपयोग पर भरते हैं",
        );
        if controls::button(ui, label, Tone::Normal, true).on_hover_text(tip).clicked() {
            insert_blank(ui.ctx(), state, text);
        }
        if spec.is_some() {
            let label = studio::text(state, "Probar", "Try it", "Subukan", "आज़माएँ");
            open_try = controls::button(ui, label, Tone::Normal, true).clicked();
        }
    });

    let Some(spec) = spec else {
        return;
    };
    if open_try {
        let answers = crate::ui::form_view::initial_answers(&spec);
        ui.ctx().data_mut(|d| {
            d.insert_temp(trying_key(), true);
            d.insert_temp(try_answers_key(), answers);
        });
    }

    ui.add_space(controls::GAP_STACK);
    field_label(
        ui,
        studio::text(state, "Campos por completar", "Fields to fill in", "Mga field na pupunan", "भरने वाले फ़ील्ड"),
    );
    ui.add_space(controls::GAP_TIGHT);
    for (i, field) in spec.fields.iter().enumerate() {
        if i > 0 {
            ui.add_space(controls::GAP);
        }
        blank_row(ui, state, edit, text, i, field, is_light);
    }

    try_dialog(ui.ctx(), state, edit, &spec);
}

/// One blank: its name, what kind it is, and what it starts with.
fn blank_row(
    ui: &mut egui::Ui,
    state: &AppState,
    edit: &mut EditState,
    text: &mut String,
    index: usize,
    field: &FormField,
    is_light: bool,
) {
    let kinds = [
        studio::text(state, "Texto corto", "Short text", "Maikling teksto", "छोटा टेक्स्ट"),
        studio::text(state, "Texto largo", "Long text", "Mahabang teksto", "लंबा टेक्स्ट"),
        studio::text(state, "Lista", "List", "Listahan", "सूची"),
    ];
    let selected = match field.kind {
        FieldKind::Short => 0,
        FieldKind::Long => 1,
        FieldKind::Choice => 2,
    };
    controls::inset_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());

        // The name is typed into a copy and written back only while it is a name the text can
        // hold: not empty, not another blank's, nothing that would end the marker early.
        let name_id = egui::Id::new(("editor-blank-name", index));
        let draft_id = name_id.with("draft");
        let ctx = ui.ctx().clone();
        let mut draft = if ctx.memory(|m| m.has_focus(name_id)) {
            ctx.data(|d| d.get_temp::<String>(draft_id)).unwrap_or_else(|| field.name.clone())
        } else {
            field.name.clone()
        };
        let kinds_width = controls::segmented_natural_width(ui, &kinds);
        let room = ui.available_width() - kinds_width - controls::GAP;
        let side_by_side = room >= 160.0;
        let mut picked = None;
        let mut name_row = |ui: &mut egui::Ui, width: f32| {
            ui.add(controls::text_field(&mut draft).id(name_id).desired_width(width))
        };
        let name_changed = if side_by_side {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = controls::GAP;
                let changed = name_row(ui, room).changed();
                picked = controls::segmented(ui, ("editor-blank-kind", index), &kinds, Some(selected));
                changed
            })
            .inner
        } else {
            let changed = name_row(ui, ui.available_width()).changed();
            ui.add_space(controls::GAP);
            picked = controls::segmented(ui, ("editor-blank-kind", index), &kinds, Some(selected));
            changed
        };
        if name_changed {
            ctx.data_mut(|d| d.insert_temp(draft_id, draft.clone()));
            let new = draft.trim();
            let valid = !new.is_empty()
                && new.chars().count() <= form::NAME_MAX
                && !new.contains(['[', ']', '\n', '\r'])
                && !form::names(text).iter().any(|n| n == new);
            if valid {
                *text = form::rename(text, &field.name, new);
                edit.fields.retain(|f| f.name != new);
                if let Some(f) = edit.fields.iter_mut().find(|f| f.name == field.name) {
                    f.name = new.to_string();
                }
            }
        }
        if let Some(picked) = picked {
            edit.field_mut(&field.name).kind = match picked {
                0 => FieldKind::Short,
                1 => FieldKind::Long,
                _ => FieldKind::Choice,
            };
        }

        ui.add_space(controls::GAP);
        match field.kind {
            FieldKind::Short | FieldKind::Long => {
                let mut value = field.default.clone();
                let hint = studio::text(
                    state,
                    "Valor inicial (opcional)",
                    "Starting value (optional)",
                    "Panimulang halaga (opsyonal)",
                    "शुरुआती मान (वैकल्पिक)",
                );
                let input = if field.kind == FieldKind::Short {
                    controls::text_field(&mut value)
                } else {
                    controls::multiline_field(&mut value).desired_rows(2)
                };
                if ui.add(input.desired_width(f32::INFINITY).hint_text(hint)).changed() {
                    edit.field_mut(&field.name).default = value;
                }
            }
            FieldKind::Choice => {
                let mut options = field.options.join("\n");
                let hint = studio::text(
                    state,
                    "Una opción por línea. La primera aparece elegida.",
                    "One option per line. The first one starts selected.",
                    "Isang pagpipilian bawat linya. Ang una ang nakapili sa simula.",
                    "हर पंक्ति में एक विकल्प। पहला पहले से चुना रहता है।",
                );
                let response = ui.add(
                    controls::multiline_field(&mut options)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text(hint),
                );
                if response.changed() {
                    edit.field_mut(&field.name).options = options.split('\n').map(str::to_string).collect();
                }
                if field.options.iter().all(|o| o.trim().is_empty()) {
                    ui.add_space(controls::GAP_TIGHT);
                    let missing = studio::text(
                        state,
                        "Añade al menos una opción para poder guardar.",
                        "Add at least one option to be able to save.",
                        "Magdagdag ng kahit isang pagpipilian para makapag-save.",
                        "सहेजने के लिए कम से कम एक विकल्प जोड़ें।",
                    );
                    ui.label(egui::RichText::new(missing).color(crate::app::caution(is_light)));
                }
            }
        }
    });
}

/// «Probar»: the window the form opens when used, in a dialog, without espanso. Nothing typed here
/// is kept, and nothing is inserted anywhere.
fn try_dialog(ctx: &egui::Context, state: &AppState, edit: &EditState, spec: &FormSpec) {
    if !ctx.data(|d| d.get_temp::<bool>(trying_key())).unwrap_or(false) {
        return;
    }
    let lang = state.settings.lang;
    let mut answers = ctx
        .data(|d| d.get_temp::<Vec<String>>(try_answers_key()))
        .filter(|a| a.len() == spec.fields.len())
        .unwrap_or_else(|| crate::ui::form_view::initial_answers(spec));
    let first = ctx.data(|d| d.get_temp::<bool>(try_answers_key().with("focused"))).is_none();
    let mut close = false;
    let modal = controls::dialog(ctx, "editor-try-dialog").show(ctx, |ui| {
        let is_light = !ui.visuals().dark_mode;
        ui.set_width(controls::dialog_width(ctx, 460.0));
        let title = edit.name.trim();
        let title = if title.is_empty() { edit.trigger() } else { title.to_string() };
        ui.label(controls::h3(title));
        ui.add_space(controls::GAP_TIGHT);
        ui.label(controls::muted(
            studio::text(
                state,
                "Así se ve al usar la expansión. Aquí no se escribe nada.",
                "This is how it looks when you use it. Nothing is typed from here.",
                "Ganito ang itsura kapag ginamit mo. Walang ita-type mula rito.",
                "उपयोग करने पर यह ऐसा दिखता है। यहाँ से कुछ नहीं लिखा जाता।",
            ),
            is_light,
        ));
        ui.add_space(controls::GAP_SECTION);
        crate::ui::form_view::fields(ui, "editor-try", spec, &mut answers, first);
        ui.add_space(controls::GAP_SECTION);
        ui.label(controls::small_muted(crate::ui::form_view::preview_label(lang), is_light));
        ui.add_space(controls::GAP_TIGHT);
        crate::ui::form_view::result_preview(ui, spec, &answers);
        ui.add_space(controls::GAP_SECTION);
        controls::dialog_buttons(ui, |ui| {
            let done = studio::text(state, "Listo", "Done", "Tapos na", "हो गया");
            close = controls::button(ui, done, Tone::Primary, true).clicked();
        });
    });
    // Asked outside `data_mut`: `should_close` reads the context itself, and egui's lock does not
    // let a closure that holds it ask again. Inside, the second frame of the dialog hung.
    let close = close || modal.should_close();
    ctx.data_mut(|d| {
        if close {
            d.remove::<bool>(trying_key());
            d.remove::<Vec<String>>(try_answers_key());
            d.remove::<bool>(try_answers_key().with("focused"));
        } else {
            d.insert_temp(try_answers_key(), answers);
            d.insert_temp(try_answers_key().with("focused"), true);
        }
    });
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

/// What the expansion will actually produce, under the trigger that produces it.
///
/// Not a second copy of the box above it: the pairing is the point, and it is the same "you type
/// this, this appears" the library's two columns are headed with. For a date it is the only place
/// the result can be seen at all — nobody reads `%-d de %B de %Y` and sees a date. The sample comes
/// from [`crate::datefmt`], which is the code that runs when the expansion is used, so what is on
/// screen is what espanso will write.
fn preview_pair(ui: &mut egui::Ui, state: &AppState, edit: &EditState, is_light: bool) {
    let trigger = edit.trigger();
    let result = match &edit.kind {
        Kind::Text { replace } => replace.clone(),
        Kind::Date {
            preset,
            custom_format,
            custom_tz,
            month_lang,
        } => match preset {
            Some(p) => p.sample(*month_lang),
            None => datefmt::sample(
                custom_format,
                (!custom_tz.trim().is_empty()).then(|| custom_tz.trim()),
                *month_lang,
            ),
        },
    };

    ui.label(controls::small_muted(
        studio::text(state, "Vista previa", "Preview", "Preview", "पूर्वावलोकन"),
        is_light,
    ));
    ui.add_space(controls::GAP_TIGHT);
    controls::inset_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(
            egui::Label::new(
                controls::code(
                    if trigger.is_empty() { "…" } else { &trigger },
                    ui.visuals(),
                )
                .strong(),
            )
            .wrap()
            .selectable(false),
        );
        ui.add_space(controls::GAP);
        let (rule, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rule, 0, crate::app::hairline(is_light));
        ui.add_space(controls::GAP);
        // Rounded down to whole lines, so the box never shows the top half of a line of text.
        let line = ui.text_style_height(&egui::TextStyle::Body);
        let cap = (PREVIEW_MAX_HEIGHT / line).floor().max(3.0) * line;
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
                } else if matches!(edit.kind, Kind::Text { .. }) {
                    // The blanks in the accent here too, so the preview reads like the box above.
                    let job = highlighted(ui, &result, ui.visuals().text_color(), ui.available_width());
                    ui.add(egui::Label::new(job).wrap().selectable(true));
                } else {
                    ui.add(egui::Label::new(&result).wrap().selectable(true));
                }
            });
    });

    ui.add_space(controls::GAP);
    let footnote = match &edit.kind {
        Kind::Text { .. } => {
            let n = result.chars().count();
            let template = studio::plural(
                n,
                studio::text(state, "{n} carácter", "{n} character", "{n} karakter", "{n} अक्षर"),
                studio::text(
                    state,
                    "{n} caracteres",
                    "{n} characters",
                    "{n} karakter",
                    "{n} अक्षर",
                ),
            );
            let characters = fill(template, &[("n", &n.to_string())]);
            match &edit.kind {
                Kind::Text { replace } => match edit.form(replace) {
                    Some(spec) => format!("{} · {characters}", studio::blanks_to_fill(state, spec.fields.len())),
                    None => characters,
                },
                Kind::Date { .. } => characters,
            }
        }
        // Said out loud because the preview shows *today*, and a date expansion that wrote today's
        // date for ever would be a bug rather than a feature.
        Kind::Date { .. } => studio::text(
            state,
            "La fecha se calcula cada vez que escribes el activador.",
            "The date is worked out each time you type the trigger.",
            "Kinakalkula ang petsa sa tuwing ita-type mo ang trigger.",
            "हर बार ट्रिगर लिखने पर दिनांक की गणना होती है।",
        )
        .to_string(),
    };
    ui.label(controls::small_muted(footnote, is_light));
}

/// Where this expansion is filed. Last, and quieter than the other two: it changes nothing about
/// what espanso does.
/// What it is called, if the trigger alone does not say. Laid out like the folder below it: both
/// are optional, both are for finding it again, and neither changes what gets written.
fn name_card(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, is_light: bool) {
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        ui.label(controls::h3(studio::text(
            state,
            "Nombre (opcional)",
            "Name (optional)",
            "Pangalan (opsyonal)",
            "नाम (वैकल्पिक)",
        )));
        ui.add_space(controls::GAP_TIGHT);
        ui.label(controls::muted(
            studio::text(
                state,
                "Para reconocerla de un vistazo en la lista y encontrarla en el buscador.",
                "To recognise it at a glance in the list and find it in the search window.",
                "Para makilala ito agad sa listahan at mahanap sa paghahanap.",
                "ताकि सूची में इसे एक नज़र में पहचान सकें और खोज में ढूँढ सकें।",
            ),
            is_light,
        ));
        ui.add_space(controls::GAP_ROW);
        ui.add(
            controls::text_field(&mut edit.name)
                .desired_width(ui.available_width())
                .hint_text(studio::text(
                    state,
                    "Por ejemplo: Firma del correo",
                    "For example: Email signature",
                    "Halimbawa: Pirma sa email",
                    "उदाहरण: ईमेल हस्ताक्षर",
                )),
        );
    });
}

/// The box for a new folder's name, so the «+» beside the folder list can put the cursor in it.
fn new_folder_field_id() -> egui::Id {
    egui::Id::new("edit-new-folder-name")
}

fn folder_card(ui: &mut egui::Ui, state: &AppState, edit: &mut EditState, is_light: bool) {
    let t = state.t();
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        // The string carries the colon it needs as a field label; as a heading it does not.
        ui.label(controls::h3(t.folder_optional.trim_end_matches(':')));
        ui.add_space(controls::GAP_TIGHT);
        ui.label(controls::muted(
            studio::text(
                state,
                "Solo sirve para encontrarla aquí: la expansión funciona igual en cualquier carpeta.",
                "Only for finding it here: the expansion works the same in any folder.",
                "Para lang mahanap ito rito: pareho ang gana ng expansion sa kahit anong folder.",
                "यह केवल इसे यहाँ खोजने के लिए है: विस्तार किसी भी फ़ोल्डर में एक जैसा काम करता है।",
            ),
            is_light,
        ));
        ui.add_space(controls::GAP_ROW);

        let mut folder_names = state.settings.all_folder_names();
        // A folder named with ✓ exists only in this draft until Guardar, but it is the one chosen,
        // so the list has to be able to show it as chosen.
        if let Some(pending) = edit.folder.as_ref().filter(|f| !folder_names.contains(f)) {
            folder_names.push(pending.clone());
            folder_names.sort_by_key(|f| f.to_lowercase());
        }
        // «Nueva carpeta» is the «+» before the list, and only that. Pressed, the list itself becomes
        // the box for the new name — nothing opens below it — and «+» gives way to ✓ and ✗, which
        // are never on screen at the same time as it.
        let new_tip = t.add_new_folder.trim_start_matches(['+', ' ']).trim_end_matches('…');
        let create_tip = studio::text(state, "Crear carpeta", "Create folder", "Gumawa ng folder", "फ़ोल्डर बनाएँ");
        let creating = edit.choosing_new_folder;
        let shift = ui.ctx().animate_bool_with_time(egui::Id::new("edit-new-folder-shift"), creating, 0.22);
        // Smoothstep, so the slot eases out of one shape and into the other instead of sliding at
        // one speed and stopping dead.
        let eased = shift * shift * (3.0 - 2.0 * shift);
        let mut add = false;
        let mut confirm = false;
        let mut cancel = false;
        let name = edit.new_folder_input.trim().to_string();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP_TIGHT;
            let one = controls::FIELD_HEIGHT;
            let two = 2.0 * one + controls::GAP_TIGHT;
            let (slot, _) =
                ui.allocate_exact_size(egui::vec2(egui::lerp(one..=two, eased), one), egui::Sense::hover());
            // «+» leaves in the first half of the move and ✓ ✗ arrive in the second, so the two never
            // share a frame.
            if eased < 0.5 {
                let fade = 1.0 - eased * 2.0;
                let at = egui::Rect::from_min_size(slot.min, egui::Vec2::splat(one));
                ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| {
                    ui.set_opacity(fade);
                    add = controls::icon_button(ui, Glyph::Add, new_tip, !creating).clicked();
                });
            } else {
                let fade = eased * 2.0 - 1.0;
                let from = slot.min + egui::vec2(-6.0 * (1.0 - fade), 0.0);
                let at = egui::Rect::from_min_size(from, egui::vec2(two, one));
                ui.scope_builder(egui::UiBuilder::new().max_rect(at).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                    ui.spacing_mut().item_spacing.x = controls::GAP_TIGHT;
                    ui.set_opacity(fade);
                    let ok = !name.is_empty() && creating;
                    confirm = controls::icon_button(ui, Glyph::Check, create_tip, ok).clicked();
                    cancel = controls::icon_button(ui, Glyph::Close, t.cancel, creating).clicked();
                });
            }
            let rest = ui.available_width();
            if creating {
                // The box takes the list's place at the list's height, so the row does not jump: a
                // field's own padding would make it four points taller than the list it replaces.
                let field = ui.add_sized(
                    [rest, one],
                    controls::text_field(&mut edit.new_folder_input)
                        .margin(egui::Margin::symmetric(12, 7))
                        .id(new_folder_field_id())
                        .hint_text(t.new_folder_placeholder),
                );
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty() {
                    confirm = true;
                }
            } else {
                let selected_text = edit.folder.clone().unwrap_or_else(|| t.no_folder.to_string());
                // Truncating rather than extending: a long folder name may not push the control out
                // past the edge of the card it is in.
                egui::ComboBox::from_id_salt("edit_folder_combo")
                    .selected_text(selected_text)
                    .truncate()
                    .width(rest)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(edit.folder.is_none(), t.no_folder).clicked() {
                            edit.folder = None;
                        }
                        for name in &folder_names {
                            if ui.selectable_label(edit.folder.as_deref() == Some(name.as_str()), name).clicked() {
                                edit.folder = Some(name.clone());
                            }
                        }
                    });
            }
        });
        // Esc while naming a folder backs out of the naming, not out of the whole form.
        if creating && !confirm && keys::pressed(ui.ctx(), keys::BACK) {
            cancel = true;
        }
        if add {
            edit.choosing_new_folder = true;
            edit.new_folder_input.clear();
            ui.memory_mut(|m| m.request_focus(new_folder_field_id()));
        } else if confirm {
            // An existing name typed again is that folder, chosen; a new one is made with Guardar.
            let existing = folder_names.iter().find(|f| f.to_lowercase() == name.to_lowercase());
            edit.folder = Some(existing.cloned().unwrap_or(name));
            edit.choosing_new_folder = false;
            edit.new_folder_input.clear();
        } else if cancel {
            edit.choosing_new_folder = false;
            edit.new_folder_input.clear();
        }
    });
}

/// The form itself: what you type, what appears, its name and its folder. The title sits above it, outside
/// the scroll.
///
/// It takes `edit` by value and puts it back at the end, which is what lets the parts hold
/// `&mut EditState` without the borrow checker having to reason about `state.view` at the same
/// time. The draft is one thing throughout the frame, and it is written back once.
fn show_form(
    ui: &mut egui::Ui,
    state: &mut AppState,
    mut edit: EditState,
    problem: Option<&Problem>,
    is_light: bool,
) {
    when_you_type(ui, state, &mut edit, problem, is_light);
    ui.add_space(PART_GAP);
    appears_here(ui, state, &mut edit, is_light);
    ui.add_space(PART_GAP);
    name_card(ui, state, &mut edit, is_light);
    ui.add_space(PART_GAP);
    folder_card(ui, state, &mut edit, is_light);
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
}
