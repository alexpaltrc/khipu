/*
 * This file is part of Espanso Manager.
 *
 * Copyright (C) 2026 Alex Palacios
 *
 * Espanso Manager is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Espanso Manager is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Espanso Manager.  If not, see <https://www.gnu.org/licenses/>.
 */

//! The search window: Alt+Space anywhere in Windows, pick an expansion, and it lands where the
//! cursor was.
//!
//! Espanso's own version of this is what it replaces, and the division of labour is the point.
//! Everything visible here is ours — the type ramp, the accent, the row layout, the way a selected
//! row is lit — while the insertion itself is handed back to espanso through `match exec`. Putting
//! text into Word, Teams, a terminal and an Electron app, with the clipboard restored afterwards,
//! is the hard part of a text expander, and it is already solved.
//!
//! Keyboard first, because that is how this is used: it opens with the field focused, Up and Down
//! move, Enter picks, Esc closes. The mouse works too, but nobody reaching for Alt+Space wants it.
//!
//! ## Why it is a window that is always there, hidden
//!
//! Alt+Space has to work from the tray, with the main window hidden — that is nearly the only way
//! it is ever pressed. And with its window hidden, eframe 0.36.1 runs no egui pass for the main
//! window at all, only `App::logic`, which cannot create a window. So this one is created once, on
//! the very first frame, hidden, and is never let go of; opening it is a viewport command, which
//! logic *can* send. From then on it draws itself, in its own pass, from [`Launcher`] — which is
//! why everything it shows is a copy taken when it opens, and nothing reaches into `AppState`.
//!
//! ## Why the list scrolls instead of stopping at eight
//!
//! The window is a glance, not a page, so it is a fixed eight rows tall. That used to be done by
//! throwing away every result past the eighth, which quietly hid expansions from a search that had
//! genuinely found them. Now every match is kept and the list scrolls through the window — with the
//! arrows, with the wheel, and with the selection dragging the view along behind it, which is how
//! espanso's own bar behaves.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::app::{accent, hairline, secondary_text, text_tertiary};
use crate::i18n::Strings;

/// An upper bound on how many matches are held at once. High enough that no real search ever meets
/// it, low enough that one keystroke can never turn into thousands of rows.
const MAX_RESULTS: usize = 200;

/// How many rows the window is tall. Everything past this is reached by scrolling.
pub const VISIBLE_ROWS: usize = 8;

/// One row's height, and therefore the pitch of the list: rows are drawn with no spacing between
/// them so that this number alone decides how tall the window has to be.
pub const ROW_HEIGHT: f32 = 34.0;

/// Wide enough for a trigger and a useful slice of what it expands to.
pub const WINDOW_WIDTH: f32 = 560.0;

const PAD: f32 = 12.0;
const GAP_ABOVE_LIST: f32 = 8.0;
const GAP_BELOW_LIST: f32 = 6.0;
const GAP_ABOVE_EMPTY: f32 = 10.0;

/// Breathing room at both ends of a row. The right-hand one is what keeps a long preview from
/// running into the window's border instead of ending somewhere deliberate.
const ROW_INSET: f32 = 12.0;

/// Width reserved for the trigger, so every preview in the list starts on the same vertical line —
/// the same column rule the main window follows.
const TRIGGER_COLUMN: f32 = 132.0;

/// Kept clear between the trigger and the preview, so a long trigger ends with an ellipsis rather
/// than butting up against the text beside it.
const COLUMN_GAP: f32 = 8.0;

pub struct Hit {
    pub trigger: String,
    pub preview: String,
}

fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("espanso_manager_search")
}

/// Everything the search window shows and everything it has decided, shared between the main
/// window's logic, which opens it and acts on the pick, and the window's own pass, which draws it.
#[derive(Default)]
struct Launcher {
    open: bool,
    query: String,
    selected: usize,
    /// Asks the list to scroll the lit row back into view on the next frame. Set by the arrows and
    /// by a change to the query, so the selection can never walk off the edge of the window.
    follow: bool,
    /// Set when the window opens, cleared once the keyboard has been asked for. Without this the
    /// window appears but the typing goes to whatever was behind it — which is worse than not
    /// opening at all, because it puts stray characters in the user's document.
    needs_focus: bool,
    /// Set once the window has actually held the keyboard. Losing it after that means the person
    /// clicked somewhere else, which is a dismissal — but the frames *before* it arrives are not.
    had_focus: bool,
    /// A copy of the expansions taken when the window opened.
    entries: Vec<Hit>,
    t: Option<&'static Strings>,
    /// The size last asked of the window, so it is only asked again when it changes.
    size: egui::Vec2,
    /// The window's own scale the last time it drew, which is what egui turns a position in points
    /// into pixels with. Zero until it has drawn once.
    scale: f32,
    /// What was picked, waiting for the main window's logic to collect it.
    chosen: Option<String>,
}

