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

//! The shape of `base.yml`, and the promise printed at the top of it.
//!
//! That banner says Khipu preserves any match it cannot show in its interface, and this
//! module is where that is kept or broken. Two mechanisms hold it:
//!
//! - `MatchEntry::Advanced` carries anything the form cannot represent — shell commands, regex
//!   triggers, per-app rules — as its original `Value`, untouched, and writes it back out
//!   unchanged. `is_safely_editable` is what the interface asks before offering to edit a row.
//! - `MatchFile::extra_top_level` does the same for keys beside `matches:` that we know nothing
//!   about, such as `global_vars`.
//! - `MatchFile::off_kept` and `off_top_level` do both for `_disabled.yml`, the file of expansions
//!   switched off, which gets the same promise in a banner of its own.
//!
//! Which is why a `matches:` that is neither a list nor absent is a hard error rather than an empty
//! file: taking it for "no expansions" and saving over it would delete everything the user had,
//! under a banner promising the opposite. An absent or null `matches:` is genuinely empty and
//! loads fine.

use crate::i18n::Strings;
use serde::{Deserialize, Serialize};
use serde_norway::{Mapping, Value};
use std::collections::BTreeSet;

/// A `vars:` entry inside a match, e.g. the "current date" variable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VarEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub var_type: String,
    #[serde(default, skip_serializing_if = "Mapping::is_empty")]
    pub params: Mapping,
}

impl VarEntry {
    pub fn param_str(&self, key: &str) -> Option<&str> {
        self.params.get(key)?.as_str()
    }
}

/// A match the GUI fully understands and can edit with a simple form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimpleMatch {
    pub trigger: String,
    pub replace: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<VarEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl SimpleMatch {
    /// If this match is exactly a "insert current date/time" expansion created by one of our
    /// presets (a single `date`-typed var, referenced verbatim as the whole replacement), returns
    /// that var. Used so editing a previously-created date expansion re-opens the preset picker
    /// instead of the raw-text editor.
    pub fn as_date_var(&self) -> Option<&VarEntry> {
        if self.vars.len() != 1 {
            return None;
        }
        let var = &self.vars[0];
        if var.var_type != "date" {
            return None;
        }
        if self.replace.trim() == format!("{{{{{}}}}}", var.name) {
            Some(var)
        } else {
            None
        }
    }

    /// The text espanso would insert for this match, when it can be known for certain without
    /// asking espanso: plain text, or one date we format letter for letter the way espanso does.
    ///
    /// `None` for everything else, which the launcher hands to espanso instead: other kinds of
    /// `vars`, a `{{...}}` that could name a global var, the `$|$` cursor mark, a date code, time
    /// zone, language or parameter that [`crate::datefmt`] does not reproduce.
    pub fn literal_text(&self) -> Option<String> {
        if self.vars.is_empty() {
            let plain = !self.replace.contains("{{") && !self.replace.contains("$|$");
            return plain.then(|| self.replace.clone());
        }
        let var = self.as_date_var()?;
        let format = var.param_str("format")?;
        let tz = var.param_str("tz");
        let locale = var.param_str("locale");
        let known_params = var.params.keys().all(|key| {
            matches!(key.as_str(), Some("format" | "tz" | "locale"))
        });
        let known_locale = locale.is_none_or(|tag| {
            crate::datefmt::locale_tag(crate::datefmt::lang_from_locale_tag(Some(tag))) == tag
        });
        if !known_params
            || !known_locale
            || !tz.is_none_or(|tz| tz.eq_ignore_ascii_case("UTC"))
            || !crate::datefmt::understands(format)
        {
            return None;
        }
        let date = crate::datefmt::sample(format, tz, crate::datefmt::lang_from_locale_tag(locale));
        Some(self.replace.replacen(&format!("{{{{{}}}}}", var.name), &date, 1))
    }
}

/// One entry of `matches:`. Anything the GUI doesn't fully understand (plural `triggers:`,
/// `regex:`, per-app conditionals, unknown extra fields, ...) is kept as an opaque, untouched
/// value so editing OTHER matches can never corrupt it.
#[derive(Debug, Clone)]
pub enum MatchEntry {
    Simple(SimpleMatch),
    Advanced(Value),
}

impl MatchEntry {
    /// Identity used for bookkeeping (folder assignment, selection, lookups). Never translated, so
    /// switching languages can't silently detach an expansion from its folder.
    ///
    /// Borrowed rather than cloned: the list re-derives itself from these on every rebuild, and at
    /// a few thousand expansions an allocation per entry per pass is real work for no gain.
    pub fn trigger_str(&self) -> &str {
        match self {
            MatchEntry::Simple(m) => m.trigger.as_str(),
            MatchEntry::Advanced(v) => v.get("trigger").and_then(Value::as_str).unwrap_or(""),
        }
    }

