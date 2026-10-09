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

/// The text with every marker swapped for its answer. A name with no answer comes out empty.
pub fn fill(template: &str, answer: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(template.len());
    let mut last = 0;
    for (range, name) in markers(template) {
        out.push_str(&template[last..range.start]);
        out.push_str(&answer(name));
        last = range.end;
    }
    out.push_str(&template[last..]);
    out
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
        VarEntry {
            name: VAR_NAME.to_string(),
            var_type: "script".to_string(),
            params,
        }
    }

    /// The form a match carries, if it is exactly one of ours: one `script` variable that runs the
    /// helper and is the whole replacement. Anything else is not a Khipu form, and is left to the
    /// rules for everything else the editor cannot show.
    pub fn of(m: &SimpleMatch) -> Option<FormSpec> {
        let [var] = m.vars.as_slice() else {
            return None;
        };
        if var.var_type != "script" || m.replace.trim() != format!("{{{{{}}}}}", var.name) {
            return None;
        }
        let args = var.params.get("args")?.as_sequence()?;
        let first = args.first()?.as_str()?;
        if !first.to_ascii_lowercase().ends_with(HELPER) || args.len() != 2 {
            return None;
        }
        // Only the keys this module writes, so nothing someone added by hand is lost on a save.
        let known = var.params.keys().all(|k| {
            matches!(k.as_str(), Some("args" | "trim" | "template" | "fields"))
        });
        if !known {
            return None;
        }
        let template = var.params.get("template")?.as_str()?.to_string();
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
        Some(FormSpec { template, fields })
    }
}

impl FormField {
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
            fill(text, |n| n.to_uppercase()),
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

    #[test]
    fn the_helper_names_this_khipu_and_doubles_its_percent_signs() {
        let text = helper_text(Path::new("C:\\Users\\José\\100%\\Khipu.exe"));
        assert!(text.contains("\"C:\\Users\\José\\100%%\\Khipu.exe\" --form %*"), "{text}");
        assert!(text.starts_with("@echo off\r\nchcp 65001"));
    }
}
