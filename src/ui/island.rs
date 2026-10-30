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

//! The island: what Khipu says about what was just done, said where the eye already is.
//!
//! Every message posted while the list or Ajustes is on screen is said here, never in a box at the
//! top of the window or at the foot of a page that has to be scrolled to. On the list the round
//! «+» becomes it; over Ajustes, and over an expansion opened in place of the list, it rises from
//! nothing at the foot. A message that needs nothing from anyone runs out by itself along a line of
//! time at its foot; one that can be taken back offers «Deshacer»; an error stays, with its cross,
//! until it is closed. The pointer on it stops its time.
//!
//! What it says, and for how long, is `AppState`'s ([`AppState::note`]): nothing here decides what
//! is said or keeps it. Elsewhere — the editor, Consejos — the banner at the top says it instead.
//!
//! Every frame is worked out from springs and the clock, so a new message, Deshacer, the cross or
//! the pointer can catch it anywhere along the way.

use crate::app::{accent, AppState, BannerKind};
use crate::ui::list_view::{lift, NEW_BUTTON, PLUS_ARM, PLUS_STROKE};

// The choreography. The «+» does not make way for a message: it becomes one. Its two bars fold
// into the check (or the «!» of an error) and slide to the left end while the circle stretches into
// a pill on springs, the accent melting into the surface; the mark lands with a small beat and a
// ring; then the words arrive a letter at a time, the hairline grows, «Deshacer» or the cross comes
// last, and the line of time draws itself in before it starts to run out. Rising from nothing, it
// comes up as a round mark first and stretches once it is nearly there. Leaving is the same played
// backwards.

/// The island's height: a little less than the button's, so it reads as resting where it was.
const ISLAND_HEIGHT: f32 = 44.0;
/// From the island's left end to the middle of its mark, and from there to the words.
const ISLAND_ICON: f32 = 24.0;
const ISLAND_AFTER_ICON: f32 = 18.0;
/// Round the hairline between the words and «Deshacer», and how tall it is.
const ISLAND_BEFORE_RULE: f32 = 10.0;
const ISLAND_AFTER_RULE: f32 = 4.0;
const ISLAND_RULE: f32 = 18.0;
/// «Deshacer»'s own rounded patch: from its sides to the word, its height, and from it to the
/// island's right end. The cross's patch is round, as tall as this one.
const CHIP_PAD: f32 = 12.0;
const CHIP_HEIGHT: f32 = 30.0;
const ISLAND_END: f32 = 6.0;
/// Half the cross's arms.
const CROSS: f32 = 4.5;
/// The right end when there is nothing after the words.
const ISLAND_PAD: f32 = 18.0;
/// The most lines a message takes before it ends in «…»: an error's reason is worth reading whole.
const MAX_ROWS: usize = 4;

/// The choreography, in seconds. The words wait until the pill is well on its way.
const WORDS_DELAY: f64 = 0.15;
/// Rising from nothing the pill has further to come, so the words wait longer.
const RISE_WORDS_DELAY: f64 = 0.28;
/// How far below its place it starts when it rises from nothing, and how far along the rise it
/// begins to stretch.
const RISE_DROP: f32 = 14.0;
const RISE_STRETCH: f32 = 0.55;
/// One letter to the next, and how long one takes to arrive; leaving is quicker, from the end.
const LETTER_STAGGER: f64 = 0.013;
const LETTER_IN: f64 = 0.24;
const LETTER_OUT: f64 = 0.15;
const LETTER_OUT_STAGGER: f64 = 0.006;
/// However long the message, its letters are all on their way out within this, as «Expansión
/// eliminada · Deshacer» is: a long error must not still be leaving when the pill closes in.
const LETTER_OUT_SPAN: f64 = 0.17;

