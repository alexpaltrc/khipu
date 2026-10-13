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

//! The search window: Alt+Space anywhere in Windows, pick an expansion, and it lands where the
//! cursor was.
//!
//! Espanso's own version of this is what it replaces. It is the library's list, made small: the
//! same search box, the same rows — trigger in its column, what it writes beside it, quietly — and
//! the same neutral fill with the accent bar on the chosen one. Nothing else: no title, no hints
//! underneath. Up, Down, Enter and Esc are what every launcher answers to.
//!
//! Two things move, both briefly, and neither with Windows' animation effects off: the contents
//! fade in as the window opens, and the chosen row's fill glides to the next one rather than
//! jumping — the library's own spring, so the two windows move alike.
//!
//! Keyboard first, because that is how this is used: it opens with the field focused. The mouse
//! works too: pointing at a row chooses it, clicking inserts it.
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
//! ## Why the list scrolls instead of stopping at five
//!
//! The window is a glance, not a page, so it is at most five rows tall: a launcher the size of a
//! page stops being one. Keeping it that short used to mean throwing away every result past the
//! last row, which quietly hid expansions from a search that had genuinely found them. Now every
//! match is kept and the list scrolls through the window — with the
//! arrows, with the wheel, and with the selection dragging the view along behind it, which is how
//! espanso's own bar behaves.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::app::{accent, hairline, text_tertiary};
use crate::i18n::Strings;
use crate::ui::controls;

/// An upper bound on how many matches are held at once. High enough that no real search ever meets
/// it, low enough that one keystroke can never turn into thousands of rows.
const MAX_RESULTS: usize = 200;

/// How many rows the window is tall. Everything past this is reached by scrolling.
const VISIBLE_ROWS: usize = 5;

/// One row's height, and therefore the pitch of the list: rows are drawn with no spacing between
/// them so that this number alone decides how tall the window has to be. The library's dense row:
/// the dense padding above and below one line of body text.
const ROW_HEIGHT: f32 = 36.0;

/// Wide enough for a trigger and a useful slice of what it expands to, and no wider: the slice is
/// there to tell two expansions apart, not to read them.
const WINDOW_WIDTH: f32 = 480.0;

/// Between the window's edge and everything in it, and between the search box and the list.
const PAD: f32 = 8.0;
const GAP_ABOVE_LIST: f32 = 6.0;

/// The trigger column: as wide as the widest trigger, so none is cut short — but never more than
/// this share of the row, so the text beside it always has room. The floor is the library's.
const TRIGGER_MIN: f32 = 92.0;
const TRIGGER_SHARE: f32 = 0.4;

/// How long the contents take to fade in when the window opens.
const APPEAR: f32 = 0.12;

pub struct Hit {
    pub trigger: String,
    /// The name the user gave it, if any: drawn ahead of the text and searched like the trigger.
    pub name: Option<String>,
    pub preview: String,
}

fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("khipu_search")
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
    /// The search box's height as last drawn, which is what the window's height is worked out
    /// from. Zero until it has been drawn once; [`field_height`] estimates it until then.
    field_height: f32,
    /// The trigger column for this opening, measured once from every trigger rather than from the
    /// matches, so the column does not shift while typing. Zero until measured.
    trigger_width: f32,
    /// Where the chosen row's fill is, and how fast it is going: see [`glide_selection`].
    glide: Glide,
    /// Set when the list itself changed, so the fill lands on the new first row instead of
    /// gliding there from a row of a list that is gone.
    jump: bool,
    /// When the window first drew after opening, for the fade in.
    shown_at: Option<f64>,
    /// The window's native handle, found once: see [`own_hwnd`]. Zero until found.
    hwnd: isize,
    /// Whether the compositor is being kept from drawing the window, and how many passes it has
    /// drawn since it opened: see [`set_cloaked`].
    cloaked: bool,
    passes: u32,
}

/// The chosen row's fill, as an offset from the top of the list.
#[derive(Default, Clone, Copy)]
struct Glide {
    top: f32,
    speed: f32,
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
            .with_title("Khipu")
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

