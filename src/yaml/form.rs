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

//! Expansions with blanks to fill in: what one looks like in `base.yml`, and the small helper file
//! that lets espanso open Khipu's own window for it.
//!
//! Espanso has forms of its own, but its window is its own design, and Khipu's look stops at its
//! edge. So a Khipu form is not an espanso `form:`. It is one `script` variable that runs Khipu:
//!
//! ```yaml
//! - trigger: ":cliente"
//!   replace: "{{khipu}}"
//!   vars:
//!     - name: khipu
//!       type: script
//!       params:
//!         args: ["%CONFIG%\\khipu-form.cmd", "3a636c69656e7465"]
//!         trim: false
//!         template: "Hola [[cliente]], …"
//!         fields:
//!           cliente: { type: text }
//!           tono: { type: choice, values: [Formal, Cercano], default: Formal }
//! ```
//!
//! Espanso runs the helper, the helper runs `Khipu.exe --form <trigger in hex>`, Khipu finds the
//! expansion in `base.yml`, asks for the blanks in its own window and prints the result, and espanso
//! types what was printed. `template` and `fields` ride along as parameters espanso does not read;
//! the field types are written the way espanso writes its own (`text`, `multiline`, `choice`), so
//! the file stays legible to anyone who knows espanso.
//!
//! A form may also hold what was last copied, as `{{name}}` in its text with `copied: name` beside
//! `template`, and the date, as `{{fecha}}` with `date: { name: fecha, format: … }` beside it.
//! Outside a form those markers are espanso's own `clipboard` and `date` variables, which espanso
//! fills in; inside one, espanso never sees the text, so the window fills them in.
//!
//! The invariant: **the hex argument always names the expansion's own trigger.** It is written
//! from the trigger whenever the match is written ([`SimpleMatch::normalized`]), so a trigger
//! renamed anywhere — the editor, a folder's prefix, an import — can never leave the form pointing
//! at a trigger that is gone.
//!
//! The helper exists because espanso has to find Khipu, and nothing espanso knows says where Khipu
//! is: beside the data in one layout, two folders away in another, somewhere under WindowsApps when
//! installed. Khipu writes it into espanso's folder on every start, naming itself; `%CONFIG%` is
//! the one path espanso expands in a script's arguments.
//!
//! What does **not** go here: the window (`crate::ui::form_view`), and the editor's half
//! (`crate::ui::edit_form`).

use super::model::{SimpleMatch, VarEntry};
use serde_norway::{Mapping, Value};
use std::path::Path;

