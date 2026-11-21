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

//! Expansions with format: bold, italics and lists, written the way chat apps write them
//! (`**bold**`, `*italics*`, a line starting with `- ` or `1. `).
//!
//! Such a text is saved under espanso's `markdown:` key instead of `replace:`. Espanso turns it
//! into HTML and always pastes it, never types it, putting both the formatted version and a plain
//! one on the clipboard: an app that understands format takes the first, any other the second.
//!
//! Espanso reads Markdown strictly, which is not how people write. Two things are bridged here,
//! in both directions, so the editor keeps showing exactly what the user typed:
//!
//! - A single line break means a new line, not a space: one is marked as a hard break (a `\` at
//!   the end of the line) wherever Markdown would otherwise join two lines.
//! - `<` and `\` are taken literally, not as HTML or escapes.
//!
//! Anything else Markdown knows (headings, quotes, links) is left as Markdown reads it. A
//! `markdown:` written by hand that this round trip would change is not taken as ours: the model
//! keeps it untouched as an advanced match.

/// Whether the text uses any of the format the editor offers.
pub fn is_rich(text: &str) -> bool {
    text.lines().any(|line| numbered_or_bullet(line) || emphasised(line))
}

/// The Markdown espanso is given for a text written in the editor.
pub fn to_markdown(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::with_capacity(text.len() + 16);
    for (i, line) in lines.iter().enumerate() {
        for c in line.chars() {
            if matches!(c, '\\' | '<') {
                out.push('\\');
            }
            out.push(c);
        }
        if let Some(next) = lines.get(i + 1) {
            if !line.trim().is_empty() && !next.trim().is_empty() && !list_item(next) {
                out.push('\\');
            }
            out.push('\n');
        }
    }
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The text the editor shows for Markdown written by [`to_markdown`]; `None` for Markdown that
/// [`to_markdown`] would not write back the same.
pub fn from_markdown(markdown: &str) -> Option<String> {
    let mut text = String::with_capacity(markdown.len());
    for (i, line) in markdown.split('\n').enumerate() {
        if i > 0 {
            text.push('\n');
        }
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some(escaped @ ('\\' | '<')) => text.push(escaped),
                    None => {} // the hard break
                    Some(other) => {
                        text.push(c);
                        text.push(other);
                    }
                }
            } else {
                text.push(c);
            }
        }
    }
    (to_markdown(&text) == markdown).then_some(text)
}

/// A list as people write one: a bullet, or a number of one or two digits. A year that opens a
/// line, «2026. Año», is a number Markdown would take as a list too, but nobody means it as one.
fn numbered_or_bullet(line: &str) -> bool {
    let digits = line.trim_start().chars().take_while(char::is_ascii_digit).count();
    list_item(line) && digits <= 2
}

/// A line Markdown starts a list item with: `- `, `* `, `+ `, or a number and `. ` or `) `.
fn list_item(line: &str) -> bool {
    let line = line.trim_start();
    if ["- ", "* ", "+ "].iter().any(|mark| line.starts_with(mark)) {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    (1..=9).contains(&digits) && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}

/// Whether the line has a stretch between asterisks, `*like this*`, that Markdown would emphasise:
/// the opening one followed by a letter, the closing one preceded by one.
fn emphasised(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let opens = |i: usize| chars.get(i + 1).is_some_and(|c| !c.is_whitespace() && *c != '*');
    let closes = |i: usize| i > 0 && !chars[i - 1].is_whitespace() && chars[i - 1] != '*';
    let mut open = None;
    for (i, c) in chars.iter().enumerate() {
        if *c != '*' {
            continue;
        }
        if open.is_some() && closes(i) {
            return true;
        }
        if open.is_none() && opens(i) {
            open = Some(i);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_is_noticed_and_plain_text_is_left_plain() {
        for rich in ["Hola **Ana**.", "Es *muy* importante.", "Lista:\n- uno\n- dos", "1. Primero"] {
            assert!(is_rich(rich), "{rich:?}");
        }
        for plain in ["Hola Ana.", "5 * 3 = 15", "a* b", "Precio: 2*", "-sin espacio", "2026. Año"] {
            assert!(!is_rich(plain), "{plain:?}");
        }
    }

    #[test]
    fn lines_stay_lines_and_paragraphs_stay_paragraphs() {
        let text = "Hola **Ana**,\ngracias.\n\nPuntos:\n- uno\n- dos\nFin <3 C:\\x";
        let md = to_markdown(text);
        assert_eq!(md, "Hola **Ana**,\\\ngracias.\n\nPuntos:\n- uno\n- dos\\\nFin \\<3 C:\\\\x");
        assert_eq!(from_markdown(&md).as_deref(), Some(text));
    }

    #[test]
    fn markdown_written_another_way_is_not_taken_as_ours() {
        assert_eq!(from_markdown("uno\ndos"), None);
        assert_eq!(from_markdown("\\*literal\\*").as_deref(), None);
        assert_eq!(from_markdown("**ya**").as_deref(), Some("**ya**"));
    }
}