    /// Whether what is typed in the box is Hindi, so its letters show although no expansion is.
    pub fn query_has_devanagari(&self) -> bool {
        let launcher = self.lock();
        launcher.open && crate::yaml::model::has_devanagari(&launcher.query)
    }

    /// Puts the window up over `over`, the window that had the keyboard, with `entries` to search.
    pub fn open(
        &self,
        ctx: &egui::Context,
        entries: Vec<Hit>,
        t: &'static Strings,
        over: windows::Win32::Foundation::HWND,
    ) {
        self.put_up(ctx, entries, t, over, None);
    }

    /// The preview build's way in, for the Store's screenshots: the same window with `query`
    /// already typed, placed at `at` (off-screen) and never handed the keyboard, so whoever is
    /// working at the computer is not interrupted. The released build never calls it.
    #[cfg(feature = "preview")]
    pub fn open_for_studio(
        &self,
        ctx: &egui::Context,
        entries: Vec<Hit>,
        t: &'static Strings,
        query: &str,
        at: egui::Pos2,
    ) {
        let over = windows::Win32::Foundation::HWND::default();
        self.put_up(ctx, entries, t, over, Some((query, at)));
    }

    fn put_up(
        &self,
        ctx: &egui::Context,
        entries: Vec<Hit>,
        t: &'static Strings,
        over: windows::Win32::Foundation::HWND,
        studio: Option<(&str, egui::Pos2)>,
    ) {
        let mut l = self.lock();
        l.entries = entries;
        l.t = Some(t);
        l.query = studio.map_or(String::new(), |(query, _)| query.to_owned());
        l.selected = 0;
        l.follow = true;
        l.needs_focus = studio.is_none();
        l.had_focus = false;
        l.chosen = None;
        l.trigger_width = 0.0;
        l.jump = true;
        l.shown_at = None;
        l.open = true;
        let size = window_size(ctx, l.field_height, hits(&l.entries, &l.query).len());
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
        if let Some((_, at)) = studio {
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::OuterPosition(at));
        } else if let Some(area) = area {
            let x = area.left + (area.width - size.x * target_scale) * 0.5;
            let y = area.top + (area.height - size.y * target_scale) * 0.35;
            ctx.send_viewport_cmd_to(
                id,
                egui::ViewportCommand::OuterPosition(egui::pos2(x / own_scale, y / own_scale)),
            );
        }
        if l.hwnd == 0 {
            l.hwnd = own_hwnd();
        }
        l.cloaked = set_cloaked(l.hwnd, true);
        l.passes = 0;
        ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Visible(true));
        if studio.is_none() {
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
        }
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
        // One pass has been drawn and presented: from now on there is something to see.
        if l.cloaked && l.passes > 0 {
            set_cloaked(l.hwnd, false);
            l.cloaked = false;
            l.shown_at = None;
        }
        l.passes += 1;
        if l.cloaked {
            ctx.request_repaint();
        }

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

/// This window's native handle, as an `isize`; zero if it cannot be found.
///
/// egui does not hand a child window's handle to the code that draws it, so it is looked for among
/// this thread's windows — which is where it lives, since eframe makes every window on the one
/// thread. Ours is the one that carries the app's name and sits above everything; the main window
/// never does. (Not "the one with no title bar": winit keeps the title bar style bits on a window
/// without decorations and hides the bar another way.)
fn own_hwnd() -> isize {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, GetWindowLongPtrW, GetWindowTextW, GWL_EXSTYLE, WS_EX_TOPMOST,
    };
    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> windows::core::BOOL {
        let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        let mut title = [0u16; 32];
        let len = unsafe { GetWindowTextW(hwnd, &mut title) }.max(0) as usize;
        if ex_style & WS_EX_TOPMOST.0 != 0
            && String::from_utf16_lossy(&title[..len]) == "Khipu"
        {
            unsafe { *(found.0 as *mut isize) = hwnd.0 as isize };
            return false.into();
        }
        true.into()
    }
    let mut found: isize = 0;
    unsafe {
        let _ = EnumThreadWindows(
            windows::Win32::System::Threading::GetCurrentThreadId(),
            Some(visit),
            LPARAM(&mut found as *mut isize as isize),
        );
    }
    found
}

