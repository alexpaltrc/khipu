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
//! Espanso reads Markdown strictly, which is not how people write. Only the four marks above are
//! format; everything else is taken as written, in both directions, so the editor keeps showing
//! exactly what the user typed:
//!
//! - A single line break means a new line, not a space: it is marked as a hard break (a `\` at
//!   the end of the line) wherever Markdown would otherwise join two lines.
//! - An asterisk with no partner, as in `*8374`, is an asterisk. So are `_`, `#`, `<`, `&` and
//!   the rest of what Markdown would read as headings, links, code or HTML: each goes out escaped.
//!
//! A `markdown:` written by hand that this round trip would change is not taken as ours: the model
//! keeps it untouched as an advanced match.
//!
//! [`from_html`] goes the other way for a paste: format copied from Teams, Word, Outlook or a web
//! page arrives in the editor as these same marks.

/// Characters Markdown would read as something other than themselves, escaped wherever they are.
const ALWAYS_ESCAPED: &[char] = &['\\', '<', '>', '_', '`', '[', ']', '&', '#'];

/// Whether the text uses any of the format the editor offers.
pub fn is_rich(text: &str) -> bool {
    text.lines()
        .any(|line| numbered_or_bullet(line) || asterisks(line).iter().any(|&(_, format)| format))
}