/// One letter to the next on the way out, for a message of `n` letters.
fn out_stagger(n: usize) -> f64 {
    LETTER_OUT_STAGGER.min(LETTER_OUT_SPAN / n.max(1) as f64)
}
/// How far a letter rises as it arrives, and as it leaves.
const LETTER_RISE: f32 = 5.0;
/// After the words, the hairline, then «Deshacer» or the cross.
const RULE_AFTER: f64 = 0.02;
const RULE_GROW: f64 = 0.3;
const ACTION_AFTER: f64 = 0.06;
/// Deshacer pressed: the old words go, these come in, and they stay this long before it closes.
const SWAP_DELAY: f64 = 0.17;
const UNDONE_STAY: f64 = 1.5;
/// From the words leaving to the pill closing.
const CLOSE_DELAY: f64 = 0.15;
/// The line of time draws itself in this long after the words begin, and fades when done.
const BAR_DELAY: f64 = 0.37;
const BAR_IN: f64 = 0.36;
const BAR_OUT: f64 = 0.2;
/// The mark landing: its beat, and the ring it lets go.
const POP: f64 = 0.24;
const POP_SIZE: f32 = 0.16;
const RIPPLE: f64 = 0.52;

/// Where the island comes from, and so where it goes back to.
#[derive(Clone, Copy, PartialEq)]
pub enum Source {
    /// The list's round «+», drawn where this rect is: it becomes the island, and the island it.
    Plus(egui::Rect),
    /// Nothing, with its foot here: the window's own messages where the «+» is not drawn.
    Window(egui::Pos2),
    /// Nothing, with its foot here, over Ajustes: what Ajustes' own buttons did.
    Settings(egui::Pos2),
}

impl Source {
    fn foot(self) -> egui::Pos2 {
        match self {
            Source::Plus(circle) => circle.center_bottom(),
            Source::Window(foot) | Source::Settings(foot) => foot,
        }
    }

    fn rises(self) -> bool {
        !matches!(self, Source::Plus(_))
    }

    fn settings(self) -> bool {
        matches!(self, Source::Settings(_))
    }

    fn id(self) -> egui::Id {
        egui::Id::new(if self.settings() { "settings-island" } else { "island" })
    }
}

/// Lets go of the island over Ajustes, so the next time the dialog opens it starts afresh.
pub fn forget_settings(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Island>(Source::Settings(egui::Pos2::ZERO).id()));
}

/// What the island says: the message as it was posted, without its closing stop, which would sit
/// oddly in front of «Deshacer».
fn island_words(message: &str) -> String {
    message.trim_end_matches(['.', '।']).to_string()
}

/// A spring: where it is, how fast it moves, where it is going. Stiffness and damping are its
/// character; the ones here are a little under-damped, so they pass their mark and settle.
#[derive(Clone, Copy)]
struct Spring {
    x: f32,
    v: f32,
    to: f32,
    k: f32,
    c: f32,
}

impl Spring {
    const fn new(x: f32, k: f32, c: f32) -> Self {
        Self { x, v: 0.0, to: x, k, c }
    }

    /// Four small steps a frame keep a stiff spring steady at any frame rate.
    fn step(&mut self, dt: f32) {
        let h = dt / 4.0;
        for _ in 0..4 {
            let a = -self.k * (self.x - self.to) - self.c * self.v;
            self.v += a * h;
            self.x += self.v * h;
        }
    }

    fn snap(&mut self) {
        self.x = self.to;
        self.v = 0.0;
    }
}

/// What follows the words.
#[derive(Clone, Copy, PartialEq)]
enum Act {
    /// Nothing: the message runs out by itself.
    Quiet,
    /// «Deshacer».
    Undo,
    /// The cross, on an error, which stays until it is used.
    Close,
}

/// One line of the island: what it says, what follows it, when it began to arrive and, once it is
/// on its way out, when it began to leave.
#[derive(Clone)]
struct Words {
    text: String,
    act: Act,
    at: f64,
    out: Option<f64>,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Saying something.
    Open,
    /// Deshacer was pressed: saying so for a moment.
    Undone,
    /// On its way back to the «+», or to nothing.
    Closing,
}

#[derive(Clone)]
struct Island {
    phase: Phase,
    /// When the phase began.
    since: f64,
    /// Which message it is showing.
    key: u64,
    words: Words,
    leaving: Option<Words>,
    /// The share of time left, as last seen while there was a message, and whether it runs out.
    left: f32,
    timed: bool,
    width: Spring,
    height: Spring,
    /// 0 is the plus, 1 the mark.
    morph: Spring,
    /// 0 is the check, 1 the «!».
    bang: Spring,
    /// Where the mark's middle is, and where the words begin, from the foot's middle.
    icon: Spring,
    anchor: Spring,
    placed: bool,
    /// 0 is the accent, 1 the surface.
    tone: Spring,
    /// 0 is nothing, below its place; 1 is there. Always 1 for the «+».
    appear: Spring,
    /// The pointer on it: a little higher, a little more shadow.
    lift: Spring,
    /// The pointer on «Deshacer» or the cross, and the patch sinking as it is pressed.
    hover: Spring,
    press: Spring,
    /// Deshacer turns the check once round backwards, as a tape rewinds.
    spin: Spring,
    bar_at: f64,
    bar_out: Option<f64>,
    landed: Option<f64>,
}