    /// The name the user gave it, kept in espanso's own `label` key. `None` for one without a name,
    /// and for a name that is only spaces: shown, that would be a gap where the text should start.
    pub fn name(&self) -> Option<&str> {
        let label = match self {
            MatchEntry::Simple(m) => m.label.as_deref(),
            MatchEntry::Advanced(v) => v.get("label").and_then(Value::as_str),
        };
        label.map(str::trim).filter(|name| !name.is_empty())
    }

    /// Owned form of [`trigger_str`], for the places that need to keep it past the borrow.
    pub fn trigger_label(&self) -> String {
        self.trigger_str().to_string()
    }

    pub fn preview(&self, t: &'static Strings) -> String {
        match self {
            MatchEntry::Simple(m) => {
                if let Some(var) = m.as_date_var() {
                    // Shown as a worked example ("e.g. August 29, 2026") rather than as the
                    // strftime pattern that produced it: the pattern is an implementation detail
                    // that means nothing to the people this list is for.
                    let example = crate::datefmt::sample(
                        var.param_str("format").unwrap_or(""),
                        var.param_str("tz"),
                        super::presets::month_lang_of(var),
                    );
                    crate::i18n::fill(t.preview_date, &[("example", &example)])
                } else if let Some(form) = m.as_form() {
                    // The text with its blanks showing, so a form reads as the message it writes.
                    form.template.chars().map(|c| if c == '\n' { ' ' } else { c }).collect()
                } else if !m.vars.is_empty() {
                    t.preview_advanced_vars.to_string()
                } else {
                    m.replace
                        .chars()
                        .map(|c| if c == '\n' { ' ' } else { c })
                        .collect()
                }
            }
            MatchEntry::Advanced(_) => t.preview_advanced.to_string(),
        }
    }

    /// The text as the user wrote it, line breaks and all, for the one place that shows it whole
    /// and copies it. [`preview`](Self::preview) flattens lines because a list row has one; a
    /// three-line signature read back as one line, and was copied as one, is not the signature.
    /// Dates and the matches the form cannot edit have no single text to show, so they keep
    /// their preview.
    pub fn full_text(&self, t: &'static Strings) -> String {
        match self {
            MatchEntry::Simple(m) if m.vars.is_empty() => m.replace.clone(),
            MatchEntry::Simple(m) => match m.as_form() {
                Some(form) => form.template,
                None => self.preview(t),
            },
            _ => self.preview(t),
        }
    }

    /// Whether the simple form can safely rewrite this match's `replace`/`vars` without losing
    /// anything. A plain text match, one of our own date presets or one of our own forms (see
    /// [`super::form`]) qualifies; a match with other kinds of `vars` (shell commands, clipboard,
    /// ...) round-trips fine when left untouched, but our form doesn't model those, so it must not
    /// be allowed to silently drop them on save.
    pub fn is_safely_editable(&self) -> bool {
        match self {
            MatchEntry::Simple(m) => {
                m.vars.is_empty() || m.as_date_var().is_some() || m.as_form().is_some()
            }
            MatchEntry::Advanced(_) => false,
        }
    }
}

/// Internal shape used only to classify+round-trip a `matches:` entry. `extra` catches any field
/// the GUI doesn't model; if it's non-empty after parsing, the entry is treated as Advanced.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SimpleMatchRaw {
    trigger: String,
    replace: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    vars: Vec<VarEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(flatten)]
    extra: Mapping,
}

fn classify(value: &Value) -> MatchEntry {
    if let Ok(raw) = serde_norway::from_value::<SimpleMatchRaw>(value.clone()) {
        if raw.extra.is_empty() {
            return MatchEntry::Simple(SimpleMatch {
                trigger: raw.trigger,
                replace: raw.replace,
                vars: raw.vars,
                label: raw.label,
            });
        }
    }
    MatchEntry::Advanced(value.clone())
}

fn to_value(entry: &MatchEntry) -> Value {
    match entry {
        MatchEntry::Simple(m) => {
            // A form's argument is re-derived from its trigger here, the one door every match
            // leaves by. See [`super::form`].
            let m = m.normalized();
            let raw = SimpleMatchRaw {
                trigger: m.trigger,
                replace: m.replace,
                vars: m.vars,
                label: m.label,
                extra: Mapping::new(),
            };
            serde_norway::to_value(&raw).expect("SimpleMatchRaw always serializes")
        }
        MatchEntry::Advanced(v) => v.clone(),
    }
}