/// The one variable a form has, and what `replace` says to insert it.
pub const VAR_NAME: &str = "khipu";
/// The helper's name inside espanso's config folder.
pub const HELPER: &str = "khipu-form.cmd";
/// The first argument, as written into the YAML. `%CONFIG%` is expanded by espanso itself.
const HELPER_ARG: &str = "%CONFIG%\\khipu-form.cmd";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldKind {
    /// One line.
    #[default]
    Short,
    /// Several lines.
    Long,
    /// One of a list of options.
    Choice,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormField {
    /// What `[[...]]` holds in the text, and the label the field is shown with.
    pub name: String,
    pub kind: FieldKind,
    /// What the field holds when the window opens. For a list, the option chosen at first.
    pub default: String,
    /// A list's options, in order. Ignored by the other two kinds, but kept, so switching a field
    /// to text and back does not lose them.
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormSpec {
    /// The text, with a `[[name]]` where each answer goes.
    pub template: String,
    /// One per name in the text, in the order they first appear.
    pub fields: Vec<FormField>,
    /// The name of the `{{name}}` in the text that stands for what was last copied, if it has one.
    /// Outside a form espanso fills that in itself; inside one, Khipu's window does.
    pub copied: Option<String>,
    /// The date the text holds as `{{name}}`, if it has one: a `date` variable as espanso writes
    /// one, which the window formats the way espanso would.
    pub date: Option<VarEntry>,
}

/// One stretch of a text, as the editor and the window see it.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece<'a> {
    /// Written as it stands.
    Text(&'a str),
    /// A `[[name]]`: its trimmed name, and the marker as written.
    Blank(&'a str, &'a str),
    /// Where what was last copied goes: the marker as written.
    Copied(&'a str),
    /// Where the date goes: the marker as written.
    Date(&'a str),
}

/// Every `{{name}}` for `name` in the text, spaces inside the braces allowed the way espanso
/// allows them, as byte ranges of the whole marker.
pub fn copied_markers(text: &str, name: &str) -> Vec<std::ops::Range<usize>> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(open) = text[at..].find("{{").map(|i| at + i) {
        let inner_start = open + 2;
        let Some(close) = text[inner_start..].find("}}").map(|i| inner_start + i) else {
            break;
        };
        if !name.is_empty() && text[inner_start..close].trim() == name {
            found.push(open..close + 2);
            at = close + 2;
        } else {
            at = open + 1;
        }
    }
    found
}

/// The text cut into what is fixed, its blanks, and where what was last copied and the date go
/// (their `{{name}}` markers, for the names given). The text is cut before anything is put in, so
/// brackets in what was copied, or in an answer, are never read as a blank.
pub fn pieces<'a>(text: &'a str, copied: Option<&str>, date: Option<&str>) -> Vec<Piece<'a>> {
    fn segment<'a>(out: &mut Vec<Piece<'a>>, part: &'a str) {
        let mut last = 0;
        for (range, name) in markers(part) {
            if range.start > last {
                out.push(Piece::Text(&part[last..range.start]));
            }
            out.push(Piece::Blank(name, &part[range.clone()]));
            last = range.end;
        }
        if last < part.len() {
            out.push(Piece::Text(&part[last..]));
        }
    }
    let found = |name: Option<&str>, is_date: bool| {
        name.map(|name| copied_markers(text, name)).unwrap_or_default().into_iter().map(move |r| (r, is_date))
    };
    let mut braces: Vec<_> = found(copied, false).chain(found(date, true)).collect();
    braces.sort_by_key(|(range, _)| range.start);
    let mut out = Vec::new();
    let mut last = 0;
    for (range, is_date) in braces {
        segment(&mut out, &text[last..range.start]);
        let marker = &text[range.clone()];
        out.push(if is_date { Piece::Date(marker) } else { Piece::Copied(marker) });
        last = range.end;
    }
    segment(&mut out, &text[last..]);
    out
}

/// The longest a field's name may be. Past this it is a sentence in brackets, not a blank.
pub const NAME_MAX: usize = 40;

/// Every `[[name]]` in the text, as `(byte range of the whole marker, trimmed name)`.
///
/// A marker never spans a line and never holds another `[`, so a stray `[[` in ordinary text is
/// left as text.
pub fn markers(text: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(open) = text[at..].find("[[").map(|i| at + i) {
        let inner_start = open + 2;
        let Some(close) = text[inner_start..].find("]]").map(|i| inner_start + i) else {
            break;
        };
        let inner = &text[inner_start..close];
        if inner.contains(['[', '\n', '\r']) {
            at = open + 1;
            continue;
        }
        let name = inner.trim();
        if !name.is_empty() && name.chars().count() <= NAME_MAX {
            found.push((open..close + 2, name));
        }
        at = close + 2;
    }
    found
}

/// The names the text asks for, each once, in the order they first appear.
pub fn names(text: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for (_, name) in markers(text) {
        if !seen.iter().any(|s| s == name) {
            seen.push(name.to_string());
        }
    }
    seen
}

/// Every marker naming `old` renamed to `new`, spacing inside the brackets and all.
pub fn rename(template: &str, old: &str, new: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut last = 0;
    for (range, name) in markers(template) {
        if name == old {
            out.push_str(&template[last..range.start]);
            out.push_str("[[");
            out.push_str(new);
            out.push_str("]]");
            last = range.end;
        }
    }
    out.push_str(&template[last..]);
    out
}