impl Island {
    fn new(now: f64, key: u64, words: Words, source: Source, error: bool) -> Self {
        let (start, there) = if source.rises() { (ISLAND_HEIGHT, 1.0) } else { (NEW_BUTTON, 0.0) };
        Self {
            phase: Phase::Open,
            since: now,
            key,
            bar_at: words.at + BAR_DELAY,
            words,
            leaving: None,
            left: 1.0,
            timed: false,
            width: Spring::new(start, 420.0, 30.0),
            height: Spring::new(start, 420.0, 38.0),
            morph: Spring::new(there, 240.0, 23.0),
            bang: Spring::new(if error { 1.0 } else { 0.0 }, 300.0, 31.0),
            icon: Spring::new(0.0, 300.0, 31.0),
            anchor: Spring::new(0.0, 300.0, 31.0),
            placed: false,
            tone: Spring::new(there, 260.0, 32.0),
            appear: Spring::new(1.0 - there, 230.0, 24.0),
            lift: Spring::new(0.0, 300.0, 35.0),
            hover: Spring::new(0.0, 500.0, 45.0),
            press: Spring::new(0.0, 700.0, 40.0),
            spin: Spring::new(0.0, 130.0, 19.0),
            bar_out: None,
            landed: None,
        }
    }

    /// New words in place of the ones showing, which begin to leave at `out` as these arrive.
    fn say(&mut self, out: f64, words: Words) {
        let mut old = std::mem::replace(&mut self.words, words);
        old.out.get_or_insert(out);
        self.leaving = Some(old);
    }

    fn springs(&mut self) -> [&mut Spring; 12] {
        [
            &mut self.width,
            &mut self.height,
            &mut self.morph,
            &mut self.bang,
            &mut self.icon,
            &mut self.anchor,
            &mut self.tone,
            &mut self.appear,
            &mut self.lift,
            &mut self.hover,
            &mut self.press,
            &mut self.spin,
        ]
    }
}

fn ease_out(p: f64) -> f32 {
    let p = p.clamp(0.0, 1.0) as f32;
    1.0 - (1.0 - p).powi(3)
}

/// How far letter `i` of `n` has arrived (and not yet left), and how far off its line it sits.
/// Words making way for others leave from the first letter, so the new ones follow them in;
/// words leaving with the island go from the last, as its end closes in on them.
fn letter(now: f64, words: &Words, at: f64, i: usize, n: usize, ahead: bool) -> (f32, f32) {
    let came = ease_out((now - at) / LETTER_IN);
    let stayed = words.out.map_or(1.0, |out| {
        let place = if ahead { i } else { n.saturating_sub(i + 1) };
        let out = out + place as f64 * out_stagger(n);
        1.0 - ease_out((now - out) / LETTER_OUT)
    });
    (came * stayed, (1.0 - came) * LETTER_RISE - (1.0 - stayed) * LETTER_RISE)
}

/// A message laid out at full size: the pill grows towards its width and height. A message too
/// long for one line wraps, and the pill grows taller from its foot; past [`MAX_ROWS`] it ends in
/// «…» rather than push the island off the page.
struct Laid {
    line: std::sync::Arc<egui::Galley>,
    action: Option<std::sync::Arc<egui::Galley>>,
    /// The patch after the words, «Deshacer»'s or the cross's, as wide as it is.
    chip: Option<f32>,
    width: f32,
    height: f32,
    /// One line's height.
    row: f32,
}

impl Laid {
    /// The glyphs of the message, and of everything that arrives with it: message, hairline, and
    /// «Deshacer» or the cross.
    fn counts(&self) -> (usize, usize) {
        let glyphs = |g: &egui::Galley| g.rows.iter().map(|r| r.row.glyphs.len()).sum::<usize>();
        let n = glyphs(&self.line);
        let after = match (&self.action, self.chip) {
            (Some(action), _) => 1 + glyphs(action),
            (None, Some(_)) => 2,
            (None, None) => 0,
        };
        (n, n + after)
    }
}