/// The search window, from the main window's side. Cheap to clone: every clone is the same window.
#[derive(Clone, Default)]
pub struct SearchWindow(Arc<Mutex<Launcher>>);

impl SearchWindow {
    fn lock(&self) -> MutexGuard<'_, Launcher> {
        // Everything here runs on the UI thread, so a poisoned lock can only mean a panic that has
        // already ended the program; the data behind it is still the best there is.
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Keeps the window in existence. Called on every pass of the main window, from `logic`:
    /// egui forgets a child window that one of its parent's passes does not mention.
    ///
    /// The builder never changes, on purpose. eframe compares it with the last one and turns any
    /// difference into a command, so a builder that said "hidden" while the window was showing
    /// would hide it again on the next pass. Showing, hiding, moving and resizing are all commands
    /// instead, sent when they are meant.
    pub fn keep_alive(&self, ctx: &egui::Context) {
        let builder = egui::ViewportBuilder::default()
            .with_title("Espanso Manager")
            .with_inner_size(egui::vec2(WINDOW_WIDTH, 120.0))
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_active(false)
            .with_visible(false);
        let me = self.clone();
        ctx.show_viewport_deferred(viewport_id(), builder, move |ui, _class| me.draw(ui));
    }

    pub fn is_open(&self) -> bool {
        self.lock().open
    }

    /// Puts the window up over `over`, the window that had the keyboard, with `entries` to search.
    pub fn open(
        &self,
        ctx: &egui::Context,
        entries: Vec<(String, String)>,
        t: &'static Strings,
        over: windows::Win32::Foundation::HWND,
    ) {
        let mut l = self.lock();
        l.entries = entries
            .into_iter()
            .map(|(trigger, preview)| Hit { trigger, preview })
            .collect();
        l.t = Some(t);
        l.query.clear();
        l.selected = 0;
        l.follow = true;
        l.needs_focus = true;
        l.had_focus = false;
        l.chosen = None;
        l.open = true;
        let size = window_size(ctx, hits(&l.entries, "").len());
        l.size = size;

        // Centred on the screen the person is actually working on, not on the primary one, a
        // little above the middle the way every launcher sits. Worked out in pixels and only then
        // turned into points, because egui converts with the scale of the screen this window is on
        // *now*, which need not be the one it is about to go to.
        let target_scale = window_scale(over);
        let own_scale = if l.scale > 0.0 { l.scale } else { crate::display::system_scale() };
        let area = crate::display::work_area_of_window(over, 1.0)
            .or_else(|| crate::display::work_area(1.0));
        let id = viewport_id();
        ctx.send_viewport_cmd_to(id, egui::ViewportCommand::InnerSize(size));
        if let Some(area) = area {
            let x = area.left + (area.width - size.x * target_scale) * 0.5;
            let y = area.top + (area.height - size.y * target_scale) * 0.35;
            ctx.send_viewport_cmd_to(
                id,
                egui::ViewportCommand::OuterPosition(egui::pos2(x / own_scale, y / own_scale)),
            );
        }
        ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
        ctx.request_repaint_of(id);
    }

    /// Takes the window down without picking anything.
    pub fn close(&self, ctx: &egui::Context) {
        let mut l = self.lock();
        if l.open {
            l.open = false;
            l.query.clear();
            ctx.send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::Visible(false));
        }
    }

    /// The trigger picked since the last time this was asked, if any. The window is already hidden
    /// by the time it shows up here.
    pub fn take_chosen(&self) -> Option<String> {
        self.lock().chosen.take()
    }

    /// The window's own pass.
    fn draw(&self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut l = self.lock();
        if !l.open {
            return;
        }
        if let Some(scale) = ctx.input(|i| i.viewport().native_pixels_per_point) {
            l.scale = scale;
        }

        // Asked for again from inside, as well as by `open`. A hotkey press gives this process the
        // right to take the foreground, but only if it actually asks — and a search box that does
        // not have the keyboard is worse than none at all, because the keystrokes land in whatever
        // document was underneath.
        if std::mem::take(&mut l.needs_focus) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // Clicking anywhere else dismisses it, the way every launcher on every platform behaves —
        // including espanso's own. Gated on having held the keyboard first, because the frame
        // between asking for focus and getting it also reports "not focused", and closing there
        // would make the window flash and vanish.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        let mut outcome = if focused {
            l.had_focus = true;
            Outcome::Stay
        } else if l.had_focus {
            Outcome::Dismiss
        } else {
            Outcome::Stay
        };
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            outcome = Outcome::Dismiss;
        }
        if matches!(outcome, Outcome::Stay) {
            outcome = egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| show(ui, &mut l))
                .inner;
        }

        match outcome {
            Outcome::Stay => {}
            Outcome::Dismiss | Outcome::Pick(_) => {
                l.open = false;
                l.query.clear();
                if let Outcome::Pick(trigger) = outcome {
                    l.chosen = Some(trigger);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                // The main window may well be asleep in the tray; this is what wakes its logic to
                // collect the pick.
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
        }
    }
}