/// A whole `*.yml` match file: our editable list of matches, plus any top-level keys we don't
/// touch (e.g. `global_vars:`, `imports:`) preserved exactly as read.
///
/// Read from disk it is the whole library, and that is two files: `base.yml` and the expansions
/// that are switched off, in [`OFF_FILE`] beside it. In memory they are one list — `entries` holds
/// both, and `off` says which are off — so that everything that looks a trigger up (the duplicate
/// check, folders, the editor, an import) sees an expansion that is off exactly as it sees one that
/// is on. Only [`split`](Self::split) tells them apart, on the way back to disk.
#[derive(Debug, Clone, Default)]
pub struct MatchFile {
    pub entries: Vec<MatchEntry>,
    pub extra_top_level: Mapping,
    /// Triggers of the expansions that are switched off. Only ever names a `Simple` entry.
    pub off: BTreeSet<String>,
    /// What the file of switched-off expansions holds that is not an expansion this app shows: a
    /// match the form cannot edit, or a second copy of a trigger `base.yml` already has. Written
    /// back to that file untouched, never shown and never switched on — the same promise
    /// `Advanced` keeps for `base.yml`.
    pub off_kept: Vec<Value>,
    /// Top-level keys of that file other than `matches:`, kept as read.
    pub off_top_level: Mapping,
}

/// Where the switched-off expansions live, beside `base.yml`.
///
/// Espanso loads every file in its `match` folder **except** the ones whose name starts with an
/// underscore — its documented way to keep a file it should not read. So these are still a match
/// file in espanso's own format, readable and movable by hand, and none of them fires.
pub const OFF_FILE: &str = "_disabled.yml";