/// The trigger as the helper's argument: hexadecimal, so no character of it can mean anything to
/// the command line on the way through.
pub fn encode_trigger(trigger: &str) -> String {
    trigger.bytes().map(|b| format!("{b:02x}")).collect()
}

pub fn decode_trigger(hex: &str) -> Option<String> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

fn key(name: &str) -> Value {
    Value::String(name.to_string())
}

impl FormSpec {
    /// The variable this form is saved as, for the expansion answering to `trigger`.
    pub fn to_var(&self, trigger: &str) -> VarEntry {
        let mut fields = Mapping::new();
        for field in &self.fields {
            let mut spec = Mapping::new();
            let kind = match field.kind {
                FieldKind::Short | FieldKind::Long => "text",
                FieldKind::Choice => "choice",
            };
            spec.insert(key("type"), key(kind));
            if field.kind == FieldKind::Long {
                spec.insert(key("multiline"), Value::Bool(true));
            }
            let options: Vec<Value> = field
                .options
                .iter()
                .map(|o| o.trim())
                .filter(|o| !o.is_empty())
                .map(key)
                .collect();
            if !options.is_empty() {
                spec.insert(key("values"), Value::Sequence(options));
            }
            if !field.default.is_empty() {
                spec.insert(key("default"), key(&field.default));
            }
            fields.insert(key(&field.name), Value::Mapping(spec));
        }
        let mut params = Mapping::new();
        params.insert(
            key("args"),
            Value::Sequence(vec![key(HELPER_ARG), key(&encode_trigger(trigger))]),
        );
        params.insert(key("trim"), Value::Bool(false));
        params.insert(key("template"), key(&self.template));
        params.insert(key("fields"), Value::Mapping(fields));
        if let Some(copied) = &self.copied {
            params.insert(key("copied"), key(copied));
        }
        if let Some(date) = &self.date {
            let mut spec = Mapping::new();
            spec.insert(key("name"), key(&date.name));
            spec.extend(date.params.clone());
            params.insert(key("date"), Value::Mapping(spec));
        }
        VarEntry {
            name: VAR_NAME.to_string(),
            var_type: "script".to_string(),
            params,
            // The text rides along in `template`, and espanso would otherwise read every `{{...}}`
            // in it — what was last copied, or braces the user typed — as a variable of its own,
            // and fail the expansion for want of one.
            inject_vars: Some(false),
        }
    }