/// The Markdown espanso is given for a text written in the editor.
pub fn to_markdown(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::with_capacity(text.len() + 16);
    for (i, line) in lines.iter().enumerate() {
        let lone: Vec<usize> = asterisks(line).into_iter().filter(|&(_, f)| !f).map(|(at, _)| at).collect();
        let trimmed = line.trim();
        // A line of only dashes or equals signs would underline the one above into a heading.
        let rule = !trimmed.is_empty() && (trimmed.chars().all(|c| c == '-') || trimmed.chars().all(|c| c == '='));
        // A number of three digits or more opening a line, «2026. Año», is not a list.
        let year = list_item(line) && !numbered_or_bullet(line);
        let mut first = true;
        for (at, c) in line.char_indices() {
            let escape = ALWAYS_ESCAPED.contains(&c)
                || lone.contains(&at)
                || (rule && first && !c.is_whitespace())
                || (year && c == '.' && line[..at].trim_start().chars().all(|d| d.is_ascii_digit()));
            if escape {
                out.push('\\');
            }
            if !c.is_whitespace() {
                first = false;
            }
            out.push(c);
        }
        if let Some(next) = lines.get(i + 1) {
            if !line.trim().is_empty() && !next.trim().is_empty() && !numbered_or_bullet(next) {
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
            if c != '\\' {
                text.push(c);
                continue;
            }
            match chars.next() {
                Some(escaped) if escaped.is_ascii_punctuation() => text.push(escaped),
                None => {} // the hard break
                Some(other) => {
                    text.push(c);
                    text.push(other);
                }
            }
        }
    }
    (to_markdown(&text) == markdown).then_some(text)
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

/// A list as people write one: a bullet, or a number of one or two digits. A year that opens a
/// line, «2026. Año», is a number Markdown would take as a list too, but nobody means it as one.
fn numbered_or_bullet(line: &str) -> bool {
    let digits = line.trim_start().chars().take_while(char::is_ascii_digit).count();
    list_item(line) && digits <= 2
}

/// Every asterisk in the line but a list's bullet, by byte position, and whether it is format:
/// one of a pair, `*like this*` or `**like this**`, the opening run followed by a letter and the
/// closing one, as long as the opening, preceded by one. The rest, `*8374` or `5 * 3`, are
/// asterisks.
fn asterisks(line: &str) -> Vec<(usize, bool)> {
    let indent = line.len() - line.trim_start().len();
    let skip = if line[indent..].starts_with("* ") { Some(indent) } else { None };
    // Runs of asterisks: where they start, how many, and what is on either side.
    let mut runs: Vec<(usize, usize, bool, bool)> = Vec::new();
    let mut chars = line.char_indices().peekable();
    let mut before: Option<char> = None;
    while let Some((at, c)) = chars.next() {
        if c != '*' || Some(at) == skip {
            before = Some(c);
            continue;
        }
        let mut len = 1;
        while chars.peek().is_some_and(|&(_, c)| c == '*') {
            chars.next();
            len += 1;
        }
        let after = chars.peek().map(|&(_, c)| c);
        let opens = after.is_some_and(|c| !c.is_whitespace());
        let closes = before.is_some_and(|c| !c.is_whitespace());
        runs.push((at, len, opens, closes));
        before = Some('*');
    }
    let mut paired = vec![false; runs.len()];
    let mut open: Vec<usize> = Vec::new();
    for (i, &(_, len, opens, closes)) in runs.iter().enumerate() {
        if len > 2 {
            continue;
        }
        if closes {
            if let Some(k) = open.iter().rposition(|&o| runs[o].1 == len) {
                paired[open[k]] = true;
                paired[i] = true;
                open.truncate(k);
                continue;
            }
        }
        if opens {
            open.push(i);
        }
    }
    let mut out = Vec::new();
    for (i, &(at, len, _, _)) in runs.iter().enumerate() {
        out.extend((0..len).map(|n| (at + n, paired[i])));
    }
    out
}

/// The editor's text for HTML put on the clipboard by an app that copies format: bold and
/// italics become `**` and `*`, lists `- ` and `1. `, line breaks stay, and everything else
/// (fonts, colours, links, tables) leaves only its words. Images keep a short `alt`, which is how
/// Teams copies an emoji.
pub fn from_html(html: &str) -> String {
    let mut w = HtmlText::default();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            w.text(rest);
            break;
        };
        w.text(&rest[..lt]);
        rest = &rest[lt..];
        if let Some(body) = rest.strip_prefix("<!--") {
            rest = body.find("-->").map_or("", |end| &body[end + 3..]);
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        w.tag(&rest[1..gt]);
        rest = &rest[gt + 1..];
    }
    w.finish()
}

#[derive(Default)]
struct HtmlText {
    out: String,
    /// Open elements: name, and the mark it opened that its closing tag has to close.
    open: Vec<(String, &'static str)>,
    bold: usize,
    italic: usize,
    /// One counter per open list; `None` for a bulleted one.
    lists: Vec<Option<u32>>,
    /// A list item's mark, written with its first word.
    prefix: Option<String>,
    space: bool,
    /// Inside `<style>`, `<script>`, `<head>`, or Word's own bullet (`mso-list:Ignore`), whose
    /// text is collected to tell a numbered list from a bulleted one.
    hidden: usize,
    word_bullet: String,
    /// A Word paragraph that is a list item: its level, waiting for its bullet's text.
    word_item: Option<usize>,
}

impl HtmlText {
    fn text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        let decoded = decode_entities(raw);
        if self.hidden > 0 {
            self.word_bullet.push_str(&decoded);
            return;
        }
        for c in decoded.chars() {
            // `&nbsp;` is a space the page meant to keep, but a space all the same.
            if c.is_whitespace() || c == '\u{a0}' {
                self.space = true;
                continue;
            }
            self.word_start();
            self.out.push(c);
        }
    }

    /// Before a word or an opening mark: the list item's mark, and the space owed since the last.
    fn word_start(&mut self) {
        if let Some(level) = self.word_item.take() {
            let bullet = self.word_bullet.trim().to_string();
            self.word_bullet.clear();
            let numbered = bullet.trim_end_matches(['.', ')']).chars().all(|c| c.is_ascii_digit()) && !bullet.is_empty();
            let mark = if numbered { format!("{}. ", bullet.trim_end_matches(['.', ')'])) } else { "- ".to_string() };
            self.prefix = Some(format!("{}{mark}", "   ".repeat(level.saturating_sub(1))));
        }
        if let Some(prefix) = self.prefix.take() {
            self.out.push_str(&prefix);
            self.space = false;
        }
        if self.space && !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push(' ');
        }
        self.space = false;
    }

    fn newline(&mut self) {
        self.space = false;
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    fn tag(&mut self, inside: &str) {
        let inside = inside.trim();
        // Word's conditional comments, `<![if !supportLists]>`, and doctype.
        if inside.starts_with('!') || inside.starts_with('?') {
            return;
        }
        let closing = inside.starts_with('/');
        let inside = inside.trim_start_matches('/');
        let name_end = inside.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(inside.len());
        let name = inside[..name_end].to_ascii_lowercase();
        let attrs = &inside[name_end..];
        if closing {
            self.close(&name);
        } else {
            let self_closing = inside.ends_with('/') || matches!(name.as_str(), "br" | "img" | "hr" | "meta" | "link" | "input");
            self.open_tag(&name, attrs, self_closing);
        }
    }

    fn open_tag(&mut self, name: &str, attrs: &str, self_closing: bool) {
        let style: String = attr(attrs, "style").unwrap_or_default().to_ascii_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
        // One property by its exact name: Word's `mso-bidi-font-weight:normal` is not `font-weight`.
        let property = |wanted: &str| {
            style.split(';').find_map(|declaration| declaration.split_once(':').filter(|(name, _)| *name == wanted).map(|(_, value)| value))
        };
        let hides = matches!(name, "style" | "script" | "head" | "title") || property("mso-list") == Some("ignore");
        match name {
            "br" => {
                self.prefix = None;
                self.word_item = None;
                self.out.push('\n');
                self.space = false;
            }
            "img" => {
                if let Some(alt) = attr(attrs, "alt").filter(|a| !a.trim().is_empty() && a.chars().count() <= 4) {
                    self.text(&alt);
                }
            }
            "ul" | "ol" => {
                self.newline();
                self.lists.push((name == "ol").then_some(0));
            }
            "li" => {
                self.newline();
                let depth = self.lists.len().max(1);
                let mark = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        format!("{n}. ")
                    }
                    _ => "- ".to_string(),
                };
                self.prefix = Some(format!("{}{mark}", "   ".repeat(depth - 1)));
            }
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "tr" | "blockquote" | "table" | "section"
            | "article" | "header" | "footer" | "hr" => {
                self.newline();
                if let Some(level) = property("mso-list").and_then(word_level) {
                    self.word_item = Some(level);
                }
            }
            _ => {}
        }
        if self_closing {
            return;
        }
        if hides {
            self.hidden += 1;
        }
        let weight = property("font-weight");
        let bold = match weight {
            Some(w) => matches!(w, "bold" | "bolder" | "600" | "700" | "800" | "900"),
            None => matches!(name, "b" | "strong" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"),
        };
        let italic = matches!(name, "i" | "em") || property("font-style") == Some("italic");
        let mark = if bold && self.bold == 0 && self.italic == 0 {
            "**"
        } else if italic && self.italic == 0 && self.bold == 0 {
            "*"
        } else {
            ""
        };
        if !mark.is_empty() && self.hidden == 0 {
            self.word_start();
            self.out.push_str(mark);
        }
        self.bold += usize::from(mark == "**");
        self.italic += usize::from(mark == "*");
        self.open.push((name.to_string(), if self.hidden == 0 { mark } else { "" }));
        if hides {
            self.open.last_mut().expect("just pushed").0.push('!');
        }
    }

    fn close(&mut self, name: &str) {
        let Some(k) = self.open.iter().rposition(|(n, _)| n.trim_end_matches('!') == name) else { return };
        for (n, mark) in self.open.split_off(k).into_iter().rev() {
            if n.ends_with('!') {
                self.hidden = self.hidden.saturating_sub(1);
            }
            if !mark.is_empty() {
                self.bold -= usize::from(mark == "**");
                self.italic -= usize::from(mark == "*");
                if self.out.ends_with(mark) {
                    // Nothing between the marks: no format, and no stray asterisks.
                    self.out.truncate(self.out.len() - mark.len());
                } else {
                    self.out.push_str(mark);
                }
            }
            match n.trim_end_matches('!') {
                "ul" | "ol" => {
                    self.lists.pop();
                    self.newline();
                }
                "li" | "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "tr" | "blockquote" | "table"
                | "section" | "article" | "header" | "footer" => {
                    // An empty paragraph is a blank line the author left on purpose.
                    if self.out.ends_with('\n') && !self.out.ends_with("\n\n") && self.prefix.is_none() {
                        self.out.push('\n');
                    }
                    self.prefix = None;
                    self.word_item = None;
                    self.newline();
                }
                _ => {}
            }
        }
    }

    fn finish(self) -> String {
        let mut lines: Vec<&str> = self.out.lines().map(str::trim_end).collect();
        // At most one blank line in a row, and none at either end.
        lines.dedup_by(|a, b| a.is_empty() && b.is_empty());
        while lines.first().is_some_and(|l| l.is_empty()) {
            lines.remove(0);
        }
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }
}