/// Keeps the compositor from drawing the window, or lets it again. Says whether it took.
///
/// Windows puts a window on screen the moment it is shown, before the program behind it has drawn
/// a thing into it, and for that instant it is a white rectangle — a flash, on a dark screen, in
/// one opening out of two, measured. So the window is shown cloaked, draws its first frame unseen,
/// and is uncloaked on the pass after, with that frame already in it; the fade in starts there.
fn set_cloaked(hwnd: isize, cloaked: bool) -> bool {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    if hwnd == 0 {
        return false;
    }
    // A Win32 BOOL, which is an i32.
    let value = i32::from(cloaked);
    unsafe {
        DwmSetWindowAttribute(
            windows::Win32::Foundation::HWND(hwnd as *mut std::ffi::c_void),
            DWMWA_CLOAK,
            (&value as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        )
        .is_ok()
    }
}

/// The expansions matching the query, best first.
///
/// Matches on the trigger, on the name *and* on the replacement text, because half the time you
/// remember what the expansion says or what it is for rather than what you type. A trigger match
/// sorts first: if you typed something that looks like a trigger, that is almost certainly what you
/// meant. A name comes next, because you chose it to find this by.
fn hits<'a>(entries: &'a [Hit], query: &str) -> Vec<&'a Hit> {
    let needle = query.trim().to_lowercase();
    let mut by_trigger = Vec::new();
    let mut by_name = Vec::new();
    let mut by_text = Vec::new();

    for hit in entries {
        if needle.is_empty() || hit.trigger.to_lowercase().contains(&needle) {
            by_trigger.push(hit);
        } else if hit.name.as_ref().is_some_and(|name| name.to_lowercase().contains(&needle)) {
            by_name.push(hit);
        } else if hit.preview.to_lowercase().contains(&needle) {
            by_text.push(hit);
        }
    }

    by_trigger.extend(by_name);
    by_trigger.extend(by_text);
    by_trigger.truncate(MAX_RESULTS);
    by_trigger
}


/// How tall and wide the window has to be to show `rows` results without cutting anything off.
///
/// Every gap in the window is one of the constants above and nothing else, so what this computes
/// and what gets drawn cannot drift apart. `field` is the search box's height as last drawn, zero
/// if it never has been.
fn window_size(ctx: &egui::Context, field: f32, rows: usize) -> egui::Vec2 {
    let field = if field > 0.0 { field } else { field_height(ctx) };
    // No matches still keeps one row, for the line that says so.
    let list = rows.clamp(1, VISIBLE_ROWS) as f32 * ROW_HEIGHT;
    let height = PAD * 2.0 + field + GAP_ABOVE_LIST + list;
    egui::vec2(WINDOW_WIDTH, height.ceil())
}

/// The search box's height before it has ever been drawn: one line of body text inside the box's
/// margins, which [`controls::search_field`] sets at nine points above and below. Only the first
/// opening uses it; from then on the box's real height is known.
fn field_height(ctx: &egui::Context) -> f32 {
    let font = egui::TextStyle::Body.resolve(&ctx.global_style());
    ctx.fonts_mut(|f| f.row_height(&font)) + 18.0
}

/// The trigger column for `entries`: see [`TRIGGER_MIN`].
fn trigger_width(ui: &egui::Ui, entries: &[Hit]) -> f32 {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let widest = ui.ctx().fonts_mut(|f| {
        entries
            .iter()
            .map(|hit| {
                f.layout_no_wrap(hit.trigger.clone(), font.clone(), egui::Color32::PLACEHOLDER)
                    .size()
                    .x
            })
            .fold(0.0, f32::max)
    });
    let room = WINDOW_WIDTH - 2.0 * PAD - 2.0 * controls::ROW_PAD_X;
    (widest + controls::GAP_WIDE).clamp(TRIGGER_MIN, room * TRIGGER_SHARE)
}