    /// The form a match carries, if it is exactly one of ours: one `script` variable that runs the
    /// helper and is the whole replacement. Anything else is not a Khipu form, and is left to the
    /// rules for everything else the editor cannot show.
    pub fn of(m: &SimpleMatch) -> Option<FormSpec> {
        let [var] = m.vars.as_slice() else {
            return None;
        };
        if var.var_type != "script"
            || var.inject_vars == Some(true)
            || m.replace.trim() != format!("{{{{{}}}}}", var.name)
        {
            return None;
        }
        let args = var.params.get("args")?.as_sequence()?;
        let first = args.first()?.as_str()?;
        if !first.to_ascii_lowercase().ends_with(HELPER) || args.len() != 2 {
            return None;
        }
        // Only the keys this module writes, so nothing someone added by hand is lost on a save.
        let known = var.params.keys().all(|k| {
            matches!(k.as_str(), Some("args" | "trim" | "template" | "fields" | "copied" | "date"))
        });
        if !known {
            return None;
        }
        let template = var.params.get("template")?.as_str()?.to_string();
        let copied = match var.params.get("copied") {
            None => None,
            Some(name) => Some(name.as_str()?.to_string()),
        };
        let date = match var.params.get("date") {
            None => None,
            Some(Value::Mapping(spec)) => {
                let mut params = spec.clone();
                let name = params.remove("name")?.as_str()?.to_string();
                let known = params.keys().all(|k| matches!(k.as_str(), Some("format" | "tz" | "locale")));
                if !known || params.get("format").and_then(Value::as_str).is_none() {
                    return None;
                }
                Some(VarEntry { name, var_type: "date".to_string(), params, inject_vars: None })
            }
            Some(_) => return None,
        };
        let declared = match var.params.get("fields") {
            Some(Value::Mapping(map)) => map.clone(),
            None | Some(Value::Null) => Mapping::new(),
            Some(_) => return None,
        };
        let mut fields = Vec::new();
        for name in names(&template) {
            let spec = declared.get(name.as_str());
            let str_of = |k: &str| spec.and_then(|s| s.get(k)).and_then(Value::as_str);
            let options: Vec<String> = spec
                .and_then(|s| s.get("values"))
                .and_then(Value::as_sequence)
                .map(|seq| seq.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let kind = match str_of("type") {
                Some("choice" | "list") => FieldKind::Choice,
                _ if spec.and_then(|s| s.get("multiline")).and_then(Value::as_bool) == Some(true) => {
                    FieldKind::Long
                }
                _ => FieldKind::Short,
            };
            fields.push(FormField {
                name,
                kind,
                default: str_of("default").unwrap_or("").to_string(),
                options,
            });
        }
        Some(FormSpec { template, fields, copied, date })
    }

    /// The pieces of this form's text. See [`pieces`].
    pub fn pieces(&self) -> Vec<Piece<'_>> {
        pieces(&self.template, self.copied.as_deref(), self.date.as_ref().map(|d| d.name.as_str()))
    }

    /// The date as it would be written right now; empty if the form has none.
    pub fn date_text(&self) -> String {
        self.date.as_ref().map(super::presets::sample_of).unwrap_or_default()
    }

    /// The text this form writes: each blank swapped for its answer, and what was last copied and
    /// the date put where they go.
    pub fn filled(&self, copied_text: &str, answer: impl Fn(&str) -> String) -> String {
        let date = self.date_text();
        self.pieces()
            .into_iter()
            .map(|piece| match piece {
                Piece::Text(text) => text.to_string(),
                Piece::Blank(name, _) => answer(name),
                Piece::Copied(_) => copied_text.to_string(),
                Piece::Date(_) => date.clone(),
            })
            .collect()
    }
}

impl FormSpec {
    /// Whether the form writes anything in Devanagari: its text, or what its blanks offer.
    pub fn has_devanagari(&self) -> bool {
        use super::model::has_devanagari;
        has_devanagari(&self.template)
            || has_devanagari(&self.date_text())
            || self.fields.iter().any(FormField::has_devanagari)
    }
}

impl FormField {
    pub fn has_devanagari(&self) -> bool {
        use super::model::has_devanagari;
        has_devanagari(&self.default) || self.options.iter().any(|o| has_devanagari(o))
    }

    /// What the field holds when the window opens: its starting value, or a list's first option.
    pub fn initial(&self) -> String {
        match self.kind {
            FieldKind::Choice if !self.options.contains(&self.default) => {
                self.options.first().cloned().unwrap_or_default()
            }
            _ => self.default.clone(),
        }
    }
}

impl SimpleMatch {
    /// The form this match carries, if it is one of ours. See [`FormSpec::of`].
    pub fn as_form(&self) -> Option<FormSpec> {
        FormSpec::of(self)
    }

    /// The match as it is written to disk: a form's argument re-derived from its trigger, so the
    /// two can never disagree. Everything else is written as it is.
    pub fn normalized(&self) -> SimpleMatch {
        match self.as_form() {
            Some(spec) => SimpleMatch {
                vars: vec![spec.to_var(&self.trigger)],
                ..self.clone()
            },
            None => self.clone(),
        }
    }
}