/// Whether `text` has a character from the Devanagari block, which only the system's Devanagari
/// face can draw. See [`crate::fonts`].
pub fn has_devanagari(text: &str) -> bool {
    text.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

impl MatchFile {
    /// Whether anything the list shows needs the Devanagari face: an expansion written in it, or a
    /// date that spells its month in Hindi. The face is otherwise loaded only for a Hindi interface,
    /// and without it these come out as rows of empty boxes.
    pub fn wants_devanagari(&self) -> bool {
        self.entries.iter().any(|entry| match entry {
            MatchEntry::Simple(m) => {
                has_devanagari(&m.trigger)
                    || has_devanagari(&m.replace)
                    || m.label.as_deref().is_some_and(has_devanagari)
                    || m.vars.iter().any(|var| {
                        crate::datefmt::lang_from_locale_tag(var.param_str("locale"))
                            == crate::i18n::Lang::Hi
                            && var.param_str("locale").is_some()
                    })
            }
            MatchEntry::Advanced(_) => {
                has_devanagari(entry.trigger_str()) || entry.name().is_some_and(has_devanagari)
            }
        })
    }

    pub fn from_str(contents: &str) -> Result<Self, serde_norway::Error> {
        let root: Value = serde_norway::from_str(contents)?;
        let mut top = match root {
            Value::Mapping(m) => m,
            Value::Null => Mapping::new(),
            other => {
                // Not a mapping at all: treat the whole thing as unparseable rather than guessing.
                use serde::de::Error as _;
                return Err(serde_norway::Error::custom(format!(
                    "unexpected top-level shape in match file: {other:?}"
                )));
            }
        };

        let matches_key = Value::String("matches".to_string());
        let entries = match top.remove(&matches_key) {
            Some(Value::Sequence(seq)) => seq.iter().map(classify).collect(),
            // `matches:` with nothing under it, or no key at all. An empty file is a fine file.
            Some(Value::Null) | None => Vec::new(),
            // Anything else is refused rather than read as "no matches", for the same reason the
            // root shape above is. `remove` has already taken the value out, so it would not
            // survive in `extra_top_level` either: the next save would write `matches: []` over
            // whatever was there. Forgetting the leading `-` on a hand-edited entry is all it takes
            //
            //     matches:
            //       trigger: ":hi"
            //       replace: hola
            //
            // to turn every expansion in the file into a mapping — and the banner this very module
            // writes promises that anything it cannot show is preserved.
            Some(other) => {
                use serde::de::Error as _;
                return Err(serde_norway::Error::custom(format!(
                    "`matches:` is not a list: {other:?}"
                )));
            }
        };

        Ok(MatchFile {
            entries,
            extra_top_level: top,
            ..Default::default()
        })
    }

    pub fn is_off(&self, trigger: &str) -> bool {
        self.off.contains(trigger)
    }

    /// Takes in the file of switched-off expansions, read on its own with [`from_str`].
    ///
    /// Each expansion this app shows joins `entries` and is marked off. Anything else stays in
    /// `off_kept`, including an expansion whose trigger `base.yml` already has: the one that fires
    /// wins, and the other is kept where it was rather than dropped, since it may not say the same.
    /// One that says exactly the same is dropped: that is what a save interrupted between its two
    /// files leaves behind (see [`save`](super::io::save)), and keeping it would carry a dead copy
    /// along for ever.
    ///
    /// [`from_str`]: Self::from_str
    pub fn take_off_file(&mut self, file: MatchFile) {
        for entry in file.entries {
            if let MatchEntry::Simple(m) = &entry {
                let same = |e: &MatchEntry| matches!(e, MatchEntry::Simple(b) if b == m);
                if self.entries.iter().any(same) {
                    continue;
                }
            }
            match entry {
                MatchEntry::Simple(m)
                    if !m.trigger.trim().is_empty()
                        && !self.entries.iter().any(|e| e.trigger_str() == m.trigger) =>
                {
                    self.off.insert(m.trigger.clone());
                    self.entries.push(MatchEntry::Simple(m));
                }
                other => self.off_kept.push(to_value(&other)),
            }
        }
        self.off_top_level = file.extra_top_level;
    }

    /// The two files this library is written to: what espanso loads, and what is switched off.
    pub fn split(&self) -> (MatchFile, MatchFile) {
        let (off, on): (Vec<&MatchEntry>, Vec<&MatchEntry>) = self
            .entries
            .iter()
            .partition(|e| matches!(e, MatchEntry::Simple(m) if self.off.contains(&m.trigger)));
        let base = MatchFile {
            entries: on.into_iter().cloned().collect(),
            extra_top_level: self.extra_top_level.clone(),
            ..Default::default()
        };
        let mut off_entries: Vec<MatchEntry> = off.into_iter().cloned().collect();
        off_entries.extend(self.off_kept.iter().cloned().map(MatchEntry::Advanced));
        let off_file = MatchFile {
            entries: off_entries,
            extra_top_level: self.off_top_level.clone(),
            ..Default::default()
        };
        (base, off_file)
    }

    /// Moves the off mark with a trigger that changes, the way the folder assignment moves.
    pub fn rename_trigger(&mut self, old: &str, new: &str) {
        self.rename_triggers([(old, new)]);
    }

    /// The same for many renames made at once. Every mark is lifted before any is set down, so a
    /// chain — `:a` becomes `:b` while `:b` becomes `:c` — moves each mark one step and no further.
    pub fn rename_triggers<'a>(&mut self, renames: impl IntoIterator<Item = (&'a str, &'a str)>) {
        let moving: Vec<(&str, &str)> = renames
            .into_iter()
            .filter(|(old, new)| old != new && self.off.contains(*old))
            .collect();
        for (old, _) in &moving {
            self.off.remove(*old);
        }
        for (_, new) in moving {
            self.off.insert(new.to_string());
        }
    }

    /// Forgets the mark of every trigger that is no longer in the list, so an expansion made later
    /// with the same trigger is not born switched off.
    pub fn prune_off(&mut self) {
        let present: std::collections::HashSet<&str> =
            self.entries.iter().map(MatchEntry::trigger_str).collect();
        self.off.retain(|t| present.contains(t.as_str()));
    }

    /// The file of switched-off expansions as text, with a banner of its own saying why espanso
    /// leaves it alone.
    pub fn to_string_as_off_file(&self) -> Result<String, serde_norway::Error> {
        let body = self.body()?;
        Ok(format!(
            "# Expansions switched off in Khipu. Espanso does not load files whose name\n\
             # starts with an underscore, so none of these fire. Turn them back on in the app.\n\
             # https://espanso.org/docs/\n\n{body}"
        ))
    }

    fn body(&self) -> Result<String, serde_norway::Error> {
        let mut root = Mapping::new();
        let seq: Vec<Value> = self.entries.iter().map(to_value).collect();
        root.insert(Value::String("matches".to_string()), Value::Sequence(seq));
        for (k, v) in &self.extra_top_level {
            root.insert(k.clone(), v.clone());
        }
        serde_norway::to_string(&Value::Mapping(root))
    }

    pub fn to_string_with_banner(&self) -> Result<String, serde_norway::Error> {
        let body = self.body()?;
        Ok(format!(
            // Kept in English regardless of interface language: this is a comment inside a shared
            // config file that a colleague or a support engineer may open on any machine.
            "# Managed by Khipu — you can still edit this file by hand;\n\
             # Khipu preserves any match it cannot show in its interface.\n\
             # https://espanso.org/docs/\n\n{body}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn literal(yaml: &str) -> Option<String> {
        let mf = MatchFile::from_str(yaml).expect("loads");
        match &mf.entries[0] {
            MatchEntry::Simple(m) => m.literal_text(),
            MatchEntry::Advanced(_) => panic!("{yaml:?} should be simple"),
        }
    }

    /// What the launcher pastes itself, and what it has to leave to espanso because only espanso
    /// knows what it would come out as.
    #[test]
    fn the_launcher_pastes_only_what_it_can_write_exactly() {
        assert_eq!(
            literal("matches:\n  - trigger: \":sig\"\n    replace: |-\n      Saludos,\n      Alex\n"),
            Some("Saludos,\nAlex".to_string())
        );
        // A global var, and the cursor mark.
        assert_eq!(literal("matches:\n  - trigger: \":g\"\n    replace: \"{{nombre}}\"\n"), None);
        assert_eq!(literal("matches:\n  - trigger: \":c\"\n    replace: \"<b>$|$</b>\"\n"), None);

        let date = |params: &str| {
            literal(&format!(
                "matches:\n  - trigger: \":d\"\n    replace: \"{{{{d}}}}\"\n    vars:\n      - name: d\n        type: date\n        params:\n{params}"
            ))
        };
        let iso = date("          format: \"%Y-%m-%d\"\n").expect("our preset");
        assert_eq!(iso.len(), 10);
        assert_eq!(&iso[4..5], "-");
        assert!(date("          format: \"%B %-d, %Y\"\n          locale: es-ES\n").is_some());
        assert!(date("          format: \"%Y\"\n          tz: UTC\n").is_some());
        // A code we do not model, a zone we do not model, a language we do not have, a parameter
        // we do not know.
        assert_eq!(date("          format: \"%j\"\n"), None);
        assert_eq!(date("          format: \"%Y\"\n          tz: Europe/Madrid\n"), None);
        assert_eq!(date("          format: \"%B\"\n          locale: fr-FR\n"), None);
        assert_eq!(date("          format: \"%Y\"\n          offset: 86400\n"), None);
    }

    #[test]
    fn the_whole_text_keeps_its_lines_and_only_the_preview_flattens_them() {
        let text = "matches:\n  - trigger: \":sig\"\n    replace: |-\n      Best regards,\n      Jamie\n";
        let mf = MatchFile::from_str(text).expect("loads");
        let t = crate::i18n::Lang::En.strings();
        assert_eq!(mf.entries[0].full_text(t), "Best regards,\nJamie");
        assert_eq!(mf.entries[0].preview(t), "Best regards, Jamie");
    }

    #[test]
    fn an_empty_matches_key_is_an_empty_file_and_nothing_worse() {
        for text in ["matches:\n", "matches: []\n", "# nothing here\n", ""] {
            let mf = MatchFile::from_str(text)
                .unwrap_or_else(|e| panic!("{text:?} should load: {e}"));
            assert!(mf.entries.is_empty());
        }
    }

    #[test]
    fn matches_that_is_not_a_list_is_refused_instead_of_emptied() {
        // The leading `-` forgotten while hand-editing. This used to load as zero matches without
        // a word, and the next save wrote `matches: []` over it.
        let text = "matches:\n  trigger: \":hi\"\n  replace: hola\n";
        assert!(MatchFile::from_str(text).is_err());
        // A scalar is refused the same way.
        assert!(MatchFile::from_str("matches: hola\n").is_err());
    }

    #[test]
    fn a_list_still_loads_and_keeps_the_other_top_level_keys() {
        let text = "imports:\n  - other.yml\nmatches:\n  - trigger: \":hi\"\n    replace: hola\n";
        let mf = MatchFile::from_str(text).unwrap();
        assert_eq!(mf.entries.len(), 1);
        assert_eq!(mf.entries[0].trigger_str(), ":hi");
        assert!(mf
            .extra_top_level
            .contains_key(Value::String("imports".to_string())));
    }

    #[test]
    fn a_chain_of_renames_moves_each_off_mark_one_step() {
        let mut mf = MatchFile::default();
        mf.off.insert(":a".to_string());
        mf.rename_triggers([(":a", ":b"), (":b", ":c")]);
        assert_eq!(mf.off.iter().collect::<Vec<_>>(), [":b"]);
    }
}