enum Outcome {
    Stay,
    Dismiss,
    Pick(String),
}

/// The scale of the screen `hwnd` is on: 1.0 at 100 %, 1.5 at 150 %.
fn window_scale(hwnd: windows::Win32::Foundation::HWND) -> f32 {
    let dpi = if hwnd.is_invalid() {
        0
    } else {
        unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }
    };
    if dpi == 0 {
        crate::display::system_scale()
    } else {
        dpi as f32 / 96.0
    }
}

/// The expansions matching the query, best first.
///
/// Matches on the trigger *and* on the replacement text, because half the time you remember what
/// the expansion says rather than what you named it. A trigger match sorts first: if you typed
/// something that looks like a trigger, that is almost certainly what you meant.
fn hits<'a>(entries: &'a [Hit], query: &str) -> Vec<&'a Hit> {
    let needle = query.trim().to_lowercase();
    let mut by_trigger = Vec::new();
    let mut by_text = Vec::new();

    for hit in entries {
        if needle.is_empty() || hit.trigger.to_lowercase().contains(&needle) {
            by_trigger.push(hit);
        } else if hit.preview.to_lowercase().contains(&needle) {
            by_text.push(hit);
        }
    }

    by_trigger.extend(by_text);
    by_trigger.truncate(MAX_RESULTS);
    by_trigger
}

/// How tall and wide the window has to be to show `rows` results without cutting anything off.
///
/// Derived from the constants above and from the fonts actually loaded, rather than guessed. The
/// guess it replaces was a fixed number that took no account of the spacing egui inserts between
/// widgets, so on a full list the window came up some seventy points too short and the footer line
/// underneath the results was sliced off the bottom edge — a line of text that was there, that the
/// window promised room for, and that could not be read.
fn window_size(ctx: &egui::Context, rows: usize) -> egui::Vec2 {
    let style = ctx.style_of(ctx.theme());
    let line = |text_style: egui::TextStyle| {
        let font = text_style.resolve(&style);
        ctx.fonts_mut(|f| f.row_height(&font))
    };
    let contents = if rows == 0 {
        GAP_ABOVE_EMPTY + line(egui::TextStyle::Body)
    } else {
        GAP_ABOVE_LIST
            + rows.min(VISIBLE_ROWS) as f32 * ROW_HEIGHT
            + GAP_BELOW_LIST
            + line(egui::TextStyle::Small)
    };
    let height = PAD * 2.0 + crate::ui::controls::FIELD_HEIGHT + contents;
    egui::vec2(WINDOW_WIDTH, height.ceil())
}

/// Draws the window's contents and reads its keys.
fn show(ui: &mut egui::Ui, l: &mut Launcher) -> Outcome {
    let Some(t) = l.t else {
        return Outcome::Dismiss;
    };
    let is_light = !ui.visuals().dark_mode;
    let accent_color = accent(ui.visuals());
    let mut outcome = Outcome::Stay;

    let (up, down, enter, escape) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if escape {
        return Outcome::Dismiss;
    }

    let count = hits(&l.entries, &l.query).len();
    // Keep the highlight inside the list as it shrinks under a longer query.
    if l.selected >= count {
        l.selected = count.saturating_sub(1);
    }
    if down && count > 0 {
        l.selected = (l.selected + 1) % count;
        l.follow = true;
    }
    if up && count > 0 {
        l.selected = l.selected.checked_sub(1).unwrap_or(count - 1);
        l.follow = true;
    }
    if enter {
        if let Some(hit) = hits(&l.entries, &l.query).get(l.selected) {
            return Outcome::Pick(hit.trigger.clone());
        }
    }

    egui::Frame::default()
        .fill(crate::app::win_background_for(is_light))
        .stroke(egui::Stroke::new(1.0, crate::app::line_strong(is_light)))
        .corner_radius(10u8)
        .inner_margin(egui::Margin::same(PAD as i8))
        .show(ui, |ui| {
            // Every gap in this window is written out below, so that what `window_size` computes
            // and what actually gets drawn cannot drift apart.
            ui.spacing_mut().item_spacing.y = 0.0;

            let field = ui.add(
                crate::ui::controls::text_field(&mut l.query)
                    .desired_width(ui.available_width())
                    .hint_text(t.search_window_hint),
            );
            // Focused every frame rather than once: the one thing that must never happen is that
            // you hit Alt+Space, start typing, and the letters go nowhere.
            if !field.has_focus() {
                field.request_focus();
            }
            if field.changed() {
                // A narrower query means a different list; start it from the top.
                l.selected = 0;
                l.follow = true;
            }

            let results = hits(&l.entries, &l.query);
            // Sized to what it is actually showing, the way every launcher does: the box grows
            // and shrinks as the query narrows, instead of leaving a slab of empty window under
            // the last result.
            let size = window_size(ui.ctx(), results.len());
            if size != l.size {
                l.size = size;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            }

            if results.is_empty() {
                ui.add_space(GAP_ABOVE_EMPTY);
                ui.label(egui::RichText::new(t.search_window_empty).color(text_tertiary(is_light)));
                return;
            }

            ui.add_space(GAP_ABOVE_LIST);
            let selected = l.selected;
            let follow = std::mem::take(&mut l.follow);
            egui::ScrollArea::vertical()
                .max_height(VISIBLE_ROWS as f32 * ROW_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, hit) in results.iter().enumerate() {
                        if row(ui, hit, i == selected, follow && i == selected, accent_color, is_light)
                        {
                            outcome = Outcome::Pick(hit.trigger.clone());
                        }
                    }
                });

            ui.add_space(GAP_BELOW_LIST);
            ui.label(
                egui::RichText::new(t.search_window_footer)
                    .small()
                    .color(text_tertiary(is_light)),
            );
        });

    outcome
}