/// Draws the window's contents and reads its keys.
fn show(ui: &mut egui::Ui, l: &mut Launcher) -> Outcome {
    let Some(t) = l.t else {
        return Outcome::Dismiss;
    };
    let is_light = !ui.visuals().dark_mode;
    let animate = crate::theme::animations_enabled();
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
    // Going round from one end to the other is a jump, not a glide: sliding the fill across the
    // whole list would show a journey nobody made.
    if down && count > 0 {
        l.selected = (l.selected + 1) % count;
        l.jump |= l.selected == 0;
        l.follow = true;
    }
    if up && count > 0 {
        l.jump |= l.selected == 0;
        l.selected = l.selected.checked_sub(1).unwrap_or(count - 1);
        l.follow = true;
    }
    if enter {
        if let Some(hit) = hits(&l.entries, &l.query).get(l.selected) {
            return Outcome::Pick(hit.trigger.clone());
        }
    }

    // The fade in, eased so that it is mostly there at once and settles rather than creeping up.
    let now = ui.input(|i| i.time);
    let shown_at = *l.shown_at.get_or_insert(now);
    let appear = if animate { ((now - shown_at) as f32 / APPEAR).min(1.0) } else { 1.0 };
    if appear < 1.0 {
        ui.ctx().request_repaint();
    }

    // No border and no rounding of our own: Windows 11 rounds this window's corners and draws its
    // edge and shadow, the same as it does for every other window.
    egui::Frame::default()
        .fill(crate::app::win_background_for(is_light))
        .inner_margin(egui::Margin::same(PAD as i8))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.multiply_opacity(1.0 - (1.0 - appear).powi(3));
            ui.spacing_mut().item_spacing.y = 0.0;

            let field = controls::search_field(
                ui,
                egui::Id::new("launcher-search"),
                &mut l.query,
                t.search_window_hint,
                ui.available_width(),
            );
            l.field_height = field.rect.height();
            // Focused every frame rather than once: the one thing that must never happen is that
            // you hit Alt+Space, start typing, and the letters go nowhere.
            if !field.has_focus() {
                field.request_focus();
            }
            if field.changed() {
                // A different query is a different list; start it from the top.
                l.selected = 0;
                l.follow = true;
                l.jump = true;
            }
            if l.trigger_width == 0.0 {
                l.trigger_width = trigger_width(ui, &l.entries);
            }

            let results = hits(&l.entries, &l.query);
            // Sized to what it is showing: the window grows and shrinks as the query narrows,
            // instead of leaving a slab of empty window under the last result.
            let size = window_size(ui.ctx(), l.field_height, results.len());
            if size != l.size {
                l.size = size;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            }

            ui.add_space(GAP_ABOVE_LIST);
            if results.is_empty() {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                    egui::Sense::hover(),
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    t.search_window_empty,
                    egui::TextStyle::Body.resolve(ui.style()),
                    text_tertiary(is_light),
                );
                return;
            }

            let selected = l.selected;
            let follow = std::mem::take(&mut l.follow);
            let jump = std::mem::take(&mut l.jump);
            // Pointing chooses, but only a pointer that is moving: rows sliding under a still one
            // as the list scrolls, or a window opening under it, choose nothing.
            let pointing = ui.input(|i| i.pointer.is_moving());
            let drop = controls::baseline_drop(
                ui,
                egui::RichText::new("x"),
                egui::RichText::new("x").monospace(),
            );
            let columns = Columns { trigger: l.trigger_width, drop };
            // Lowercased and trimmed the way [`hits`] searched.
            let needle = l.query.trim().to_lowercase();
            let glide = &mut l.glide;
            let mut pointed = None;
            egui::ScrollArea::vertical()
                .max_height(VISIBLE_ROWS as f32 * ROW_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let band = glide_selection(ui, glide, selected, jump, animate, is_light);
                    for (i, hit) in results.iter().enumerate() {
                        let response = row(ui, hit, &needle, i == 0, columns, band, is_light);
                        if follow && i == selected {
                            // `None`: scroll the least that brings it into view, which keeps the
                            // list still when the row is already on screen.
                            ui.scroll_to_rect(response.rect, None);
                        }
                        if pointing && i != selected && response.hovered() {
                            pointed = Some(i);
                        }
                        if response.clicked() {
                            outcome = Outcome::Pick(hit.trigger.clone());
                        }
                    }
                });
            if let Some(i) = pointed {
                l.selected = i;
                ui.ctx().request_repaint();
            }
        });

    outcome
}