struct Ink {
    text: egui::Color32,
    accent: egui::Color32,
    body: egui::FontId,
    strong: egui::FontId,
}

fn lay(painter: &egui::Painter, words: &Words, undo: &str, room: f32, ink: &Ink) -> Laid {
    let action = (words.act == Act::Undo)
        .then(|| painter.layout_no_wrap(undo.to_owned(), ink.strong.clone(), ink.accent));
    let chip = match words.act {
        Act::Quiet => None,
        Act::Undo => action.as_ref().map(|a| 2.0 * CHIP_PAD + a.size().x),
        Act::Close => Some(CHIP_HEIGHT),
    };
    let fixed = ISLAND_ICON
        + ISLAND_AFTER_ICON
        + chip.map_or(ISLAND_PAD, |chip| ISLAND_BEFORE_RULE + 1.0 + ISLAND_AFTER_RULE + chip + ISLAND_END);
    let mut job = egui::text::LayoutJob::default();
    job.append(&words.text, 0.0, egui::TextFormat::simple(ink.body.clone(), ink.text));
    job.wrap = egui::text::TextWrapping {
        max_width: (room - fixed).max(0.0),
        max_rows: MAX_ROWS,
        ..Default::default()
    };
    let line = painter.layout_job(job);
    let row = line.size().y / line.rows.len().max(1) as f32;
    Laid {
        width: fixed + line.size().x,
        height: ISLAND_HEIGHT + line.size().y - row,
        row,
        line,
        action,
        chip,
    }
}

/// Paints a laid-out message a letter at a time, each as far along as `step` says, every letter
/// on its own line's baseline.
fn paint_letters(
    painter: &egui::Painter,
    galley: &std::sync::Arc<egui::Galley>,
    top_left: egui::Pos2,
    font: &egui::FontId,
    ink: egui::Color32,
    step: impl Fn(usize) -> (f32, f32),
) {
    let mut i = 0;
    for row in &galley.rows {
        let Some(first) = row.row.glyphs.first() else { continue };
        let baseline = top_left.y + row.pos.y + first.pos.y;
        for glyph in &row.row.glyphs {
            let (shown, rise) = step(i);
            i += 1;
            if shown <= 0.003 || glyph.chr.is_whitespace() {
                continue;
            }
            let one = painter.layout_no_wrap(glyph.chr.to_string(), font.clone(), ink);
            let Some(own) = one.rows.first().and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y)) else {
                continue;
            };
            let at = egui::pos2(top_left.x + row.pos.x + glyph.pos.x, baseline - own + rise);
            painter.galley_with_override_text_color(at, one, ink.gamma_multiply(shown));
        }
    }
}