/// The level of a Word list paragraph, from its style's `mso-list:l0 level2 lfo1`.
fn word_level(list: &str) -> Option<usize> {
    let level = list.split("level").nth(1)?;
    level.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

/// An attribute's value, quoted with either quote or not at all.
fn attr(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        let starts_word = at == 0 || lower[..at].ends_with(|c: char| c.is_whitespace());
        let rest = attrs[from..].trim_start();
        if !starts_word || !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or(""),
            _ => rest.split(|c: char| c.is_whitespace()).next().unwrap_or(""),
        };
        return Some(decode_entities(value));
    }
    None
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let end = rest.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            let code = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .map(|hex| u32::from_str_radix(hex, 16))
                    .or_else(|| name.strip_prefix('#').map(str::parse))
                    .and_then(Result::ok)
                    .and_then(char::from_u32),
            };
            code.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_is_noticed_and_plain_text_is_left_plain() {
        for rich in ["Hola **Ana**.", "Es *muy* importante.", "Lista:\n- uno\n- dos", "1. Primero", "**Cuenta:** *8374"] {
            assert!(is_rich(rich), "{rich:?}");
        }
        for plain in ["Hola Ana.", "5 * 3 = 15", "a* b", "Precio: 2*", "-sin espacio", "2026. Año", "Tarjeta *8374", "*8374 y *1234", "Mal **cerrado*"] {
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
    fn what_is_not_format_goes_out_as_written() {
        let text = "**Cuenta:** *8374, mi_correo #1 & [nota]\n2026. Año\n---";
        let md = to_markdown(text);
        assert_eq!(md, "**Cuenta:** \\*8374, mi\\_correo \\#1 \\& \\[nota\\]\\\n2026\\. Año\\\n\\---");
        assert_eq!(from_markdown(&md).as_deref(), Some(text));
    }

    #[test]
    fn markdown_written_another_way_is_not_taken_as_ours() {
        assert_eq!(from_markdown("uno\ndos"), None);
        assert_eq!(from_markdown("\\*literal\\*").as_deref(), None);
        assert_eq!(from_markdown("**ya**").as_deref(), Some("**ya**"));
    }

    #[test]
    fn a_formatted_paste_arrives_as_marks() {
        let teams = "<p>Hola <strong>Ana</strong>, <em>gracias</em>&nbsp;<img alt=\"😀\" src=\"x\"></p>\
                     <ul><li>uno</li><li><b>dos </b>y tres</li></ul><p>Fin &amp; adiós</p>";
        assert_eq!(from_html(teams), "Hola **Ana**, *gracias* 😀\n- uno\n- **dos** y tres\nFin & adiós");
        let numbered = "<div>Pasos:</div><ol><li>abrir</li><li>cerrar</li></ol><div><br></div><div>Listo</div>";
        assert_eq!(from_html(numbered), "Pasos:\n1. abrir\n2. cerrar\n\nListo");
        let docs = "<b style=\"font-weight:normal\" id=\"docs\"><span style=\"font-weight:700\">Sí</span> no</b>";
        assert_eq!(from_html(docs), "**Sí** no");
        let word = "<p class=MsoListParagraphCxSpFirst style='mso-list:l0 level1 lfo1'><![if !supportLists]>\
                    <span style='font-family:Symbol;mso-list:Ignore'>·<span>&nbsp;&nbsp; </span></span><![endif]>Uno</p>\
                    <p class=MsoListParagraphCxSpLast style='mso-list:l0 level2 lfo1'><![if !supportLists]>\
                    <span style='mso-list:Ignore'>1.<span>&nbsp; </span></span><![endif]>Dos</p>";
        assert_eq!(from_html(word), "- Uno\n   1. Dos");
        let word_bold = "<p class=MsoNormal>Hola <b style='mso-bidi-font-weight:normal'>Ana</b>,\r\ngracias</p>";
        assert_eq!(from_html(word_bold), "Hola **Ana**, gracias");
    }
}