/// Moves the chosen row's fill one frame towards the chosen row, paints it with the accent bar
/// beside it, and says where it is. Painted before the rows, so it lies under their text.
///
/// Rows all have the one height, so where the chosen row is never has to be measured: it is
/// `selected` rows down from the top of the list. Kept relative to that top, so a scroll carries
/// the fill along with its row.
fn glide_selection(
    ui: &egui::Ui,
    glide: &mut Glide,
    selected: usize,
    jump: bool,
    animate: bool,
    is_light: bool,
) -> egui::Rect {
    let target = selected as f32 * ROW_HEIGHT;
    if jump || !animate {
        *glide = Glide { top: target, speed: 0.0 };
    } else {
        let dt = ui.input(|i| i.stable_dt).clamp(0.0, 1.0 / 20.0);
        let (x, v) = crate::ui::glide::spring_step(glide.top - target, glide.speed, dt);
        if x.abs() > 0.1 || v.abs() > 0.5 {
            *glide = Glide { top: target + x, speed: v };
            ui.ctx().request_repaint();
        } else {
            *glide = Glide { top: target, speed: 0.0 };
        }
    }
    let rect = egui::Rect::from_min_size(
        egui::pos2(ui.max_rect().left(), ui.cursor().top() + glide.top),
        egui::vec2(ui.available_width(), ROW_HEIGHT),
    );
    ui.painter().rect_filled(
        rect,
        controls::RADIUS_CONTROL,
        crate::app::selection_tint(is_light, accent(ui.visuals())),
    );
    controls::accent_bar(ui, rect);
    rect
}

/// What every row of one frame shares: the trigger column's width, and how far below centre the
/// monospace trigger sits to share a baseline with the text beside it.
#[derive(Clone, Copy)]
struct Columns {
    trigger: f32,
    drop: f32,
}

/// One result: the library's row — the trigger in its column, what it writes beside it, quietly —
/// so the two windows are recognisably the same program.
fn row(
    ui: &mut egui::Ui,
    hit: &Hit,
    needle: &str,
    first: bool,
    columns: Columns,
    band: egui::Rect,
    is_light: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );
    if !ui.is_rect_visible(rect) {
        return response;
    }

    // The rule between two rows stops where the fill is, so the chosen row reads as one shape.
    let y = rect.top();
    if !first && !(band.top() - 0.5..=band.bottom() + 0.5).contains(&y) {
        ui.painter()
            .hline(rect.x_range(), y, egui::Stroke::new(1.0, hairline(is_light)));
    }

    let left = rect.left() + controls::ROW_PAD_X;
    let preview_left = left + columns.trigger;
    let strong = ui.visuals().strong_text_color();
    // What was typed, wherever it turns up, in the accent: the same mark as in the library.
    let mut trigger = egui::text::LayoutJob::default();
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    controls::marked(&mut trigger, &hit.trigger, needle, &mono, strong, accent(ui.visuals()));
    let trigger = egui::WidgetText::from(trigger).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (columns.trigger - controls::GAP_WIDE).max(1.0),
        egui::TextStyle::Body,
    );
    let preview = egui::WidgetText::from(controls::named_text(
        ui,
        hit.name.as_deref(),
        strong,
        &hit.preview,
        crate::app::secondary_text(is_light),
        needle,
    ))
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (rect.right() - controls::ROW_PAD_X - preview_left).max(1.0),
        egui::TextStyle::Body,
    );
    let middle = rect.center().y;
    let painter = ui.painter();
    painter.galley(
        egui::pos2(left, middle - trigger.size().y * 0.5 + columns.drop),
        trigger,
        egui::Color32::PLACEHOLDER,
    );
    painter.galley(
        egui::pos2(preview_left, middle - preview.size().y * 0.5),
        preview,
        egui::Color32::PLACEHOLDER,
    );
    response
}