/// The island, while there is one. Returns false when there is nothing to draw — for the «+», that
/// it is to draw itself. `room` is the widest it may grow.
///
/// While it is up the «+» is not there to be pressed, though Ctrl+N still is.
pub fn show(ui: &mut egui::Ui, state: &mut AppState, source: Source, room: f32) -> bool {
    let ctx = ui.ctx().clone();
    let id = source.id();
    let settings = source.settings();
    let now = ctx.input(|i| i.time);
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let motion = crate::theme::animations_enabled();
    let later = |seconds: f64| if motion { now + seconds } else { now - 60.0 };
    let t = state.t();

    // Where the island is going follows from what there is to say.
    let note = state
        .note(settings)
        .map(|n| (island_words(n.message), n.kind == BannerKind::Error, n.undo, n.left, n.key));
    let mut island: Option<Island> = ctx.data(|d| d.get_temp(id));
    if let Some((text, error, undo, left, key)) = note {
        let act = if undo {
            Act::Undo
        } else if error {
            Act::Close
        } else {
            Act::Quiet
        };
        let delay = if source.rises() { RISE_WORDS_DELAY } else { WORDS_DELAY };
        let words = |at| Words { text, act, at, out: None };
        match island.as_mut() {
            None => island = Some(Island::new(now, key, words(later(delay)), source, error)),
            Some(isl) if isl.key != key || isl.phase == Phase::Closing => {
                isl.say(later(0.0), words(later(SWAP_DELAY)));
                isl.key = key;
                isl.phase = Phase::Open;
                isl.since = now;
                isl.bar_at = isl.words.at + BAR_DELAY;
                isl.bar_out = None;
            }
            Some(_) => {}
        }
        if let Some(isl) = island.as_mut() {
            isl.left = left.unwrap_or(1.0);
            isl.timed = left.is_some();
            isl.bang.to = if error { 1.0 } else { 0.0 };
        }
    } else if let Some(isl) = island.as_mut() {
        let done = match isl.phase {
            Phase::Open => true,
            Phase::Undone => now - isl.since >= UNDONE_STAY,
            Phase::Closing => false,
        };
        if done {
            isl.phase = Phase::Closing;
            isl.since = now;
            isl.words.out.get_or_insert(later(0.0));
            isl.bar_out.get_or_insert(later(0.0));
        }
    }
    let Some(mut isl) = island else { return false };

    let is_light = !ui.visuals().dark_mode;
    let (fill, plus_ink) = crate::app::accent_fill(ui.visuals());
    let contrast = crate::app::contrast().is_some();
    let surface = crate::app::win_control_for(is_light);
    let (good, bad) = if contrast {
        (ui.visuals().strong_text_color(), ui.visuals().strong_text_color())
    } else {
        (crate::app::success(is_light), crate::app::danger(is_light))
    };
    let body = egui::TextStyle::Body.resolve(ui.style());
    let ink = Ink {
        text: ui.visuals().strong_text_color(),
        accent: accent(ui.visuals()),
        strong: egui::FontId::new(body.size, crate::fonts::semibold_family()),
        body,
    };
    let laid = lay(ui.painter(), &isl.words, t.undo, room, &ink);
    let old = isl.leaving.as_ref().map(|w| lay(ui.painter(), w, t.undo, room, &ink));

    // Where each spring is headed. Closing waits for the words to be gone.
    let open = isl.phase != Phase::Closing || now - isl.since < if motion { CLOSE_DELAY } else { 0.0 };
    if !isl.placed {
        isl.anchor = Spring { x: -laid.width * 0.5, to: -laid.width * 0.5, ..isl.anchor };
        isl.placed = true;
    }
    // Words on their way out keep their room until the last letter is gone, so the pill never
    // closes in on them.
    let held = old.as_ref().zip(isl.leaving.as_ref()).filter(|(old, words)| {
        words.out.is_some_and(|out| now < out + old.counts().0 as f64 * out_stagger(old.counts().0) + LETTER_OUT)
    });
    let reach = laid.width.max(held.map_or(0.0, |(old, _)| old.width));
    let tall = laid.height.max(held.map_or(0.0, |(old, _)| old.height));
    let rest = if source.rises() { ISLAND_HEIGHT } else { NEW_BUTTON };
    if open {
        // Rising from nothing, it is a round mark until it is nearly in its place.
        let stretch = !source.rises() || isl.appear.x > RISE_STRETCH;
        isl.width.to = if stretch { reach } else { ISLAND_HEIGHT };
        isl.height.to = if stretch { tall } else { ISLAND_HEIGHT };
        isl.morph.to = 1.0;
        isl.tone.to = 1.0;
        isl.appear.to = 1.0;
        isl.anchor.to = -reach * 0.5;
        isl.icon.to = isl.anchor.to + ISLAND_ICON;
    } else {
        isl.width.to = rest;
        isl.height.to = rest;
        isl.icon.to = 0.0;
        if source.rises() {
            isl.appear.to = 0.0;
        } else {
            isl.morph.to = 0.0;
            isl.tone.to = 0.0;
            isl.appear.to = 1.0;
        }
    }
    for spring in isl.springs() {
        if motion {
            spring.step(dt);
        } else {
            spring.snap();
        }
    }
    let gone = if source.rises() {
        isl.appear.x < 0.02
    } else {
        (isl.width.x - NEW_BUTTON).abs() < 0.5 && isl.morph.x.abs() < 0.01 && isl.tone.x < 0.01
    };
    if isl.phase == Phase::Closing && !open && gone {
        ctx.data_mut(|d| d.remove::<Island>(id));
        return false;
    }
    if (isl.morph.to == 0.0 && isl.morph.x < 0.5) || (isl.appear.to == 0.0 && isl.appear.x < 0.5) {
        isl.landed = None;
    }
    if motion && isl.morph.to == 1.0 && isl.landed.is_none() && isl.morph.x > 0.97 && isl.appear.x > 0.9 {
        isl.landed = Some(now);
    }

    // The pill, stretched by its springs. Growing fast it gives a little in height, as something
    // soft would, and the pointer on it raises it. Rising from nothing, it comes up from a little
    // below its place as it fades in.
    let squash = (isl.width.v * 0.004).clamp(0.0, 3.0);
    let size = egui::vec2(isl.width.x, isl.height.x - squash);
    let foot = source.foot();
    let drop = (1.0 - isl.appear.x) * RISE_DROP;
    let center_x = foot.x;
    let rect = egui::Rect::from_min_size(
        egui::pos2(center_x - size.x * 0.5, foot.y - size.y - 1.5 * isl.lift.x + drop),
        size,
    );
    let radius = size.x.min(size.y).min(NEW_BUTTON) * 0.5;
    // The mark, the hairline and what follows the words sit on the first line: the middle of the
    // pill while it is one line tall, of its top line once the words wrap.
    let first_line = ISLAND_HEIGHT * 0.5 + (NEW_BUTTON - ISLAND_HEIGHT) * 0.5 * (1.0 - isl.morph.x.clamp(0.0, 1.0));
    let middle = rect.top() + (size.y * 0.5).min(first_line);
    // The words never stand outside the pill: when it narrows faster than they move, its end
    // carries them.
    let left_x = (center_x + isl.anchor.x).max(rect.left());
    let text_x = left_x + ISLAND_ICON + ISLAND_AFTER_ICON;
    let rule_x = text_x + laid.line.size().x + ISLAND_BEFORE_RULE;
    let chip = laid.chip.map(|width| {
        egui::Rect::from_min_size(
            egui::pos2(rule_x + 1.0 + ISLAND_AFTER_RULE, middle - CHIP_HEIGHT * 0.5),
            egui::vec2(width, CHIP_HEIGHT),
        )
    });

    // Being pointed at and pressed.
    let live = isl.phase == Phase::Open && isl.morph.x > 0.5 && isl.appear.x > 0.5;
    let held = ui.interact(rect, id.with("body"), egui::Sense::hover());
    held.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &isl.words.text));
    let act = isl.words.act;
    let action = chip.map(|chip| {
        let sense = if live { egui::Sense::click() } else { egui::Sense::hover() };
        let action = ui.interact(chip.intersect(rect), id.with("action"), sense);
        let label = if act == Act::Undo { t.undo } else { t.banner_close_tip };
        action.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, live, label));
        action
    });
    let on_action = live && action.as_ref().is_some_and(|a| a.hovered());
    let pointed = isl.phase == Phase::Open && (held.hovered() || on_action);
    if pointed {
        state.hold_note(settings, std::time::Duration::from_secs_f32(dt));
    }
    isl.lift.to = if pointed { 1.0 } else { 0.0 };
    isl.hover.to = if on_action { 1.0 } else { 0.0 };
    if on_action {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        if ui.input(|i| i.pointer.any_pressed()) {
            isl.press.v = -12.0;
        }
    }

    // The pill: its shadow, the accent melting into the surface, and an edge as it does. Its own
    // layer's painter, so the shadow is never cut at the edge of the dialog it floats over.
    let tone = isl.tone.x.clamp(0.0, 1.0);
    let mut painter = ctx.layer_painter(ui.layer_id()).with_clip_rect(ctx.content_rect());
    // Solid before it stretches: a pill that widens while still see-through shows the row behind it.
    painter.set_opacity((isl.appear.x / RISE_STRETCH).clamp(0.0, 1.0));
    lift(&painter, rect, is_light);
    if isl.lift.x > 0.01 && !contrast {
        let alpha = if is_light { 18.0 } else { 40.0 } * isl.lift.x;
        let shadow = egui::Shadow { offset: [0, 10], blur: 28, spread: 0, color: egui::Color32::from_black_alpha(alpha as u8) };
        painter.add(shadow.as_shape(rect, egui::CornerRadius::same(radius as u8)));
    }
    painter.rect_filled(rect, radius, fill.lerp_to_gamma(surface, tone));
    let edge = if contrast {
        ink.text
    } else if is_light {
        egui::Color32::from_black_alpha((15.0 * tone) as u8)
    } else {
        egui::Color32::from_white_alpha((18.0 * tone) as u8)
    };
    painter.rect_stroke(rect, radius, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);

    let inside = painter.with_clip_rect(rect.intersect(ctx.content_rect()));
    // The patch after the words under the pointer, sinking a hair as it is pressed.
    let (n, total) = laid.counts();
    let action_at = isl.words.at + n as f64 * LETTER_STAGGER + ACTION_AFTER;
    if let Some(chip) = chip {
        let (shown, _) = letter(now, &isl.words, action_at, total - 1, total, false);
        let patch = isl.hover.x * shown;
        if patch > 0.01 {
            let pressed = egui::Rect::from_center_size(chip.center(), chip.size() * (1.0 + 0.04 * isl.press.x));
            let wash = if is_light { 0.06 } else { 0.08 };
            inside.rect_filled(pressed, CHIP_HEIGHT * 0.5, ink.text.gamma_multiply(wash * patch));
        }
    }

    // The words: those leaving, then these, a letter at a time.
    let line_of = |laid: &Laid, words: &Words, x: f32, ahead: bool| {
        let top = egui::pos2(x, middle - laid.row * 0.5);
        let (_, total) = laid.counts();
        paint_letters(
            &inside,
            &laid.line,
            top,
            &ink.body,
            ink.text,
            |i| letter(now, words, words.at + i as f64 * LETTER_STAGGER, i, total, ahead),
        );
    };
    if let (Some(old), Some(words)) = (&old, &isl.leaving) {
        line_of(old, words, text_x, true);
    }
    line_of(&laid, &isl.words, text_x, false);
    if let Some(chip) = chip {
        let rule_at = isl.words.at + n as f64 * LETTER_STAGGER + RULE_AFTER;
        let (shown, _) = letter(now, &isl.words, rule_at, n, total, false);
        let rule = ISLAND_RULE * ease_out((now - rule_at) / RULE_GROW) * shown;
        if rule > 0.1 {
            inside.vline(
                rule_x + 0.5,
                egui::Rangef::new(middle - rule * 0.5, middle + rule * 0.5),
                egui::Stroke::new(1.0, ink.text.gamma_multiply(0.2 * shown)),
            );
        }
        if let Some(action) = &laid.action {
            let top = egui::pos2(chip.left() + CHIP_PAD, middle - action.size().y * 0.5);
            paint_letters(
                &inside,
                action,
                top,
                &ink.strong,
                ink.accent,
                |j| letter(now, &isl.words, action_at + j as f64 * LETTER_STAGGER, n + 1 + j, total, false),
            );
        } else {
            // The cross arrives as a letter would, rising into its place.
            let (shown, rise) = letter(now, &isl.words, action_at, total - 1, total, false);
            if shown > 0.003 {
                let at = chip.center() + egui::vec2(0.0, rise);
                let stroke = egui::Stroke::new(1.5, ink.text.gamma_multiply(shown));
                inside.line_segment([at + egui::vec2(-CROSS, -CROSS), at + egui::vec2(CROSS, CROSS)], stroke);
                inside.line_segment([at + egui::vec2(-CROSS, CROSS), at + egui::vec2(CROSS, -CROSS)], stroke);
            }
        }
    }
    if isl.leaving.as_ref().is_some_and(|w| w.out.is_some_and(|out| now - out > 1.0)) {
        isl.leaving = None;
    }

    // The plus folding into the mark: the upright bar turns into the check's long stroke, or the
    // «!»'s stem, and the level one shortens into the short stroke, or the point, so neither shrinks
    // to nothing on the way; the ink turning from the accent's to the green of done or the red of
    // an error. Lands with a beat and a ring. It never trails past the round end as the pill closes
    // on it: the end carries it home.
    let own = center_x + isl.icon.x;
    let own = if open { own.max(left_x + ISLAND_ICON) } else { own };
    let icon = egui::pos2(own.clamp(rect.left() + radius, (rect.right() - radius).max(rect.left() + radius)), middle);
    let m = isl.morph.x.clamp(-0.15, 1.15);
    let b = isl.bang.x.clamp(0.0, 1.0);
    let mark = good.lerp_to_gamma(bad, b);
    let landed = isl.landed.map_or(0.0, |at| (now - at) / POP);
    let beat = 1.0 + POP_SIZE * (std::f32::consts::PI * landed.clamp(0.0, 1.0) as f32).sin();
    if let Some(at) = isl.landed {
        let ring = ease_out((now - at) / RIPPLE);
        if ring < 1.0 {
            inside.circle_stroke(icon, 8.0 + 7.0 * ring, egui::Stroke::new(1.5, mark.gamma_multiply(0.45 * (1.0 - ring))));
        }
    }
    let arm = PLUS_ARM * 0.5 - PLUS_STROKE * 0.5;
    let toward = |check: [f32; 4], bang: [f32; 4]| std::array::from_fn::<f32, 4, _>(|k| check[k] + (bang[k] - check[k]) * b);
    let bars = [
        ([0.0, -arm, 0.0, arm], toward([5.6, -4.0, -2.0, 3.6], [0.0, -6.0, 0.0, 1.6])),
        ([-arm, 0.0, arm, 0.0], toward([-5.6, 0.0, -2.0, 3.6], [0.0, 5.4, 0.0, 5.4])),
    ];
    let turn = egui::emath::Rot2::from_angle(isl.spin.x);
    let width = PLUS_STROKE + (1.8 + 0.4 * b - PLUS_STROKE) * m.clamp(0.0, 1.0);
    // Rising from nothing, the mark grows out of its middle as the pill comes up.
    let grown = if source.rises() { isl.appear.x.clamp(0.0, 1.0) } else { 1.0 };
    // The ink changes over where the fill crosses from the accent's darkness to the surface's, so
    // it never sits dim on a ground of its own weight.
    let swap = ((tone - 0.4) / 0.4).clamp(0.0, 1.0);
    let color = plus_ink.lerp_to_gamma(mark, swap * swap * (3.0 - 2.0 * swap));
    for (k, (plus, end)) in bars.into_iter().enumerate() {
        let at = |j: usize| {
            let p = egui::vec2(plus[j] + (end[j] - plus[j]) * m, plus[j + 1] + (end[j + 1] - plus[j + 1]) * m);
            icon + turn * p * beat * grown
        };
        let (a, z) = (at(0), at(2));
        // The «!»'s point is a little rounder than a stroke's end.
        let cap = width * 0.5 * grown * if k == 1 { 1.0 + 0.35 * b * m.clamp(0.0, 1.0) } else { 1.0 };
        inside.line_segment([a, z], egui::Stroke::new(width * grown, color));
        inside.circle_filled(a, cap, color);
        inside.circle_filled(z, cap, color);
    }

    // The time it has left, drawn in from the left along its foot and running back out.
    let grow = ease_out((now - isl.bar_at) / BAR_IN);
    let fade = isl.bar_out.map_or(1.0, |at| 1.0 - ease_out((now - at) / BAR_OUT));
    let span = (rect.width() - 2.0 * radius) * grow * isl.left;
    if isl.timed && span > 0.5 && fade > 0.0 {
        inside.rect_filled(
            egui::Rect::from_min_size(egui::pos2(rect.left() + radius, rect.bottom() - 4.5), egui::vec2(span, 2.0)),
            1.0,
            ink.accent.gamma_multiply((0.8 + 0.2 * isl.lift.x) * fade),
        );
    }

    if action.as_ref().is_some_and(|a| a.has_focus()) {
        painter.rect_stroke(
            rect.expand(3.0),
            radius + 3.0,
            egui::Stroke::new(2.0, ui.visuals().strong_text_color()),
            egui::StrokeKind::Inside,
        );
    }

    if live && action.as_ref().is_some_and(|a| a.clicked()) {
        match act {
            // Deshacer: the list goes back, the words say so, and the check turns once backwards.
            Act::Undo => {
                state.undo_from_banner();
                isl.say(later(0.0), Words { text: t.undone.to_owned(), act: Act::Quiet, at: later(SWAP_DELAY), out: None });
                isl.phase = Phase::Undone;
                isl.since = now;
                isl.bar_out = Some(later(0.0));
                isl.spin.to -= std::f32::consts::TAU;
                isl.press.v = -9.0;
            }
            // The cross: the message is let go, and the island leaves with it next frame.
            Act::Close => {
                state.close_note(settings);
                isl.press.v = -9.0;
            }
            Act::Quiet => {}
        }
    }

    ctx.data_mut(|d| d.insert_temp(id, isl));
    ctx.request_repaint();
    true
}