/// What the helper says: run this Khipu with `--form` and whatever espanso passed on.
///
/// The batch file is read in the console's code page, so it switches that to UTF-8 first: a user
/// folder named «José» is a path the old code page cannot spell. `%` is doubled because a batch
/// file reads it as the start of a variable.
fn helper_text(exe: &Path) -> String {
    let exe = exe.display().to_string().replace('%', "%%");
    format!("@echo off\r\nchcp 65001 >nul\r\n\"{exe}\" --form %*\r\n")
}

/// Writes the helper into espanso's config folder, unless it already says the same thing.
pub fn write_helper(espanso_config: &Path, exe: &Path) -> std::io::Result<()> {
    let path = espanso_config.join(HELPER);
    let text = helper_text(exe);
    if std::fs::read(&path).is_ok_and(|old| old == text.as_bytes()) {
        return Ok(());
    }
    std::fs::create_dir_all(espanso_config)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::model::{MatchEntry, MatchFile};

    #[test]
    fn markers_are_found_in_order_and_each_name_once() {
        let text = "Hola [[ cliente ]], sobre [[asunto]]. Gracias, [[cliente]]. [[]] [[a\nb]] [x]";
        assert_eq!(names(text), ["cliente", "asunto"]);
        assert_eq!(
            FormSpec { template: text.into(), ..Default::default() }.filled("", |n| n.to_uppercase()),
            "Hola CLIENTE, sobre ASUNTO. Gracias, CLIENTE. [[]] [[a\nb]] [x]"
        );
        assert_eq!(rename("[[a]] y [[ a ]] y [[b]]", "a", "c"), "[[c]] y [[c]] y [[b]]");
    }

    #[test]
    fn the_trigger_survives_its_trip_through_hex() {
        for trigger in [":cliente", ":ñandú", "\"%&|<>^", ""] {
            assert_eq!(decode_trigger(&encode_trigger(trigger)).as_deref(), Some(trigger));
        }
        assert_eq!(decode_trigger("3"), None);
        assert_eq!(decode_trigger("zz"), None);
    }

    fn form_match(trigger: &str) -> SimpleMatch {
        let spec = FormSpec {
            template: "Hola [[cliente]],\n[[mensaje]]\n[[tono]]".to_string(),
            fields: vec![
                FormField { name: "cliente".into(), ..Default::default() },
                FormField {
                    name: "mensaje".into(),
                    kind: FieldKind::Long,
                    default: "Te escribo por".into(),
                    ..Default::default()
                },
                FormField {
                    name: "tono".into(),
                    kind: FieldKind::Choice,
                    default: "Cercano".into(),
                    options: vec!["Formal".into(), "Cercano".into()],
                },
            ],
            copied: None,
            date: None,
        };
        SimpleMatch {
            trigger: trigger.to_string(),
            replace: format!("{{{{{VAR_NAME}}}}}"),
            vars: vec![spec.to_var(trigger)],
            label: Some("Correo".into()),
        }
    }

    /// Written, read back from the text of a file, and recognised as the same form.
    #[test]
    fn a_form_round_trips_through_the_file() {
        let m = form_match(":cliente");
        let mut file = MatchFile::default();
        file.entries.push(MatchEntry::Simple(m.clone()));
        let text = file.to_string_with_banner().unwrap();
        assert!(text.contains("khipu-form.cmd"), "{text}");
        assert!(text.contains("inject_vars: false"), "{text}");
        let back = MatchFile::from_str(&text).unwrap();
        let MatchEntry::Simple(read) = &back.entries[0] else {
            panic!("a form must load as an expansion the editor can open");
        };
        assert_eq!(read, &m);
        assert_eq!(read.as_form(), m.as_form());
        assert!(back.entries[0].is_safely_editable());
        let spec = read.as_form().unwrap();
        let initial: Vec<String> = spec.fields.iter().map(FormField::initial).collect();
        assert_eq!(initial, ["", "Te escribo por", "Cercano"]);
    }

    /// A form saved before `inject_vars` was written still opens as one, and a variable with a key
    /// Khipu does not know keeps the whole expansion out of the editor, and the key in the file.
    #[test]
    fn a_var_key_khipu_does_not_know_is_kept() {
        let mut old = form_match(":cliente");
        old.vars[0].inject_vars = None;
        assert!(old.as_form().is_some());

        let yaml = "matches:\n- trigger: ':x'\n  replace: '{{a}} {{b}}'\n  vars:\n  - name: a\n    type: clipboard\n  - name: b\n    type: echo\n    depends_on: [a]\n    params: {echo: hola}\n";
        let file = MatchFile::from_str(yaml).unwrap();
        assert!(matches!(file.entries[0], MatchEntry::Advanced(_)));
        assert!(file.to_string_with_banner().unwrap().contains("depends_on"));
    }

    /// A trigger renamed by something that knows nothing of forms — a folder's prefix — is put
    /// back into the argument the moment the match is written.
    #[test]
    fn the_argument_follows_the_trigger_when_written() {
        let mut m = form_match(":cliente");
        m.trigger = "//cliente".to_string();
        let mut file = MatchFile::default();
        file.entries.push(MatchEntry::Simple(m));
        let text = file.to_string_with_banner().unwrap();
        assert!(text.contains(&encode_trigger("//cliente")), "{text}");
        assert!(!text.contains(&encode_trigger(":cliente")), "{text}");
    }

    /// Anything added by hand that this module does not write keeps the match out of the editor,
    /// which would otherwise drop it on the next save.
    #[test]
    fn a_hand_added_parameter_makes_it_something_else() {
        let mut m = form_match(":c");
        m.vars[0].params.insert(key("debug"), Value::Bool(true));
        assert_eq!(m.as_form(), None);
        assert!(!MatchEntry::Simple(m).is_safely_editable());
    }

    /// What was last copied goes where its marker is, and brackets in it are not blanks.
    #[test]
    fn what_was_copied_is_put_in_and_never_read_as_a_blank() {
        let mut m = form_match(":c");
        let mut spec = m.as_form().unwrap();
        spec.template = "Hola [[cliente]]: {{ copiado }} y {{otro}}".into();
        spec.copied = Some("copiado".into());
        m.vars = vec![spec.to_var(":c")];
        let back = m.as_form().unwrap();
        assert_eq!(back.copied.as_deref(), Some("copiado"));
        assert_eq!(names(&back.template), ["cliente"]);
        let out = back.filled("[[x]] 50%", |n| n.to_uppercase());
        assert_eq!(out, "Hola CLIENTE: [[x]] 50% y {{otro}}");
        assert_eq!(copied_markers("{{a}}{{ a }}{{ab}}{{a", "a"), [0..5, 5..12]);
    }

    /// The date rides along in the form, is read back as the same variable, and is written where
    /// its marker is, beside what was copied.
    #[test]
    fn the_date_is_put_in_where_its_marker_is() {
        let mut m = form_match(":c");
        let mut spec = m.as_form().unwrap();
        spec.template = "{{copiado}} [[cliente]], {{fecha}}".into();
        spec.fields.truncate(1);
        spec.copied = Some("copiado".into());
        spec.date = Some(super::super::presets::DatePreset::IsoDateOnly.build_var("fecha", crate::i18n::Lang::Es));
        m.vars = vec![spec.to_var(":c")];
        let back = m.as_form().unwrap();
        assert_eq!(back, spec);
        let today = crate::datefmt::sample("%Y-%m-%d", None, crate::i18n::Lang::Es);
        assert_eq!(back.filled("A", |n| n.to_uppercase()), format!("A CLIENTE, {today}"));
    }

    #[test]
    fn the_helper_names_this_khipu_and_doubles_its_percent_signs() {
        let text = helper_text(Path::new("C:\\Users\\José\\100%\\Khipu.exe"));
        assert!(text.contains("\"C:\\Users\\José\\100%%\\Khipu.exe\" --form %*"), "{text}");
        assert!(text.starts_with("@echo off\r\nchcp 65001"));
    }
}