/// One result. The same shape as a row in the main list — trigger in the accent, preview beside it —
/// so the two windows are recognisably the same program.
///
/// `follow` drags the view to this row: set on the row the arrows just moved to, so walking past
/// the bottom of the window scrolls instead of losing the highlight off the edge.
fn row(
    ui: &mut egui::Ui,
    hit: &Hit,
    selected: bool,
    follow: bool,
    accent_color: egui::Color32,
    is_light: bool,
) -> bool {
    let mut clicked = false;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );

    if follow {
        // `None` means "scroll the least that brings this into view", which keeps the list still
        // when the selection is already on screen.
        ui.scroll_to_rect(rect, None);
    }

    if ui.is_rect_visible(rect) {
        if selected {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(6),
                crate::app::selection_tint(is_light, accent_color),
            );
            // The same accent bar the main list puts against a selected row.
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
                egui::CornerRadius::same(2),
                accent_color,
            );
        } else if response.hovered() {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(6), crate::app::hover_tint(is_light));
        }

        let text_left = rect.left() + ROW_INSET;
        let preview_left = text_left + TRIGGER_COLUMN;
        let preview_color = if selected {
            ui.visuals().text_color()
        } else {
            secondary_text(is_light)
        };

        paint_line(
            ui,
            &hit.trigger,
            egui::TextStyle::Monospace,
            accent_color,
            egui::pos2(text_left, rect.center().y),
            TRIGGER_COLUMN - COLUMN_GAP,
        );
        paint_line(
            ui,
            &hit.preview,
            egui::TextStyle::Body,
            preview_color,
            egui::pos2(preview_left, rect.center().y),
            rect.right() - ROW_INSET - preview_left,
        );
        ui.painter().line_segment(
            [
                egui::pos2(rect.left() + 6.0, rect.bottom()),
                egui::pos2(rect.right() - 6.0, rect.bottom()),
            ],
            egui::Stroke::new(1.0, hairline(is_light)),
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        clicked = true;
    }
    clicked
}

/// Paints one line of text that ends at `max_width` with an ellipsis, vertically centred on `at`.
///
/// The main window has always done this properly — a `Label` set to truncate, which asks the font
/// how wide the text really is. This window paints its rows itself and was instead shortening the
/// preview by *counting characters*, at a count far larger than the room available, and then
/// hard-clipping whatever was left against the edge of the row. So the text ran all the way to the
/// window's border and stopped in the middle of a letter, with nothing to say that more had been
/// cut off. Measuring in pixels is the only way to end a line where you meant to.
fn paint_line(
    ui: &egui::Ui,
    text: &str,
    style: egui::TextStyle,
    color: egui::Color32,
    at: egui::Pos2,
    max_width: f32,
) {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        style.resolve(ui.style()),
        color,
        max_width.max(0.0),
    );
    // One row, and break wherever the width runs out: a preview is a glance at the text, not a
    // paragraph, and a word that does not fit should still show the part of it that does.
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    let top_left = egui::pos2(at.x, at.y - galley.size().y * 0.5);
    ui.painter().galley(top_left, galley, color);
}
