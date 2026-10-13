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

//! The application itself: its state, its Windows 11 palette, and the eframe loop that drives both.
//!
//! The longest-lived file here, and four neighbourhoods live in it. It is worth knowing which one
//! you are in before editing, because the rules differ.
//!
//! **1. `AppState`** — everything the interface reads, and every mutation it can perform. The
//! screens under [`crate::ui`] never open `base.yml` or `settings.json` themselves: they call a
//! method here, and that method is what validates, saves, reloads and sets the banner. If a change
//! has to reach the disk, it belongs on this side of the file, not in a view.
//!
//! **2. The palette** — `win_background` through `caution`, plus `tuned_visuals` and
//! `apply_layout_style`. These are Windows 11's own colours and type sizes rather than invented
//! ones, which is why they are written as literals with the Fluent name beside them. Every one of
//! them takes `is_light` as an argument instead of reading the theme itself, so a single frame can
//! never come out half in one theme and half in the other.
//!
//! **3. Window geometry** — `fit_window_to` and its helpers: the on-show repair that stops a size
//! or position remembered from a different machine opening off-screen or too small to use. The
//! rules come from [`crate::display`]; only the winit plumbing is here.
//!
//! **4. `KhipuApp`** — the eframe `App`. It owns the tray and the theme watch, drains
//! their queues and the hotkey's once a frame, and hands the frame to a view. The hotkey itself
//! is held by `AppState`, because Settings changes it.
//!
//! The split that matters in neighbourhood 4: `logic` runs every frame *including while the window
//! is hidden in the tray*, and eframe forbids drawing anything from it. `ui` runs only when there
//! is a window. Work that must happen whether or not anybody is looking goes in `logic`; anything
//! that draws goes in `ui`. The search window looks like the exception and is not: logic only keeps
//! it alive and sends it commands, and it draws itself in its own pass — see `ui::search_view`.

use crate::autostart;
use crate::espanso_ctl::EspansoCtl;
use crate::fonts;
use crate::i18n::{fill, Lang, Strings};
use crate::settings::{Settings, SettingsStore};
use crate::theme::ThemeMode;
use crate::tray::{Tray, TrayEvents, VISIBLE_POLL_INTERVAL};
use crate::ui;
use crate::ui::edit_form::EditState;
use crate::yaml::io as yaml_io;
use crate::yaml::model::{MatchEntry, MatchFile};
use eframe::egui;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
use std::sync::mpsc::TryRecvError;

/// Drag-and-drop payload: the triggers of the expansions being dragged (identifying entries by
/// trigger, not index, so nothing gets confused if the list re-filters mid-drag). A single-item
/// drag of something not currently selected, or the whole selection if you drag a selected row.
pub type DragPayload = Vec<String>;

/// Whether this app provides its own Alt+Space search window.
///
/// On since 0.0.4. Espanso's own bar picks a match by identity and so never sends backspaces, a
/// door it keeps to itself; ours went without one, eating the words in front of the cursor, until
/// it learnt to paste what it can write exactly (see `paste`) — measured whole in Notepad, Edge
/// and Windows Terminal, where espanso's own `match exec` types every character of `:zed` as the
/// last one, "L". It opens from the tray, which is where the shortcut is nearly always pressed:
/// see `ui::search_view` for how.
///
/// This constant is still the entire switch. Off, nothing of ours is registered, espanso gets its
/// shortcut and its bar back, and our window cannot open. What was built, measured and never
/// solved is in the notes that left the tree in 0.0.6: `git show 58753f7:LAUNCHER-BACKUP.txt`.
const OWN_LAUNCHER: bool = true;

/// Far more characters than any row can display, so the width-aware trim the label does afterwards
/// still has plenty to work with.
const PREVIEW_MAX_CHARS: usize = 200;

/// Shortens `s` to `max_chars` characters, adding an ellipsis if anything was cut. Counts
/// characters rather than bytes, so it can never split one in half.
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

/// Deliberately not boxed. `EditState` makes this enum about 240 bytes, and clippy asks for a
/// `Box` on that basis — but the reasoning behind that lint is arrays and queues of the enum, and
/// there is exactly one `View` in the whole program (`AppState::view`). Boxing it would buy back a
/// quarter of a kilobyte, once, in exchange for an allocation every time the editor opens and a
/// dereference at every place the editor is drawn.
#[allow(clippy::large_enum_variant)]
pub enum View {
    List,
    Edit(EditState),
    Tips,
    Onboarding,
    /// Everything that can be done *to* a folder rather than inside it: rename, change the prefix
    /// of its triggers, export it, delete it.
    ///
    /// A screen and not a permanent side panel, which is the whole point: these are five things a
    /// person does to a folder perhaps twice in its life, and a panel that shows them at all times
    /// spends a fifth of the window on them for ever. Reached from the library, one step back.
    FolderOptions(String),
}

// Every kind is a deletion, which is exactly what the name of each one should say out loud.
#[allow(clippy::enum_variant_names)]
pub enum PendingConfirmKind {
    /// One expansion, carrying the row index rather than only the trigger. The row is what was
    /// clicked, and a hand-edited `base.yml` can hold two entries with the same trigger, which
    /// deleting by trigger would take out together.
    DeleteOne { index: usize },
    DeleteFolder { folder: String },
    DeleteSelection,
}

/// A destructive action awaiting confirmation in an in-app modal.
///
/// All three kinds go through the same modal, including the single one. It used to raise a native
/// task dialog instead, which is grey, always light whatever the theme, and looks nothing like the
/// window that asked the question — the one surface in the app that did not match. The bulk kinds
/// were never able to use a native dialog anyway: the list of affected expansions can be long, and
/// a native dialog can neither scroll nor collapse to stay compact.
pub struct PendingConfirm {
    pub kind: PendingConfirmKind,
    /// Snapshot of affected entries at the moment the action was requested, as (trigger, preview)
    /// pairs — owned, so the confirmation stays stable even if the user somehow changes something
    /// else first.
    pub items: Vec<(String, String)>,
    pub expanded: bool,
}

/// Which half of a transfer the folder picker is standing in front of.
pub enum TransferDirection {
    /// Nothing has been named yet: the picker comes first, and the save dialog after it.
    Export,
    /// The file has already been read, so the picker can offer the folders it turned out to hold.
    Import,
}

/// One folder in the transfer picker, and whether it is travelling.
pub struct TransferGroup {
    /// `None` is the group of expansions that were never put in a folder.
    pub folder: Option<String>,
    pub count: usize,
    pub selected: bool,
}

/// An export or an import held back until the user says which folders it covers.
///
/// The document is carried here whole — already built, for an export, and already read, for an
/// import — and the folders are filtered out of it only when the picker is confirmed. An import
/// cannot ask the question any earlier than this: until the file has been read there is no way to
/// know which folders are in it.
pub struct PendingTransfer {
    pub direction: TransferDirection,
    pub export: crate::transfer::Export,
    pub groups: Vec<TransferGroup>,
    /// Imports only: expansions in the file that the picker does not offer, because nothing in
    /// their folder can be added. Added to the skipped count at the end. See
    /// [`crate::transfer::import_groups`].
    pub left_out: usize,
}

impl PendingTransfer {
    /// Every folder selected to begin with, so confirming the picker without touching anything
    /// does exactly what the button did before the picker existed.
    ///
    /// Takes the groups already computed rather than working them out again: the caller had to
    /// count them to know whether the picker was worth showing at all.
    fn new(
        direction: TransferDirection,
        export: crate::transfer::Export,
        groups: Vec<(Option<String>, usize)>,
    ) -> Self {
        Self {
            direction,
            export,
            groups: groups
                .into_iter()
                .map(|(folder, count)| TransferGroup {
                    folder,
                    count,
                    selected: true,
                })
                .collect(),
            left_out: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    Info,
    Error,
}

/// A change that the message announcing it can also take back.
///
/// Dragging expansions into a folder is the one action here that happens without a confirmation —
/// it has to, or the gesture would be unusable — and it can move a whole selection at once. Rather
/// than putting a dialog in front of every drop, the result is simply reversible: the banner that
/// says what happened carries the way to undo it.
pub enum UndoAction {
    /// Folder assignments exactly as they were before the move, as (trigger, previous folder).
    FolderMove(Vec<(String, Option<String>)>),
}

pub struct Banner {
    pub kind: BannerKind,
    pub message: String,
    pub undo: Option<UndoAction>,
}

/// One expansion as the list draws it.
pub struct ListRow {
    pub trigger: String,
    /// What the user calls it, if they named it: drawn ahead of the text, and searched like it.
    pub name: Option<String>,
    /// The replacement, already shortened. Hard-capped here rather than at the widget:
    /// `Label::truncate` does the final width-aware trim, but it has to lay the text out to know
    /// where to cut, so handing it a multi-kilobyte replacement would make the row pay for glyphs
    /// nobody can see.
    pub preview: String,
    /// Switched off: kept, drawn dimmer, and absent from espanso and from the launcher.
    pub off: bool,
    /// Starred: a filled star on its row, and ahead of the rest in the list and the launcher.
    pub favorite: bool,
}

/// Everything the list screen needs, derived once from the expansions and the folder assignments.
///
/// Rebuilt only when something it depends on actually changes (see [`AppState::list`]). Deriving it
/// per frame instead meant re-reading every expansion, re-rendering every preview and re-sorting
/// every folder sixty times a second just to draw a list that had not moved — invisible at a dozen
/// expansions, and several milliseconds of pure waste at a few thousand.
pub struct ListCache {
    /// Folder navigation is rebuilt with the list, never re-sorted on every repaint.
    pub folder_names: Vec<String>,
    pub folders: std::collections::BTreeMap<String, crate::folders::FolderInfo>,
    pub rows: Vec<ListRow>,
    /// Every index into `rows`, in order: what the list shows with no folder picked. Kept so that
    /// case can be lent out like the other two instead of being counted out afresh every frame.
    pub all: Vec<usize>,
    /// Folder name → indices into `rows`, in the A→Z order folders are shown in.
    pub grouped: Vec<(String, Vec<usize>)>,
    /// Indices into `rows` for the expansions that belong to no folder.
    pub ungrouped: Vec<usize>,
    /// Expansions in the file, including the ones this list never shows — so the screen can tell
    /// "you have none yet" apart from "your search matched none".
    pub total_entries: usize,
}

/// What the cached list was built from. Anything that can change the list changes this, so a stale
/// cache cannot survive.
#[derive(PartialEq)]
struct ListCacheKey {
    revision: u64,
    search: String,
    lang: Lang,
    /// Belt and braces alongside `revision`: even if some future edit path forgot to bump the
    /// revision, adding or removing an expansion or a folder assignment still invalidates the cache.
    entry_count: usize,
    folder_assignment_count: usize,
}

/// Who Windows' own dialogs belong to.
///
/// The confirmations and the file pickers are drawn by Windows, not by this app, and Windows has to
/// be told which window each one is *for*. Told nothing, it makes them ownerless, and two things
/// follow: they open centred on the screen rather than on the window they came from, and when one
/// closes Windows is under no obligation to hand the keyboard back to us. It can leave this thread
/// with no focused window at all — which is the failure
/// [`crate::display::restore_keyboard_focus`] exists to undo, and naming an owner here is the same
/// bug attacked from the other end.
///
/// It carries the window's handle and not the window, deliberately: `AppState` decides and saves,
/// and holding the window would be reaching well past that. A dialog needs nothing but the number
/// that names its owner, and a number is `Copy` and keeps nothing alive.
#[derive(Clone, Copy)]
pub struct DialogOwner(Option<std::num::NonZeroIsize>);

impl DialogOwner {
    fn new(window: Option<&winit::window::Window>) -> Self {
        Self(
            window
                .and_then(hwnd_of)
                .and_then(|hwnd| std::num::NonZeroIsize::new(hwnd.0 as isize)),
        )
    }
}

impl raw_window_handle::HasWindowHandle for DialogOwner {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let hwnd = self.0.ok_or(raw_window_handle::HandleError::Unavailable)?;
        // The one thing the unsafety is about is whether the handle is real and belongs to this
        // thread. It is our own main window, created by this thread, and every dialog that asks is
        // modal — the call that opened it is still on the stack below, so the window cannot have
        // gone anywhere in the meantime. When there is no window, `rfd` gets the `Err` and falls
        // back to an ownerless dialog, exactly as it behaved before it was ever asked.
        Ok(unsafe {
            raw_window_handle::WindowHandle::borrow_raw(raw_window_handle::RawWindowHandle::Win32(
                raw_window_handle::Win32WindowHandle::new(hwnd),
            ))
        })
    }
}

impl raw_window_handle::HasDisplayHandle for DialogOwner {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        // Windows has no display handle to speak of, and `rfd` never reads this one on Windows —
        // it is here because `set_parent` asks for both traits. The constructor is the safe one.
        Ok(raw_window_handle::DisplayHandle::windows())
    }
}

/// All application state that isn't specific to rendering one particular screen. Owned by
/// [`KhipuApp`] and passed down to the `ui::*` view functions.
pub struct AppState {
    pub match_file: MatchFile,
    pub match_file_path: PathBuf,
    pub backups_dir: PathBuf,
    pub settings: Settings,
    pub settings_store: SettingsStore,
    pub ctl: EspansoCtl,
    pub view: View,
    /// Ajustes is a dialog over whatever screen is behind it, not a screen of its own. See
    /// [`crate::ui::settings_view`].
    pub settings_open: bool,
    /// What was said while Ajustes was open. It is shown inside the dialog, because the banner
    /// behind it is under the dimmed backdrop, and it is dropped when the dialog closes.
    pub settings_note: Option<Banner>,
    /// «Novedades», over the list after an update. See [crate::ui::whats_new_view].
    pub whats_new_open: bool,
    /// The theme worn before «Probarlo» in Novedades put on «Ultra oscuro», so that the same button
    /// can offer the way back. Only for this run: after a restart there is nothing to go back from.
    pub whats_new_theme_before: Option<ThemeMode>,
    pub search: String,
    pub banner: Option<Banner>,
    /// Why the expansions file could not be read, if it could not. Saving is refused while this
    /// is set: the model in memory is empty, and it did not come from disk.
    pub load_error: Option<String>,
    pub autostart_enabled: bool,
    pub exe_path: PathBuf,
    /// Triggers of the expansions currently checked in the list, for bulk actions.
    pub selected: BTreeSet<String>,
    /// Where a Shift-click range starts, following the same rule as a file list: the last row that
    /// was clicked on its own.
    pub selection_anchor: Option<String>,
    /// Whether espanso is paused right now.
    ///
    /// A *copy*, refreshed from the tray at the top of every frame that has a window. The pause
    /// state genuinely belongs to [`crate::tray::Tray`] — it is the thing that owns the timed pause,
    /// the icon and the menu label — and the tray is owned by [`KhipuApp`], not by the
    /// state the views receive. Mirroring it is what lets the library show "Activo"/"Pausado"
    /// without a view ever being handed the tray or the daemon.
    pub paused: bool,
    /// Whether espanso's last answer was a yes: it came up at startup, restarted after a save, or
    /// took the last pause or resume.
    ///
    /// Never asked for on its own. Every check of the daemon blocks the interface thread (see
    /// `espanso_ctl`), so this only records the answers the app already had to wait for — which
    /// is enough for the library to stop saying "Activo" about a daemon that never replied.
    pub espanso_confirmed: bool,
    /// Set by the library's Pausar/Reanudar button and consumed by [`KhipuApp::ui`] after
    /// the frame is drawn.
    ///
    /// A request rather than an action, for the same reason: pausing means running `espanso` and
    /// waiting on it, and a view that could do that would be a view that writes to disk. The flag
    /// keeps the rule intact — every mutation goes through `AppState` or through the app itself.
    pub pause_toggle_requested: bool,
    /// Restarts espanso after a save without holding up the window. See `reload`.
    pub reload: crate::reload::Reloader,
    /// What the banner already said about the last save — its first line and any warning — so a
    /// restart that fails can be reported under it rather than instead of it.
    reload_note: Option<(String, Option<String>)>,
    /// (original name, current edit buffer) while a folder's name is being edited inline.
    pub renaming_folder: Option<(String, String)>,
    pub pending_confirm: Option<PendingConfirm>,
    /// An export or import waiting for its folders to be chosen. See [`PendingTransfer`].
    pub pending_transfer: Option<PendingTransfer>,
    /// Set when the language changes; the owning app reloads fonts and tray labels next frame,
    /// since those need the egui context and the tray handle that `AppState` doesn't hold.
    pub pending_language_refresh: bool,
    /// Set when the theme mode changes, so the palette is re-applied immediately rather than at
    /// the next scheduled poll.
    pub pending_theme_refresh: bool,
    /// Set the moment the hidden theme is found, and cleared once its option in Ajustes has been
    /// pointed out for [`crate::ui::settings_view::SPOTLIGHT`]: the option is drawn with a halo
    /// and given the keyboard until then.
    pub khipu_spotlight: Option<Instant>,
    /// What has been typed into the first-run screen's trial box, and whether the expansion has
    /// fired in it yet.
    pub onboarding_probe: String,
    pub onboarding_expanded: bool,
    /// Espanso's own version. Shown in Settings, never acted on.
    ///
    /// Read the first time that screen is opened rather than at startup. Asking for it means
    /// spawning `espanso --version` and waiting on it for up to `CTL_TIMEOUT`, and at startup that
    /// wait happened with the window already created and unresponsive — on every launch, including
    /// the hidden autostart one, for a line most people never look at. See
    /// [`AppState::ensure_espanso_version`].
    pub espanso_version: Option<String>,
    /// Whether the question above has been asked. Kept apart from the answer because "espanso would
    /// not say" is also an answer, and asking again every time Settings is opened would mean
    /// another four-second wait on exactly the machine that could least afford the first one.
    pub espanso_version_asked: bool,
    /// The shortcut that actually opens the search window. Whatever the tips screen promises has to
    /// be this.
    pub search_shortcut: crate::hotkey::Shortcut,
    /// The thread that owns the shortcut. `None` while [`OWN_LAUNCHER`] is off: espanso's own bar
    /// has the shortcut then, and Settings has nothing to change.
    pub hotkey: Option<crate::hotkey::HotkeyWatch>,
    /// While the shortcut box in Settings is listening: the modifiers held at this moment.
    pub shortcut_capture: Option<u32>,
    /// The last shortcut asked for that another program already has. Said under the box until a
    /// shortcut is taken.
    pub shortcut_taken: Option<crate::hotkey::Shortcut>,
    /// Bumped by every change to the expansions or their folders; see [`AppState::touch_list`].
    list_revision: u64,
    list_cache: Option<Rc<ListCache>>,
    list_cache_key: Option<ListCacheKey>,
    /// [`MatchFile::wants_devanagari`] for the list as it was at this revision and length, so the
    /// question is asked again only when the expansions change, not every frame.
    devanagari_cache: std::cell::Cell<Option<(u64, usize, bool)>>,
    /// The window that Windows' own confirmations and file pickers belong to. See [`DialogOwner`]:
    /// it is a handle, not the window, and it is the only thing in here that names one.
    dialog_owner: DialogOwner,
}

impl AppState {
    pub fn set_info_banner(&mut self, message: impl Into<String>) {
        self.post(BannerKind::Info, message.into());
    }

    pub fn set_error_banner(&mut self, message: impl Into<String>) {
        self.post(BannerKind::Error, message.into());
    }

    /// Where a message goes: the banner, or the dialog in front of it while Ajustes is open.
    fn post(&mut self, kind: BannerKind, message: String) {
        let banner = Some(Banner { kind, message, undo: None });
        if self.settings_open {
            self.settings_note = banner;
        } else {
            self.banner = banner;
        }
    }

    /// Opens Ajustes over the current screen. The one way in.
    ///
    /// Blocking here is fine: the user has just asked for the dialog, and the switch has to say what
    /// Windows will really do at the next start, not what it said when the app was launched.
    pub fn open_settings(&mut self) {
        self.refresh_autostart_cache();
        self.ensure_espanso_version();
        self.settings_note = None;
        self.settings_open = true;
    }

    pub fn close_settings(&mut self) {
        self.settings_open = false;
        self.settings_note = None;
        self.khipu_spotlight = None;
        self.shortcut_taken = None;
        self.cancel_shortcut_capture();
    }

    /// Starts listening for a new shortcut. Every key goes to the box until one is chosen, Esc is
    /// pressed, or [`cancel_shortcut_capture`](Self::cancel_shortcut_capture) is called.
    pub fn begin_shortcut_capture(&mut self) {
        if let Some(hotkey) = &self.hotkey {
            hotkey.begin_capture();
            self.shortcut_capture = Some(0);
            self.shortcut_taken = None;
        }
    }

    pub fn cancel_shortcut_capture(&mut self) {
        if self.shortcut_capture.take().is_some() {
            if let Some(hotkey) = &self.hotkey {
                hotkey.end_capture();
            }
        }
    }

    /// Makes `wanted` the shortcut, if no other program has it. If one does, the shortcut stays
    /// what it was and Settings says why.
    pub fn set_launcher_shortcut(&mut self, wanted: crate::hotkey::Shortcut) {
        self.shortcut_capture = None;
        let list: &[crate::hotkey::Shortcut] = if wanted == crate::hotkey::DEFAULT {
            &crate::hotkey::CANDIDATES
        } else {
            std::slice::from_ref(&wanted)
        };
        let Some(hotkey) = self.hotkey.as_mut() else {
            return;
        };
        let Some(won) = hotkey.claim(list) else {
            self.shortcut_taken = Some(wanted);
            return;
        };
        self.search_shortcut = won;
        self.shortcut_taken = None;
        let stored = (wanted != crate::hotkey::DEFAULT).then(|| wanted.to_config());
        if self.settings.launcher_shortcut != stored {
            self.settings.launcher_shortcut = stored;
            if let Err(e) = self.settings_store.save(&self.settings) {
                let msg = fill(self.t().launcher_shortcut_save_error, &[("err", &e.to_string())]);
                self.set_error_banner(msg);
            }
        }
    }

    /// The scratch file holding the one expansion the first-run screen demonstrates.
    ///
    /// Deliberately its own file rather than a line in `base.yml`. The app only ever reads and
    /// writes `base.yml`, so a trial expansion living there would be a real entry in the user's own
    /// list — and one added behind their back. Here it is separate, espanso picks it up because it
    /// loads every file in the match folder, and it is deleted the moment the screen is finished
    /// with. Anything left behind by a crash is cleared at the next start.
    fn onboarding_match_path(&self) -> PathBuf {
        self.match_file_path
            .parent()
            .map(|dir| dir.join("onboarding.yml"))
            .unwrap_or_else(|| PathBuf::from("onboarding.yml"))
    }

    /// Puts the trial expansion in place. Espanso watches the match folder, so it becomes live on
    /// its own within a moment — no restart, and nothing touched that the user owns.
    pub fn begin_onboarding(&mut self) {
        let path = self.onboarding_match_path();
        let body = format!(
            "# Temporary — created by Khipu for its first-run screen and deleted as soon\n\
             # as that screen is finished with. Nothing here is yours; edit base.yml instead.\n\
             matches:\n- trigger: '{}'\n  replace: '{}'\n",
            crate::ui::onboarding_view::PROBE_TRIGGER,
            crate::ui::onboarding_view::probe_reply(self.settings.lang),
        );
        let _ = std::fs::write(path, body);
    }

    /// Takes the trial expansion away again and remembers that the screen has been seen.
    pub fn finish_onboarding(&mut self) {
        self.clear_onboarding_match();
        self.settings.onboarding_done = true;
        // Everything is new to somebody who has just arrived: there is no update to tell them about.
        self.settings.whats_new_seen = Some(crate::ui::whats_new_view::RELEASE.to_string());
        let _ = self.settings_store.save(&self.settings);
    }

    /// Closes «Novedades» for good, until a release with new things.
    pub fn dismiss_whats_new(&mut self) {
        self.whats_new_open = false;
        self.settings.whats_new_seen = Some(crate::ui::whats_new_view::RELEASE.to_string());
        let _ = self.settings_store.save(&self.settings);
    }

    pub fn clear_onboarding_match(&self) {
        let _ = std::fs::remove_file(self.onboarding_match_path());
    }

    /// Every expansion this app manages, as trigger, name and one-line preview, for the search window.
    /// The ones switched off are left out: the launcher fires expansions, and these do not fire.
    ///
    /// Starred ones come first unless that is switched off in Ajustes. The launcher ranks by what
    /// the query matched and keeps this order inside each rank, so a star lifts an expansion among
    /// its equals without beating a trigger typed out in full.
    pub fn list_all(&self) -> Vec<crate::ui::search_view::Hit> {
        let t = self.t();
        let mut hits: Vec<_> = self
            .match_file
            .entries
            .iter()
            .filter_map(|entry| match entry {
                MatchEntry::Simple(m) if !self.match_file.is_off(&m.trigger) => {
                    Some(crate::ui::search_view::Hit {
                        trigger: m.trigger.clone(),
                        name: entry.name().map(str::to_owned),
                        // A form says how many blanks it will ask for: its text, with the
                        // blanks still empty, is not what it writes.
                        preview: match m.as_form() {
                            Some(form) => crate::ui::studio::blanks_to_fill(self, form.fields.len()),
                            None => entry.preview(t),
                        },
                    })
                }
                MatchEntry::Simple(_) => None,
                MatchEntry::Advanced(_) => None,
            })
            .collect();
        if self.settings.favorites_first {
            // Stable, so each half keeps the file's order.
            hits.sort_by_key(|hit| !self.settings.is_favorite(&hit.trigger));
        }
        hits
    }

    /// Stars or unstars one expansion, at once: a star is a mark on the list, not an edit, and
    /// asking for Guardar to keep one would be asking for a step nobody expects.
    pub fn set_favorite(&mut self, trigger: &str, on: bool) {
        if self.settings.is_favorite(trigger) == on {
            return;
        }
        self.settings.set_favorite(trigger, on);
        self.touch_list();
        if let Err(e) = self.settings_store.save(&self.settings) {
            let msg = fill(ui::studio::text(
                self,
                "No se pudo guardar el favorito: {err}",
                "Could not save the favorite: {err}",
                "Hindi ma-save ang paborito: {err}",
                "पसंदीदा सहेजा नहीं जा सका: {err}",
            ), &[("err", &e.to_string())]);
            self.set_error_banner(msg);
        }
    }

    pub fn set_favorites_first(&mut self, on: bool) {
        if self.settings.favorites_first == on {
            return;
        }
        self.settings.favorites_first = on;
        if let Err(e) = self.settings_store.save(&self.settings) {
            let msg = fill(ui::studio::text(
                self,
                "No se pudo guardar el ajuste del buscador: {err}",
                "Could not save the search setting: {err}",
                "Hindi ma-save ang setting ng paghahanap: {err}",
                "खोज की सेटिंग सहेजी नहीं जा सकी: {err}",
            ), &[("err", &e.to_string())]);
            self.set_error_banner(msg);
        }
    }

    /// What the launcher can paste for `trigger` by itself. See
    /// [`crate::yaml::model::SimpleMatch::literal_text`].
    pub fn literal_text(&self, trigger: &str) -> Option<String> {
        self.match_file.entries.iter().find_map(|entry| match entry {
            MatchEntry::Simple(m) if m.trigger == trigger => m.literal_text(),
            _ => None,
        })
    }

    /// Asks espanso its version, once, on the way into the screen that shows it.
    ///
    /// Blocking here is fine in a way it was not at startup: the user has just clicked Ajustes and
    /// the window is up and drawn. Called from [`AppState::open_settings`].
    pub fn ensure_espanso_version(&mut self) {
        if self.espanso_version_asked {
            return;
        }
        self.espanso_version_asked = true;
        self.espanso_version = self.ctl.version(self.t());
    }

    pub fn refresh_autostart_cache(&mut self) {
        self.autostart_enabled = autostart::is_enabled(&self.exe_path);
    }

    /// Shorthand for the active language's string table.
    pub fn t(&self) -> &'static Strings {
        self.settings.t()
    }

    /// Marks the list as out of date.
    ///
    /// Call this from anywhere that changes an expansion or a folder assignment. The search box and
    /// the interface language are watched by the cache key itself and need no call.
    fn touch_list(&mut self) {
        self.list_revision = self.list_revision.wrapping_add(1);
    }

    /// Whether something on screen needs the Devanagari face although the interface is not in
    /// Hindi: an expansion written in it, a date spelled in it, or the one being edited.
    pub fn needs_devanagari(&self) -> bool {
        if self.settings.lang == Lang::Hi {
            return true;
        }
        if let View::Edit(edit) = &self.view {
            let editing = match &edit.kind {
                ui::edit_form::Kind::Text { replace } => crate::yaml::model::has_devanagari(replace),
                ui::edit_form::Kind::Date { month_lang, .. } => *month_lang == Lang::Hi,
            };
            if editing
                || crate::yaml::model::has_devanagari(&edit.word)
                || crate::yaml::model::has_devanagari(&edit.name)
                || edit.fields.iter().any(crate::yaml::form::FormField::has_devanagari)
            {
                return true;
            }
        }
        let key = (self.list_revision, self.match_file.entries.len());
        match self.devanagari_cache.get() {
            Some((revision, len, wants)) if (revision, len) == key => wants,
            _ => {
                let wants = self.match_file.wants_devanagari();
                self.devanagari_cache.set(Some((key.0, key.1, wants)));
                wants
            }
        }
    }

    /// The list as it should be drawn, rebuilding it only if something it depends on has changed.
    ///
    /// Handed out behind an [`Rc`] so a caller can hold on to it while still mutating the rest of
    /// the state — selecting a row, opening the editor, deleting something — without the borrow
    /// checker forcing the whole list to be re-derived just to satisfy it.
    pub fn list(&mut self) -> Rc<ListCache> {
        let key = ListCacheKey {
            revision: self.list_revision,
            search: self.search.clone(),
            lang: self.settings.lang,
            entry_count: self.match_file.entries.len(),
            folder_assignment_count: self.settings.folder_by_trigger.len(),
        };
        if self.list_cache_key.as_ref() != Some(&key) || self.list_cache.is_none() {
            self.list_cache = Some(Rc::new(self.build_list()));
            self.list_cache_key = Some(key);
        }
        self.list_cache.clone().expect("just populated above")
    }

    fn build_list(&self) -> ListCache {
        let t = self.t();
        let query = self.search.to_lowercase();

        // Only text, date and form expansions are ever surfaced — anything else (shell commands, regex
        // matches, per-app conditionals, ...) stays untouched in the YAML file but out of this
        // list, by design, so coworkers only ever see the kinds they are meant to use.
        let mut rows: Vec<ListRow> = Vec::new();
        for entry in self.match_file.entries.iter().filter(|e| e.is_safely_editable()) {
            let preview = entry.preview(t);
            if !query.is_empty() {
                let matches = entry.trigger_str().to_lowercase().contains(&query)
                    || entry.name().is_some_and(|name| name.to_lowercase().contains(&query))
                    || preview.to_lowercase().contains(&query);
                if !matches {
                    continue;
                }
            }
            rows.push(ListRow {
                trigger: entry.trigger_label(),
                name: entry.name().map(str::to_owned),
                preview: truncate(&preview, PREVIEW_MAX_CHARS),
                off: self.match_file.is_off(entry.trigger_str()),
                favorite: self.settings.is_favorite(entry.trigger_str()),
            });
        }

        // Starred ones go to the top and what is switched off to the bottom: of the whole list, and
        // so of every folder too, since the buckets below are filled in this order. Always, unlike
        // the launcher, where Ajustes can turn the star's lift off. Off outranks the star: an
        // expansion that does not fire is not one to keep at hand. The sort is stable, so each band
        // keeps the file's order.
        rows.sort_by_key(|row| (row.off, !row.favorite));

        // Grouped in one pass (O(n)) rather than re-scanning the rows once per folder name
        // (O(folders × n)). `BTreeMap` also gives the A→Z folder ordering for free.
        let mut grouped: std::collections::BTreeMap<String, Vec<usize>> =
            std::collections::BTreeMap::new();
        let mut ungrouped: Vec<usize> = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            match self.settings.folder_of(&row.trigger) {
                // The folder name is only copied the first time it's seen, not once per row.
                Some(folder) => match grouped.get_mut(folder) {
                    Some(bucket) => bucket.push(index),
                    None => {
                        grouped.insert(folder.to_string(), vec![index]);
                    }
                },
                None => ungrouped.push(index),
            }
        }

        // The order the list is drawn in is one of `all`, `ungrouped` or a folder's bucket, picked
        // by the screen's filter. The one place that needs it as triggers builds it at the moment
        // it is asked for: `ui::list_view::visible_order`.
        ListCache {
            folder_names: self.settings.all_folder_names(),
            folders: crate::folders::summarize(&self.match_file.entries, &self.settings),
            all: (0..rows.len()).collect(),
            rows,
            grouped: grouped.into_iter().collect(),
            ungrouped,
            total_entries: self.match_file.entries.len(),
        }
    }

    pub fn set_autostart(&mut self, enabled: bool) {
        if crate::EXPERIMENTAL { return; }
        let t = self.t();
        match autostart::set_enabled(enabled, &self.exe_path) {
            Ok(()) => {
                // Read back rather than assuming the write is the whole answer. Windows keeps a
                // separate veto over start-up entries, and if clearing it did not take, the entry
                // is written and still dead — better a switch that stays where the machine has it
                // than one that says yes on top of a no.
                //
                // Nothing is announced: the switch, drawn from that read-back, is the answer. A
                // sentence repeating it under the switch was one more thing to read for nothing.
                self.refresh_autostart_cache();
            }
            // The installed copy's startup task can be vetoed in Windows' own Settings, and only
            // there: the useful thing to say is where.
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && crate::install::is_packaged() => {
                self.refresh_autostart_cache();
                self.set_error_banner(t.autostart_vetoed.to_string());
            }
            Err(e) => {
                self.refresh_autostart_cache();
                self.set_error_banner(fill(t.autostart_error, &[("err", &e.to_string())]));
            }
        }
    }

    pub fn set_prefix(&mut self, prefix: String) {
        if prefix.trim().is_empty() {
            return;
        }
        self.settings.prefix = prefix;
        if let Err(e) = self.settings_store.save(&self.settings) {
            let msg = fill(self.t().prefix_save_error, &[("err", &e.to_string())]);
            self.set_error_banner(msg);
        }
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        if self.settings.theme_mode == mode {
            return;
        }
        self.settings.theme_mode = mode;
        self.settings.khipu_theme = mode == ThemeMode::Khipu;
        self.settings.ultra_dark = mode == ThemeMode::UltraDark;
        self.save_theme_settings();
        self.pending_theme_refresh = true;
    }

    fn save_theme_settings(&mut self) {
        if let Err(e) = self.settings_store.save(&self.settings) {
            let msg = fill(self.t().theme_save_error, &[("err", &e.to_string())]);
            self.set_error_banner(msg);
        }
    }

    /// The hidden theme has just been found (↑ ↑ ↓ ↓ ← → ← → in Consejos). It joins the other three
    /// in Ajustes for good, and Ajustes opens with it pointed out — offered, not put on: wearing it
    /// is still the user's click.
    ///
    /// Found again, it is pointed out again; nothing else happens twice.
    pub fn find_khipu_theme(&mut self) {
        if !self.settings.khipu_theme_found {
            self.settings.khipu_theme_found = true;
            self.save_theme_settings();
        }
        self.open_settings();
        self.khipu_spotlight = Some(Instant::now());
    }

    pub fn set_language(&mut self, lang: Lang) {
        if self.settings.lang == lang {
            return;
        }
        self.settings.lang = lang;
        if let Err(e) = self.settings_store.save(&self.settings) {
            let msg = fill(self.t().language_save_error, &[("err", &e.to_string())]);
            self.set_error_banner(msg);
        }
        // Fonts and tray labels can't be swapped from here (they need the egui context and the
        // tray handle), so the owning app picks this up on the next frame.
        self.pending_language_refresh = true;
    }

    /// Strips a leading run of non-alphanumeric characters (`:`, `;`, `?`, `//`, ...), treating
    /// it as "whatever prefix this trigger currently uses".
    fn strip_leading_prefix(trigger: &str) -> &str {
        trigger.trim_start_matches(|c: char| !c.is_alphanumeric())
    }

    pub fn apply_prefix_to_existing(&mut self) {
        let new_prefix = self.settings.prefix.clone();
        let mut renamed = Vec::new();
        let mut resulting: Vec<String> = Vec::new();

        for entry in &self.match_file.entries {
            if let MatchEntry::Simple(m) = entry {
                // `:--` and its kind keep the trigger they have: with no word after the symbols
                // there is nothing for a new prefix to sit in front of, and replacing "the prefix"
                // would consume the whole trigger. Still listed below, so a rename that would
                // land on one of them is caught as the collision it is.
                let rest = Self::strip_leading_prefix(&m.trigger);
                let new_trigger = if crate::folders::has_word(&m.trigger) {
                    format!("{new_prefix}{rest}")
                } else {
                    m.trigger.clone()
                };
                resulting.push(new_trigger.clone());
                if new_trigger != m.trigger {
                    renamed.push((m.trigger.clone(), new_trigger));
                }
            } else {
                resulting.push(entry.trigger_label());
            }
        }

        let t = self.t();
        if renamed.is_empty() {
            self.set_info_banner(t.prefix_already_applied);
            return;
        }

        let mut seen = std::collections::HashSet::new();
        let mut collisions = Vec::new();
        for r in &resulting {
            if !seen.insert(r.as_str()) {
                collisions.push(r.clone());
            }
        }
        if !collisions.is_empty() {
            self.set_error_banner(fill(
                t.prefix_collision,
                &[("list", &collisions.join(", "))],
            ));
            return;
        }

        let summary: String = renamed
            .iter()
            .take(8)
            .map(|(old, new)| format!("  {old}  →  {new}"))
            .collect::<Vec<_>>()
            .join("\n");
        let more = if renamed.len() > 8 {
            fill(t.prefix_confirm_more, &[("n", &(renamed.len() - 8).to_string())])
        } else {
            String::new()
        };

        let confirmed = rfd::MessageDialog::new()
            .set_parent(&self.dialog_owner)
            .set_title(t.prefix_confirm_title)
            .set_description(crate::i18n::fill_count(
                t.prefix_confirm_body,
                renamed.len(),
                &[("list", &summary), ("more", &more)],
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if confirmed != rfd::MessageDialogResult::Yes {
            return;
        }

        // Exactly the renames the question listed: worked out again here, `:--` lost its symbols
        // to the prefix alone, a change nobody was shown and the collision check never saw.
        let by_old: std::collections::HashMap<&str, &str> =
            renamed.iter().map(|(old, new)| (old.as_str(), new.as_str())).collect();
        for entry in &mut self.match_file.entries {
            if let MatchEntry::Simple(m) = entry {
                if let Some(new) = by_old.get(m.trigger.as_str()) {
                    m.trigger = (*new).to_owned();
                }
            }
        }
        self.match_file
            .rename_triggers(renamed.iter().map(|(old, new)| (old.as_str(), new.as_str())));
        for (old, new) in &renamed {
            self.settings.rename_trigger(old, new);
            self.selection_rename(old, new);
        }
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();

        self.save_and_reload(crate::i18n::fill_count(t.prefix_applied, renamed.len(), &[]));
    }

    pub fn request_delete(&mut self, index: usize) {
        let t = self.t();
        let Some(entry) = self.match_file.entries.get(index) else {
            return;
        };
        self.pending_confirm = Some(PendingConfirm {
            kind: PendingConfirmKind::DeleteOne { index },
            items: vec![(entry.trigger_label(), entry.preview(t))],
            expanded: false,
        });
    }

    /// Removes every entry whose trigger is in `triggers`, cleaning up their folder assignments
    /// too. Matches by trigger rather than index so it's safe to call with a snapshot taken
    /// earlier (e.g. from a confirmation modal).
    fn remove_by_triggers(&mut self, triggers: &[String]) {
        // A `HashSet` membership check keeps this O(n) even for a huge bulk delete — `.retain`
        // with `.any(...)` over `triggers` for every entry would be O(entries × triggers), which
        // gets slow fast once both sides are in the thousands.
        let to_remove: std::collections::HashSet<&str> = triggers.iter().map(String::as_str).collect();
        self.match_file
            .entries
            .retain(|e| !to_remove.contains(e.trigger_str()));
        self.match_file.prune_off();
        for t in triggers {
            self.settings.remove_trigger(t);
            self.selection_forget(t);
        }
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();
    }

    /// Follows a renamed expansion through the selection.
    ///
    /// `selected` and `selection_anchor` hold trigger strings, and a trigger is not an identity —
    /// it is the one thing about an expansion the user is most likely to change. Renaming without
    /// this leaves the old name behind: the bar counts a row that is no longer there, and the next
    /// drag carries the dead name into [`assign_folder_to_triggers`].
    fn selection_rename(&mut self, old: &str, new: &str) {
        if self.selected.remove(old) {
            self.selected.insert(new.to_string());
        }
        if self.selection_anchor.as_deref() == Some(old) {
            self.selection_anchor = Some(new.to_string());
        }
    }

    /// Drops a deleted expansion from the selection. The anchor goes with it: a Shift-click
    /// extending from a row that no longer exists finds nothing to measure from and silently
    /// degrades into a plain click, which is a worse answer than starting a fresh range.
    fn selection_forget(&mut self, trigger: &str) {
        self.selected.remove(trigger);
        if self.selection_anchor.as_deref() == Some(trigger) {
            self.selection_anchor = None;
        }
    }

    pub fn toggle_selected(&mut self, trigger: &str) {
        if !self.selected.remove(trigger) {
            self.selected.insert(trigger.to_string());
        }
        self.selection_anchor = Some(trigger.to_string());
    }

    /// Handles a click on a row using the conventions of a Windows file list: a plain click selects
    /// just that row, Ctrl adds or removes one, and Shift extends from the last plainly-clicked row
    /// to this one.
    ///
    /// `visible_order` is the list exactly as drawn, so a Shift range covers what the user actually
    /// sees between the two rows rather than some hidden underlying order.
    pub fn click_select(
        &mut self,
        trigger: &str,
        ctrl: bool,
        shift: bool,
        visible_order: &[String],
    ) {
        if shift {
            if let Some(anchor) = self.selection_anchor.clone() {
                let from = visible_order.iter().position(|t| t == &anchor);
                let to = visible_order.iter().position(|t| t == trigger);
                if let (Some(a), Some(b)) = (from, to) {
                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                    if !ctrl {
                        self.selected.clear();
                    }
                    for t in &visible_order[lo..=hi] {
                        self.selected.insert(t.clone());
                    }
                    // The anchor deliberately stays put, so dragging the shift-click around keeps
                    // growing and shrinking the same range.
                    return;
                }
            }
        }

        if ctrl {
            self.toggle_selected(trigger);
            return;
        }

        // A plain click on an already-multi-selected row keeps the selection, so it can be dragged
        // as a group — clicking a single row still collapses down to just that one.
        let only_this = self.selected.len() == 1 && self.selected.contains(trigger);
        if only_this {
            self.selected.clear();
            self.selection_anchor = None;
        } else {
            self.selected.clear();
            self.selected.insert(trigger.to_string());
            self.selection_anchor = Some(trigger.to_string());
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.selection_anchor = None;
    }

    /// Whether the current selection contains at least one expansion that actually sits in a
    /// folder — the "Quitar de su carpeta" action is meaningless (and so stays hidden) otherwise.
    /// Cheap: at most one map lookup per selected item, and the selection is small by nature.
    pub fn selection_has_foldered(&self) -> bool {
        self.selected
            .iter()
            .any(|t| self.settings.folder_of(t).is_some())
    }

    /// The list's look: comfortable rows, compact rows, or, with `grid`, the grid of cards.
    pub fn set_list_view(&mut self, compact: bool, grid: bool) {
        if self.settings.compact_view == compact && self.settings.grid_view == grid {
            return;
        }
        self.settings.compact_view = compact;
        self.settings.grid_view = grid;
        if let Err(e) = self.settings_store.save(&self.settings) {
            let msg = fill(self.t().view_save_error, &[("err", &e.to_string())]);
            self.set_error_banner(msg);
        }
    }

    pub fn request_delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let t = self.t();
        // Walked in list order, one pass, keeping whatever is ticked. Walking the selection
        // instead needed a lookup table and still came out wrong: the selection is a set, so it
        // iterates alphabetically, and two ticked rows would swap places between the list and the
        // question asked about them.
        let items: Vec<(String, String)> = self
            .match_file
            .entries
            .iter()
            .filter_map(|e| {
                let trigger = e.trigger_label();
                if self.selected.contains(&trigger) {
                    Some((trigger, e.preview(t)))
                } else {
                    None
                }
            })
            .collect();
        self.pending_confirm = Some(PendingConfirm {
            kind: PendingConfirmKind::DeleteSelection,
            items,
            expanded: false,
        });
    }

    pub fn request_delete_folder(&mut self, folder: &str) {
        let t = self.t();
        let items: Vec<(String, String)> = self
            .match_file
            .entries
            .iter()
            .filter(|e| self.settings.folder_of(&e.trigger_label()) == Some(folder))
            .map(|e| (e.trigger_label(), e.preview(t)))
            .collect();
        // An empty folder has nothing to warn about: the confirmation exists because deleting a
        // folder deletes what is in it, and there is nothing in this one. Asking anyway would be a
        // dialog whose honest body is "this will delete nothing".
        if items.is_empty() {
            let message = crate::i18n::fill_count(t.folder_deleted, 0, &[("name", folder)]);
            self.settings.forget_folder(folder);
            let _ = self.settings_store.save(&self.settings);
            self.touch_list();
            self.view = View::List;
            self.set_info_banner(message);
            return;
        }
        self.pending_confirm = Some(PendingConfirm {
            kind: PendingConfirmKind::DeleteFolder {
                folder: folder.to_string(),
            },
            items,
            expanded: false,
        });
    }

    /// Makes a folder that holds nothing yet. Returns whether the name was free.
    pub fn create_folder(&mut self, name: &str) -> bool {
        if !self.settings.create_folder(name) {
            let t = self.t();
            self.set_error_banner(fill(t.folder_name_taken, &[("name", name.trim())]));
            return false;
        }
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();
        true
    }

    pub fn cancel_pending_confirm(&mut self) {
        self.pending_confirm = None;
    }

    pub fn confirm_pending_delete(&mut self) {
        let Some(pending) = self.pending_confirm.take() else {
            return;
        };
        let t = self.t();
        let n = pending.items.len();
        let triggers = || pending.items.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>();
        match pending.kind {
            // The single row is the one path that leaves the selection alone: deleting the
            // expansion open in the editor is no reason to forget what was ticked in the list.
            PendingConfirmKind::DeleteOne { index } => {
                self.delete_one_at(index, pending.items.first().map(|(t, _)| t.as_str()));
            }
            PendingConfirmKind::DeleteFolder { ref folder } => {
                let message = crate::i18n::fill_count(t.folder_deleted, n, &[("name", folder)]);
                self.remove_by_triggers(&triggers());
                // The declaration goes with the contents. Left behind, the folder would come back
                // as an empty one the moment the list was rebuilt — which is not what "delete
                // Trabajo and its 4 expansions" said it would do.
                self.settings.forget_folder(folder);
                let _ = self.settings_store.save(&self.settings);
                self.clear_selection();
                if matches!(self.view, View::FolderOptions(ref f) if f == folder) {
                    self.view = View::List;
                }
                self.save_and_reload(message);
            }
            PendingConfirmKind::DeleteSelection => {
                let message = crate::i18n::fill_count(t.selection_deleted, n, &[]);
                self.remove_by_triggers(&triggers());
                self.clear_selection();
                self.save_and_reload(message);
            }
        }
    }

    /// Deletes the entry at `index`, but only while that row still holds the trigger the
    /// confirmation showed. Nothing can edit the list behind a modal, so this never fires in
    /// practice — it is here so that an index kept across a frame can never delete the wrong
    /// expansion if that ever stops being true.
    ///
    /// Unlike the bulk path this leaves the selection alone: deleting one row from the editor is
    /// no reason to forget what was ticked in the list.
    fn delete_one_at(&mut self, index: usize, expected: Option<&str>) {
        let matches = self
            .match_file
            .entries
            .get(index)
            .map(|e| e.trigger_label());
        let Some(trigger) = matches else { return };
        if expected.is_some_and(|want| want != trigger) {
            return;
        }
        self.match_file.entries.remove(index);
        self.match_file.prune_off();
        self.settings.remove_trigger(&trigger);
        self.selection_forget(&trigger);
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();
        self.view = View::List;
        let t = self.t();
        self.save_and_reload(t.expansion_deleted);
    }

    pub fn start_rename_folder(&mut self, folder: &str) {
        self.renaming_folder = Some((folder.to_string(), folder.to_string()));
    }

    pub fn cancel_rename_folder(&mut self) {
        self.renaming_folder = None;
    }

    pub fn commit_rename_folder(&mut self) {
        let Some((old, new)) = self.renaming_folder.take() else {
            return;
        };
        let new = new.trim().to_string();
        if new.is_empty() || new == old {
            return;
        }
        let t = self.t();
        if self.settings.all_folder_names().iter().any(|f| f == &new) {
            self.set_error_banner(fill(t.folder_name_taken, &[("name", &new)]));
            return;
        }
        for folder in self.settings.folder_by_trigger.values_mut() {
            if *folder == old {
                *folder = new.clone();
            }
        }
        // And the declaration, when there is one — otherwise renaming an empty folder would leave
        // the old name behind and create a second, equally empty one beside it.
        if self.settings.folders.remove(&old) {
            self.settings.folders.insert(new.clone());
        }
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();
        if matches!(self.view, View::FolderOptions(ref f) if f == &old) {
            self.view = View::FolderOptions(new.clone());
        }
        self.set_info_banner(fill(t.folder_renamed, &[("name", &new)]));
    }

    /// Reconciles the folder map with the file, once, at startup.
    ///
    /// See [`Settings::drop_folders_not_in`] for what an orphan assignment costs. Every entry is
    /// checked, not just the ones the list shows: an advanced match is invisible here but is very
    /// much still in the file, and its folder is not stale just because this app declines to draw
    /// a row for it.
    ///
    /// Skipped when the file could not be read. A failed load leaves an empty model behind, and
    /// pruning against that would delete every folder the user has for the one reason that is
    /// certainly not their doing — the same reasoning that stops [`Self::save_and_reload`] from
    /// writing.
    fn drop_orphan_folders(&mut self) {
        if self.load_error.is_some() {
            return;
        }
        let live: std::collections::HashSet<String> = self
            .match_file
            .entries
            .iter()
            .map(MatchEntry::trigger_label)
            .collect();
        // Written back only when something actually went, so an ordinary start still touches
        // nothing on disk.
        if self.settings.drop_folders_not_in(&live) > 0 {
            let _ = self.settings_store.save(&self.settings);
        }
    }

    /// Moves a group of expansions into `folder`, or clears their folder if `None` — used by the
    /// "sin carpeta" drop zone as well as regular assignment from the edit form.
    pub fn assign_folder_to_triggers(&mut self, triggers: &[String], folder: Option<String>) {
        // Where each of these lived a moment ago, so the message can offer to put them back — and,
        // first, so anything already where it is being sent can be dropped from the operation.
        //
        // Without this, dragging an expansion that belongs to no folder onto the empty strip that
        // means "no folder" still announced a move and offered to undo one. Nothing had changed.
        // A gesture that does nothing should say nothing.
        //
        // A trigger that no longer names anything is dropped here rather than filed. The folder map
        // is keyed by trigger, and a key with no expansion behind it is unreachable: the list groups
        // only rows that exist, so no folder header is ever drawn for it and there is nothing to
        // click to rename or delete it — while it goes on offering its folder in the edit form's
        // picker and goes on claiming that name against a real folder being renamed.
        let previous: Vec<(String, Option<String>)> = {
            let live: std::collections::HashSet<&str> = self
                .match_file
                .entries
                .iter()
                .map(MatchEntry::trigger_str)
                .collect();
            triggers
                .iter()
                .filter(|trigger| live.contains(trigger.as_str()))
                .map(|trigger| {
                    (
                        trigger.clone(),
                        self.settings.folder_of(trigger).map(str::to_string),
                    )
                })
                .filter(|(_, was)| was.as_deref() != folder.as_deref())
                .collect()
        };

        if previous.is_empty() {
            return;
        }
        let triggers: Vec<String> = previous.iter().map(|(t, _)| t.clone()).collect();
        let triggers = &triggers[..];

        for trigger in triggers {
            self.settings.set_folder(trigger, folder.clone());
        }
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();
        self.clear_selection();
        let t = self.t();
        let n = triggers.len();
        match &folder {
            Some(f) => self.set_info_banner(crate::i18n::fill_count(t.moved_to_folder, n, &[("name", f)])),
            None => self.set_info_banner(crate::i18n::fill_count(t.removed_from_folder, n, &[])),
        }
        if let Some(banner) = &mut self.banner {
            banner.undo = Some(UndoAction::FolderMove(previous));
        }
    }

    /// Switches expansions off, or back on.
    ///
    /// Off, an expansion stays in the list and in its folder, but moves to `_disabled.yml`, which
    /// espanso does not load, and leaves the launcher. On, it goes back to `base.yml` and to both.
    /// Those already where they are asked to be are dropped first, so the message counts only what
    /// actually changed — and a gesture that changes nothing says nothing.
    pub fn set_off(&mut self, triggers: &[String], off: bool) {
        let changing: Vec<String> = triggers
            .iter()
            .filter(|trigger| self.match_file.is_off(trigger) != off)
            .filter(|trigger| {
                self.match_file
                    .entries
                    .iter()
                    .any(|e| matches!(e, MatchEntry::Simple(m) if m.trigger == **trigger))
            })
            .cloned()
            .collect();
        if changing.is_empty() {
            return;
        }

        let previous_off = self.match_file.off.clone();
        let previous_entries = self.match_file.entries.clone();
        for trigger in &changing {
            if off {
                self.match_file.off.insert(trigger.clone());
            } else {
                self.match_file.off.remove(trigger);
            }
        }
        // Switched off, an expansion also goes to the end of the list in memory, because that is
        // where the next start will find it: read back, the file of switched-off expansions comes
        // after `base.yml`. Left where it was, the order on screen would change on a restart.
        if off {
            let (moving, staying): (Vec<MatchEntry>, Vec<MatchEntry>) = std::mem::take(&mut self.match_file.entries)
                .into_iter()
                .partition(|e| changing.iter().any(|t| t == e.trigger_str()));
            self.match_file.entries = staying;
            self.match_file.entries.extend(moving);
        }
        self.touch_list();

        let t = self.t();
        let template = if off { t.turned_off } else { t.turned_on };
        let message = crate::i18n::fill_count(template, changing.len(), &[]);
        if !self.save_and_reload(message) {
            // Not on disk, so not on screen either: the red banner says why.
            self.match_file.off = previous_off;
            self.match_file.entries = previous_entries;
            self.touch_list();
        }
    }

    /// Writes one folder's expansions out to a file, with nothing to choose between first: the
    /// folder was already named by the screen this came from.
    pub fn export_folder(&mut self, folder: &str) {
        let export = crate::transfer::build_export(&self.match_file.entries, |trigger| {
            self.settings.folder_of(trigger).map(str::to_owned)
        }, |trigger| self.match_file.is_off(trigger), |trigger| self.settings.is_favorite(trigger));
        let export = crate::transfer::retain_folders(export, &[Some(folder.to_owned())]);
        if !export.expansions.is_empty() { self.write_export(export); }
    }

    pub fn apply_folder_prefix(&mut self, folder: &str, prefix: &str) {
        if let Some(error) = self.load_error.clone() { self.set_error_banner(error); return; }
        let changes = match crate::folders::plan(&self.match_file.entries, &self.settings, folder, prefix) {
            Ok(changes) => changes,
            Err(error) => { self.set_error_banner(ui::studio::prefix_error(self, &error)); return; }
        };
        if changes.is_empty() { self.set_info_banner(self.t().prefix_already_applied); return; }
        let mut candidate = self.match_file.clone();
        let mut settings = self.settings.clone();
        for change in &changes {
            if let MatchEntry::Simple(m) = &mut candidate.entries[change.index] { m.trigger.clone_from(&change.new); }
        }
        candidate.rename_triggers(changes.iter().map(|c| (c.old.as_str(), c.new.as_str())));
        crate::folders::apply_assignments(&mut settings, &changes);
        let warnings = match yaml_io::save(&self.match_file_path, &candidate, &self.backups_dir) {
            Ok(warnings) => warnings,
            Err(error) => { self.set_error_banner(error.friendly_message(self.t())); return; }
        };
        // If the folder map cannot be persisted, restore the original YAML before accepting the
        // operation. No changed trigger should be left detached from its folder after a restart.
        if let Err(error) = self.settings_store.save(&settings) {
            let rollback = yaml_io::save(&self.match_file_path, &self.match_file, &self.backups_dir);
            if rollback.is_err() { self.match_file = candidate; self.settings = settings; self.touch_list(); }
            self.set_error_banner(format!("{}: {error}{}", self.t().view_save_error,
                rollback.err().map(|e| format!("\n{}", e.friendly_message(self.t()))).unwrap_or_default()));
            return;
        }
        self.match_file = candidate;
        self.settings = settings;
        self.clear_selection();
        self.touch_list();
        let t = self.t();
        let message = crate::i18n::fill_count(t.prefix_applied, changes.len(), &[]);
        self.announce_and_reload(message, warnings.message(t));
    }

    /// Writes the expansions the list shows out to a file the user picks.
    ///
    /// With two or more folders in play the picker comes first, so that sending one folder to a
    /// colleague does not mean handing over the whole collection. With one folder or none there is
    /// nothing to choose between, and the question is skipped rather than asked with a single
    /// possible answer.
    pub fn export_expansions(&mut self) {
        let export = crate::transfer::build_export(&self.match_file.entries, |trigger| {
            self.settings.folder_of(trigger).map(str::to_string)
        }, |trigger| self.match_file.is_off(trigger), |trigger| self.settings.is_favorite(trigger));

        let groups = crate::transfer::groups(&export);
        if groups.len() < 2 {
            self.write_export(export);
            return;
        }
        self.pending_transfer = Some(PendingTransfer::new(
            TransferDirection::Export,
            export,
            groups,
        ));
    }

    /// Asks for a file name and writes `export` to it.
    ///
    /// Takes the document rather than building one, because it is reached from two places: the
    /// button directly, when there was nothing to ask, and the picker, which has already thrown
    /// away the folders the user left out.
    fn write_export(&mut self, export: crate::transfer::Export) {
        let t = self.t();
        let count = export.expansions.len();

        let Some(path) = rfd::FileDialog::new()
            .set_parent(&self.dialog_owner)
            .set_title(t.export_button)
            .set_file_name(crate::transfer::suggested_filename())
            .add_filter(t.transfer_file_kind, &["yml", "yaml"])
            .save_file()
        else {
            return; // The user closed the dialog; that is not an error worth a banner.
        };

        match crate::transfer::write(&path, &export) {
            Ok(()) => self.set_info_banner(fill(t.export_done, &[("n", &count.to_string())])),
            Err(crate::transfer::TransferError::Io(e)) => {
                self.set_error_banner(fill(t.export_error, &[("err", &e)]))
            }
            Err(_) => self.set_error_banner(t.export_error),
        }
    }

    /// Adds the expansions from a file the user picks. Never replaces anything: see
    /// [`crate::transfer::merge`].
    ///
    /// The file is read before anything is asked, because until it has been read there is no way
    /// to know which folders it holds — and those are what the picker offers.
    pub fn import_expansions(&mut self) {
        let t = self.t();
        let Some(path) = rfd::FileDialog::new()
            .set_parent(&self.dialog_owner)
            .set_title(t.import_button)
            .add_filter(t.transfer_file_kind, &["yml", "yaml"])
            .pick_file()
        else {
            return;
        };

        let export = match crate::transfer::read(&path) {
            Ok(export) => export,
            Err(crate::transfer::TransferError::Io(e)) => {
                self.set_error_banner(fill(t.import_error, &[("err", &e)]));
                return;
            }
            Err(_) => {
                // "Not one of ours" and "valid but empty" land in the same place: either way there
                // is nothing to add, and the file is the thing to look at.
                self.set_error_banner(t.import_not_ours);
                return;
            }
        };

        // Counted against what is already here, so each folder offers what it will really add.
        let (groups, left_out) = crate::transfer::import_groups(&self.match_file.entries, &export);
        if groups.len() < 2 {
            self.merge_import(export, 0);
            return;
        }
        let mut pending = PendingTransfer::new(TransferDirection::Import, export, groups);
        pending.left_out = left_out;
        self.pending_transfer = Some(pending);
    }

    /// Adds an already-read import to what is there, and says what happened. `left_out` is the
    /// number of expansions the picker already knew would be skipped and so never offered.
    fn merge_import(&mut self, export: crate::transfer::Export, left_out: usize) {
        let t = self.t();
        let merged = crate::transfer::merge(&self.match_file.entries, export);
        let added = merged.added.len();
        let skipped = merged.skipped + left_out;

        if added == 0 {
            self.set_info_banner(fill(t.import_none_added, &[("k", &skipped.to_string())]));
            return;
        }

        // Kept so a failed write can be taken back: otherwise the list would show expansions that
        // are not in the file, and the next successful save would write them in unasked.
        let previous_entries = self.match_file.entries.clone();
        let previous_off = self.match_file.off.clone();
        let previous_settings = self.settings.clone();
        self.match_file.entries.extend(merged.added);
        self.match_file.off.extend(merged.off);
        for (trigger, folder) in &merged.folders {
            self.settings.set_folder(trigger, folder.clone());
        }
        for trigger in &merged.favorites {
            self.settings.set_favorite(trigger, true);
        }
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();

        let message = if skipped == 0 {
            fill(t.import_done, &[("n", &added.to_string())])
        } else {
            fill(
                t.import_done_skipped,
                &[("n", &added.to_string()), ("k", &skipped.to_string())],
            )
        };
        if !self.save_and_reload(message) {
            self.match_file.entries = previous_entries;
            self.match_file.off = previous_off;
            self.settings = previous_settings;
            let _ = self.settings_store.save(&self.settings);
            self.touch_list();
        }
    }

    /// Closes a folder picker without exporting or importing anything.
    pub fn cancel_pending_transfer(&mut self) {
        self.pending_transfer = None;
    }

    /// Carries out the transfer the picker was standing in front of, with the chosen folders only.
    pub fn confirm_pending_transfer(&mut self) {
        let Some(pending) = self.pending_transfer.take() else {
            return;
        };

        let keep: Vec<Option<String>> = pending
            .groups
            .into_iter()
            .filter(|group| group.selected)
            .map(|group| group.folder)
            .collect();
        if keep.is_empty() {
            // The picker will not let its own button be pressed in this state; this is the second
            // line of defence. An export of nothing would still write a file, and an import of
            // nothing would announce "added: 0" as though something had gone wrong.
            return;
        }

        let export = crate::transfer::retain_folders(pending.export, &keep);
        match pending.direction {
            TransferDirection::Export => self.write_export(export),
            TransferDirection::Import => self.merge_import(export, pending.left_out),
        }
    }


    /// Puts back whatever the banner currently on screen is offering to undo.
    pub fn undo_from_banner(&mut self) {
        let Some(banner) = self.banner.take() else {
            return;
        };
        let Some(UndoAction::FolderMove(previous)) = banner.undo else {
            return;
        };
        for (trigger, folder) in &previous {
            self.settings.set_folder(trigger, folder.clone());
        }
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();
        let t = self.t();
        self.set_info_banner(t.move_undone);
    }

    pub fn save_edit(&mut self, edit: &EditState) {
        let t = self.t();
        let new_match = edit.build_match();
        if new_match.trigger.trim().is_empty() {
            self.set_error_banner(t.trigger_empty);
            return;
        }

        // Advanced matches count too. They are not shown in the list, so a trigger that collides
        // with one would look free, save without complaint, and then never fire — espanso keeps
        // the first match for a trigger, and the invisible one is usually the first.
        let duplicate = self.match_file.entries.iter().enumerate().any(|(i, e)| {
            Some(i) != edit.editing_index && e.trigger_str() == new_match.trigger
        });
        if duplicate {
            self.set_error_banner(fill(t.trigger_duplicate, &[("name", &new_match.trigger)]));
            return;
        }

        let previous_trigger = edit.editing_index.and_then(|i| match &self.match_file.entries[i] {
            MatchEntry::Simple(m) => Some(m.trigger.clone()),
            MatchEntry::Advanced(_) => None,
        });

        // What to put back if the file cannot be written. Until `base.yml` has it, the edit exists
        // only in the form — so the form stays, and the list must not show a row that disk lacks.
        let previous_entries = self.match_file.entries.clone();
        let previous_off = self.match_file.off.clone();
        let previous_settings = self.settings.clone();

        match edit.editing_index {
            Some(index) => self.match_file.entries[index] = MatchEntry::Simple(new_match.clone()),
            None => self.match_file.entries.push(MatchEntry::Simple(new_match.clone())),
        }

        if let Some(previous_trigger) = &previous_trigger {
            self.match_file.rename_trigger(previous_trigger, &new_match.trigger);
            self.settings.rename_trigger(previous_trigger, &new_match.trigger);
            self.selection_rename(previous_trigger, &new_match.trigger);
        }
        self.settings.set_folder(&new_match.trigger, edit.resolved_folder());
        self.settings.set_favorite(&new_match.trigger, edit.favorite);
        let _ = self.settings_store.save(&self.settings);
        self.touch_list();

        if self.save_and_reload(t.expansion_saved) {
            self.view = View::List;
        } else {
            // The red banner `save_and_reload` raised says why; the draft is still on screen,
            // untouched, to try again or cancel.
            self.match_file.entries = previous_entries;
            self.match_file.off = previous_off;
            self.settings = previous_settings;
            let _ = self.settings_store.save(&self.settings);
            if let Some(previous_trigger) = &previous_trigger {
                self.selection_rename(&new_match.trigger, previous_trigger);
            }
            self.touch_list();
        }
    }

    /// Writes `base.yml`, says so, and has espanso restarted behind it. Returns whether the file was
    /// written — a restart that later fails does not undo that, because the change is on disk and
    /// will be picked up by the next espanso that starts.
    fn save_and_reload(&mut self, success_message: impl Into<String>) -> bool {
        // The model never came from disk. Writing it back would put an empty file where one we
        // could not read used to be, and every expansion in it would be gone from espanso. Say so
        // instead, in the same words the dialog at startup used.
        if let Some(message) = self.load_error.clone() {
            self.set_error_banner(message);
            return false;
        }
        let t = self.t();
        match yaml_io::save(&self.match_file_path, &self.match_file, &self.backups_dir) {
            Ok(warnings) => {
                self.announce_and_reload(success_message.into(), warnings.message(t));
                true
            }
            Err(e) => {
                self.set_error_banner(e.friendly_message(t));
                false
            }
        }
    }

    /// Says what a write did, then has espanso pick it up — without waiting for it to.
    ///
    /// The edit is on disk either way, so the first line always says what was done: "1 añadida, 1
    /// omitida" is the answer to what the user asked, and a failed restart used to replace it
    /// outright. A warning goes under it, and so, later, does a restart that failed
    /// ([`collect_reload`](Self::collect_reload)) — worth the red banner, because the moment it
    /// matters is the moment nobody is looking.
    fn announce_and_reload(&mut self, message: String, warning: Option<String>) {
        match &warning {
            None => self.set_info_banner(message.clone()),
            Some(warning) => self.set_error_banner(format!("{message}\n\n{warning}")),
        }
        self.reload_note = Some((message, warning));
        self.reload.request(self.t());
    }

    /// Picks up how the last restart went, once it has. Called every frame; costs a lock.
    pub fn collect_reload(&mut self) {
        let Some(outcome) = self.reload.take_outcome() else {
            return;
        };
        self.espanso_confirmed = outcome.is_ok();
        let note = self.reload_note.take();
        if let Err(e) = outcome {
            self.set_error_banner(match note {
                Some((message, None)) => format!("{message}\n\n{e}"),
                Some((message, Some(warning))) => format!("{message}\n\n{e}\n\n{warning}"),
                None => e,
            });
        }
    }
}

// --- The palette ------------------------------------------------------------------------------
//
// One token set, defined once here, for every screen: Windows 11's own, as WinUI 3 states them.
// WinUI gives most of its fills as a translucent white or black over the window; they are resolved
// here over the surface they are drawn on, because a stored alpha would be mixed with whatever
// happened to be behind it.
//
//                      dark       light
//   window             #202020    #F3F3F3   the neutral grey Settings and Explorer sit on
//   layer              #2B2B2B    #FBFBFB   the inspector, a section, a dialog
//   sunken             #1C1C1C    #F3F3F3   a block of content set into a layer
//   control            #2D2D2D    #FFFFFF   a button's body, a field
//   divider            #333333    #E0E0E0   between two rows; round a layer
//   control border     #454545    #D4D4D4   round something you press or type in
//   text               #FFFFFF    #1B1B1B
//   secondary text     #CFCFCF    #5F5F5F   labels, the quieter half of a pair
//   tertiary text      #9E9E9E    #707070   previews and captions — still AA on the window
//
// The accent is not ours at all — it is the one the user picked in Windows, in the shade Fluent
// assigns to each theme (see [`crate::theme::system_accent`]) — and it is spent sparingly: the
// trigger, the bar beside the chosen row, the one primary button, focus.
//
// **Contrast themes win.** While one is on, every role above resolves to the colour the user chose
// for it in Windows (see [`crate::theme::ContrastPalette`]) and no tint is mixed at all: a contrast
// theme is an accessibility setting, and a blend of two of its colours is a colour it did not pick.
//
// Everything below is a `fn(is_light)`, never a stored pair, so a frame can never be drawn half in
// one theme and half in the other.

static CONTRAST: std::sync::RwLock<Option<crate::theme::ContrastPalette>> =
    std::sync::RwLock::new(None);

/// The contrast theme in force, if any. A read lock and a copy: cheap enough to ask per widget.
pub fn contrast() -> Option<crate::theme::ContrastPalette> {
    CONTRAST.read().ok().and_then(|c| *c)
}

/// Re-reads whether a contrast theme is on. `true` when that changed since the last read.
pub fn refresh_contrast() -> bool {
    let now = crate::theme::contrast_palette();
    let Ok(mut stored) = CONTRAST.write() else {
        return false;
    };
    let changed = *stored != now;
    *stored = now;
    changed
}

/// Whether the contrast theme in force is a light one, so egui's own widgets pick the right half.
pub fn contrast_is_light() -> Option<bool> {
    contrast().map(|c| {
        let w = rgb(c.window);
        0.2126 * w.r() as f32 + 0.7152 * w.g() as f32 + 0.0722 * w.b() as f32 > 128.0
    })
}

fn rgb(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// Whether the hidden theme («I got chiiills») is the one being worn. Set by [`set_khipu_palette`]
/// before the palette is (re)built, read by every token: a flag rather than an argument, because
/// the drawing code asks for colours by light or dark and has no business knowing there is a third.
static KHIPU: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Puts the hidden theme's palette on, or takes it off. Takes effect at the next [`apply_theme`].
pub fn set_khipu_palette(on: bool) {
    KHIPU.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn khipu_palette() -> bool {
    KHIPU.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether «Ultra oscuro» is the one being worn: the dark palette with its surfaces on black. A
/// flag for the same reason as [`KHIPU`].
static ULTRA_DARK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn ultra_dark_palette() -> bool {
    ULTRA_DARK.load(std::sync::atomic::Ordering::Relaxed)
}

/// Puts on the palette `mode` asks for, over light or dark. Takes effect at the next
/// [`apply_theme`].
pub fn set_palette_for(mode: ThemeMode) {
    set_khipu_palette(mode == ThemeMode::Khipu);
    ULTRA_DARK.store(mode == ThemeMode::UltraDark, std::sync::atomic::Ordering::Relaxed);
}

/// A dark surface as «Ultra oscuro» draws it. The window is black, the one colour an OLED screen
/// does not light at all; every layer keeps the step it had over the window in the dark theme,
/// only smaller, so a card, a field and a chosen row are still told apart. Text, the accent and
/// the status colours are not surfaces and pass through as they are: on black they only read
/// better.
fn ultra_dark_of(dark: u32) -> u32 {
    match dark {
        0x202020 => 0x000000, // window
        0x1C1C1C => 0x000000, // sunken: set into a layer, as the window is
        0x2B2B2B => 0x121212, // layer
        0x2D2D2D => 0x1C1C1C, // control, chosen row, InfoBar
        0x323232 => 0x242424, // control under the pointer, chosen row under it
        0x272727 => 0x171717, // control pressed
        0x292929 => 0x141414, // row under the pointer
        0x333333 => 0x262626, // divider
        0x454545 => 0x3A3A3A, // border of a control
        0x442726 => 0x3A1C1B, // InfoBar that failed
        other => other,
    }
}

/// One token: its dark value, its light value, its value in the hidden theme, and what it becomes
/// under a contrast theme.
///
/// The hidden theme is a light one, so its value stands in for the light value only; and a contrast
/// theme still wins over it, as over everything — that one is the user's accessibility choice.
/// Its values come from Khipu's own palette (`assets/khipu/`), and `khipu_palette_reads` holds
/// every pair that has to stay legible to the same floor or better.
fn token(
    is_light: bool,
    dark: u32,
    light: u32,
    khipu: u32,
    contrast_role: fn(&crate::theme::ContrastPalette) -> [u8; 3],
) -> egui::Color32 {
    if let Some(c) = contrast() {
        return rgb(contrast_role(&c));
    }
    let v = match (is_light, khipu_palette()) {
        (true, true) => khipu,
        (true, false) => light,
        (false, _) if ultra_dark_palette() => ultra_dark_of(dark),
        (false, _) => dark,
    };
    hex(v)
}

fn hex(v: u32) -> egui::Color32 {
    egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The hidden theme's accent: its ink, which is also its primary button — the darkest violet in
/// Khipu's palette, with white on it.
const KHIPU_ACCENT: u32 = 0x211B37;

/// The window itself — the surface every layer sits *on*, never the layer.
pub fn win_background_for(is_light: bool) -> egui::Color32 {
    win_background(is_light)
}

fn win_background(is_light: bool) -> egui::Color32 {
    token(is_light, 0x202020, 0xF3F3F3, 0xA99BE7, |c| c.window)
}

/// A layer: the inspector, a section of a screen, a dialog.
pub fn win_card_for(is_light: bool) -> egui::Color32 {
    win_card(is_light)
}

fn win_card(is_light: bool) -> egui::Color32 {
    token(is_light, 0x2B2B2B, 0xFBFBFB, 0xDDD2F7, |c| c.window)
}

/// A block of content set *into* a layer: the full text of an expansion, a folder shown as a tag.
pub fn win_sunken_for(is_light: bool) -> egui::Color32 {
    token(is_light, 0x1C1C1C, 0xF3F3F3, 0xCBC0F2, |c| c.window)
}

/// The body of a button or field.
pub fn win_control_for(is_light: bool) -> egui::Color32 {
    win_control(is_light)
}

fn win_control(is_light: bool) -> egui::Color32 {
    token(is_light, 0x2D2D2D, 0xFFFFFF, 0xE9E3FA, |c| c.button_face)
}

/// The same control while the pointer is over it.
fn win_control_hover(is_light: bool) -> egui::Color32 {
    token(is_light, 0x323232, 0xF6F6F6, 0xE3DCF9, |c| c.button_face)
}

/// Pressed.
fn win_control_active(is_light: bool) -> egui::Color32 {
    token(is_light, 0x272727, 0xF0F0F0, 0xDCD3F6, |c| c.button_face)
}

/// The divider: between two rows, round a layer.
fn win_stroke(is_light: bool) -> egui::Color32 {
    token(is_light, 0x333333, 0xE0E0E0, 0xBBADEC, |c| c.text)
}

/// Primary text.
fn win_text(is_light: bool) -> egui::Color32 {
    token(is_light, 0xFFFFFF, 0x1B1B1B, 0x211B37, |c| c.text)
}

/// Labels, and the quieter half of a pair.
fn win_text_secondary(is_light: bool) -> egui::Color32 {
    token(is_light, 0xCFCFCF, 0x5F5F5F, 0x392F52, |c| c.text)
}

/// The user's Windows accent, in the shade this theme uses.
///
/// A registry read, so it is called only when the palette is (re)built — never from drawing code.
/// Widgets that need the accent read it back out of the palette with [`accent`], which is a field
/// access. Under a contrast theme it is the colour Windows gives links: the one of its colours
/// meant to be read as coloured text on the window.
fn read_system_accent(is_light: bool) -> egui::Color32 {
    if let Some(c) = contrast() {
        return rgb(c.hotlight);
    }
    // The hidden theme wears its own accent: a Windows blue on violet would be two brands at once.
    if is_light && khipu_palette() {
        return hex(KHIPU_ACCENT);
    }
    let (r, g, b) = crate::theme::system_accent(is_light);
    egui::Color32::from_rgb(r, g, b)
}

/// The accent colour currently in use.
///
/// [`tuned_visuals`] stores it in the palette, so every widget can read the same value for free
/// instead of each one going back to the registry. That distinction matters: this is called once
/// per row, on every frame, and a registry read in that position turns a list of eighty rows into
/// eighty registry reads sixty times a second.
pub fn accent(visuals: &egui::Visuals) -> egui::Color32 {
    visuals.hyperlink_color
}

/// A fill in the accent — the primary button, a ticked box — and the ink written on it.
pub fn accent_fill(visuals: &egui::Visuals) -> (egui::Color32, egui::Color32) {
    match contrast() {
        Some(c) => (rgb(c.highlight), rgb(c.highlight_text)),
        None => {
            let fill = accent(visuals);
            (fill, readable_on(fill))
        }
    }
}

/// Sets Windows 11's type ramp and control metrics.
///
/// Applied once at startup and never again: it only touches `text_styles` and `spacing`, which
/// `Context::set_visuals` does not overwrite — unlike the palette in [`tuned_visuals`], which has to
/// be re-applied on every theme switch.
/// Windows' «Efectos de animación», handed to what egui animates by itself: menus fading in, the
/// banner sliding, the list scrolling to a row. Off, all of it happens at once. What the app
/// animates itself — the hover wash, the switch, the saved row's tint — asks
/// [`crate::theme::animations_enabled`] directly.
pub(crate) fn apply_motion(ctx: &egui::Context) {
    let on = crate::theme::animations_enabled();
    let defaults = egui::Style::default();
    ctx.all_styles_mut(|style| {
        style.animation_time = if on { defaults.animation_time } else { 0.0 };
        style.scroll_animation = if on {
            defaults.scroll_animation
        } else {
            egui::style::ScrollAnimation::none()
        };
    });
}

pub(crate) fn apply_layout_style(ctx: &egui::Context, text_scale: f32) {
    // Windows 11's own ramp, in the same units egui measures in, rather than egui's defaults
    // multiplied by some factor:
    //
    //   Body / Button  14   the size nearly everything is set in
    //   Caption        12   counts, captions, key names — never a thing's name
    //   Subtitle       20   Semibold: the title of a screen or a dialog
    //   Monospace      13   triggers; a hair under body so they do not outweigh it
    //
    // Two weights, Regular and Semibold, as Fluent asks. Weight and colour do the separating, not
    // size; that is what keeps a hierarchy readable instead of merely small.
    let size = |points: f32| (points * text_scale).round();

    ctx.all_styles_mut(|style| {
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(size(20.0), crate::fonts::semibold_family()),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(size(14.0), FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(size(14.0), FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(size(12.0), FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(size(13.0), FontFamily::Monospace),
        );

        // Fluent's standard control is 32 tall with 11–12 of padding either side of its label.
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = crate::ui::controls::FIELD_HEIGHT;
        style.spacing.indent = 20.0;
        style.spacing.menu_margin = egui::Margin::same(4);
    });
}

/// Repaints egui's stock palette in Windows 11's own colours.
///
/// Everything here is a plain colour/number swap the renderer already had to read anyway, so it
/// costs exactly nothing per frame — no extra textures or passes.
pub fn tuned_visuals(is_light: bool) -> egui::Visuals {
    let mut v = if is_light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    let accent = read_system_accent(is_light);
    let stroke = win_stroke(is_light);
    let text = win_text(is_light);
    let text_2 = win_text_secondary(is_light);

    // Fluent's control corner. See [`crate::ui::controls::RADIUS_CONTROL`], which is where every
    // other corner in the app is decided.
    let radius = egui::CornerRadius::same(crate::ui::controls::RADIUS_CONTROL);

    v.widgets.noninteractive.bg_fill = win_card(is_light);
    v.widgets.noninteractive.weak_bg_fill = win_card(is_light);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, stroke);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text);

    v.widgets.inactive.bg_fill = win_control(is_light);
    v.widgets.inactive.weak_bg_fill = win_control(is_light);
    // Anything you can press or type in gets the firmer border: it marks the edge of a target,
    // where a divider only separates two things that are already apart.
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, line_strong(is_light));
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, text);

    v.widgets.hovered.bg_fill = win_control_hover(is_light);
    v.widgets.hovered.weak_bg_fill = win_control_hover(is_light);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, line_strong(is_light));
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, text);

    v.widgets.active.bg_fill = win_control_active(is_light);
    v.widgets.active.weak_bg_fill = win_control_active(is_light);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, stroke);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, text);

    v.widgets.open.bg_fill = win_control_hover(is_light);
    v.widgets.open.weak_bg_fill = win_control_hover(is_light);
    v.widgets.open.bg_stroke = egui::Stroke::new(1.0, line_strong(is_light));
    v.widgets.open.fg_stroke = egui::Stroke::new(1.0, text);

    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
        w.expansion = 0.0;
    }

    // egui draws a vertical rule down the left of anything indented; this design has no such line.
    // It lives in the visuals, so it has to be set here — anywhere else the next theme change wipes
    // it.
    v.indent_has_left_vline = false;

    v.panel_fill = win_background(is_light);
    v.window_fill = win_card(is_light);
    v.window_stroke = egui::Stroke::new(1.0, stroke);
    v.window_corner_radius = egui::CornerRadius::same(crate::ui::controls::RADIUS_DIALOG);
    v.menu_corner_radius = egui::CornerRadius::same(crate::ui::controls::RADIUS_DIALOG);

    // Shadows belong to what floats — a menu, a dialog, a tooltip — and to nothing else. Fluent's
    // flyout shadow is soft and low; a contrast theme gets none, since it is not a colour it chose.
    let shadow = |offset: i8, blur: u8| egui::Shadow {
        offset: [0, offset],
        blur,
        spread: 0,
        color: egui::Color32::from_black_alpha(if is_light { 36 } else { 100 }),
    };
    let (popup, window) = if contrast().is_some() {
        (egui::Shadow::NONE, egui::Shadow::NONE)
    } else {
        (shadow(4, 16), shadow(12, 32))
    };
    v.popup_shadow = popup;
    v.window_shadow = window;

    v.extreme_bg_color = win_control(is_light);
    v.text_edit_bg_color = Some(win_control(is_light));
    v.faint_bg_color = win_card(is_light);
    v.override_text_color = Some(text);
    v.weak_text_color = Some(text_2);

    match contrast() {
        Some(c) => {
            v.selection.bg_fill = rgb(c.highlight);
            v.selection.stroke = egui::Stroke::new(1.0, rgb(c.highlight_text));
        }
        None => {
            v.selection.bg_fill = accent.gamma_multiply(if is_light { 0.28 } else { 0.38 });
            v.selection.stroke = egui::Stroke::new(1.0, accent);
        }
    }
    v.hyperlink_color = accent;

    v
}

/// The line between two rows, and the outline of a layer. The one divider colour.
pub fn hairline(is_light: bool) -> egui::Color32 {
    win_stroke(is_light)
}

/// The border round something you can type in or press — firmer than a divider, because it marks
/// the edge of a *target* rather than a division between two things already separate.
pub fn line_strong(is_light: bool) -> egui::Color32 {
    token(is_light, 0x454545, 0xD4D4D4, 0x5B4878, |c| c.button_text)
}

/// Previews, captions, key names: the quietest text there is, and still legible on the window.
pub fn text_tertiary(is_light: bool) -> egui::Color32 {
    token(is_light, 0x9E9E9E, 0x707070, 0x3E3358, |c| c.text)
}

pub fn secondary_text(is_light: bool) -> egui::Color32 {
    win_text_secondary(is_light)
}

/// The outline that sets the hidden theme's option apart in Ajustes, in Khipu's violet: the
/// palette's pale violet where the card is dark, a deeper one where it is light, and its border
/// violet on the theme's own card. Each is 3:1 or more against the card it sits on.
pub fn khipu_outline(is_light: bool) -> egui::Color32 {
    token(is_light, 0xA99BE7, 0x8A78D6, 0x5B4878, |c| c.hotlight)
}

/// Text on something that cannot be used right now.
pub fn disabled_text(is_light: bool) -> egui::Color32 {
    token(is_light, 0x787878, 0xA0A0A0, 0x7A6CA8, |c| c.gray)
}

/// Applies a theme to *everything*, including the window's own title bar.
///
/// The title bar is drawn by Windows, not by us, so it follows the system setting unless the window
/// is explicitly told otherwise — which is how a dark window could end up wearing a light title bar
/// after the theme was pinned or switched. Handing winit the same theme we just gave egui keeps the
/// two halves of the window in step (it forwards this to DWM's immersive-dark-mode attribute).
pub fn apply_theme(
    ctx: &egui::Context,
    window: Option<&winit::window::Window>,
    is_light: bool,
) {
    refresh_contrast();

    // egui does not keep *a* palette — it keeps two, one for light and one for dark, and swaps
    // between them by itself whenever the host reports that the system theme changed. Writing only
    // the currently active one therefore left the other as egui's stock palette, so a Windows theme
    // switch quietly replaced every Fluent colour with egui's defaults, and re-showing the window
    // from the tray brought that stale palette back with it.
    //
    // Both are written on every apply, so whichever one egui reaches for is ours.
    ctx.set_visuals_of(egui::Theme::Light, tuned_visuals(true));
    ctx.set_visuals_of(egui::Theme::Dark, tuned_visuals(false));

    // And the choice between them is made here, not by egui: someone who pinned "Tema claro" has
    // to stay on light even while Windows is dark, which egui's own following would override.
    // A contrast theme decides by itself: its window colour is what everything is drawn against.
    let is_light = contrast_is_light().unwrap_or(is_light);
    ctx.set_theme(if is_light {
        egui::ThemePreference::Light
    } else {
        egui::ThemePreference::Dark
    });

    if let Some(window) = window {
        window.set_theme(Some(if is_light {
            winit::window::Theme::Light
        } else {
            winit::window::Theme::Dark
        }));
        // The hidden theme dresses the title bar too, in the window's own violet and ink; every
        // other theme hands it back to Windows, Mica and all. Decided on every apply, so leaving
        // the theme — or a contrast theme taking over — undoes it on the same frame.
        // «Ultra oscuro» does the same in black, or the one grey strip left would be the title bar.
        let violet = khipu_palette() && is_light && contrast().is_none();
        let black = ultra_dark_palette() && !is_light && contrast().is_none();
        set_title_bar_colours(
            window,
            (violet || black).then(|| (win_background(is_light), win_text(is_light))),
        );
    }
}

/// Paints the title bar a fixed colour, or with `None` gives it back to Windows. Windows 11 only:
/// an older one ignores both attributes and keeps the bar it always had.
fn set_title_bar_colours(window: &winit::window::Window, colours: Option<(egui::Color32, egui::Color32)>) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_COLOR_DEFAULT, DWMWA_TEXT_COLOR,
    };
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    // A COLORREF is 0x00BBGGRR.
    let colorref = |c: egui::Color32| COLORREF(u32::from_le_bytes([c.r(), c.g(), c.b(), 0]));
    let (caption, text) = match colours {
        Some((caption, text)) => (colorref(caption), colorref(text)),
        None => (COLORREF(DWMWA_COLOR_DEFAULT), COLORREF(DWMWA_COLOR_DEFAULT)),
    };
    for (attribute, value) in [(DWMWA_CAPTION_COLOR, caption), (DWMWA_TEXT_COLOR, text)] {
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                attribute,
                (&value as *const COLORREF).cast(),
                std::mem::size_of::<COLORREF>() as u32,
            );
        }
    }
}

/// Shrinks and nudges the window so it sits entirely inside `area`.
///
/// Only ever makes the window smaller or moves it back on-screen; a window that already fits is
/// left exactly where the user put it.
fn fit_window_to(window: &winit::window::Window, area: crate::display::WorkArea) {
    let scale = window.scale_factor();
    // Measured and set in the same units. This used to read the *outer* size and then set the
    // *inner* one, so every correction quietly shaved off the title bar as well.
    let inner = window.inner_size().to_logical::<f32>(scale);

    // A ceiling *and* a floor. Too big is a nuisance you can drag back; too small is not — below a
    // certain size the window has no edges left to grab and no buttons left to press, which is
    // exactly the state it came back in from the tray. On a screen too small for even the minimum,
    // the screen wins, because a window larger than its monitor is still recoverable.
    let ceiling = area.fits([inner.width.max(1.0), inner.height.max(1.0)]);
    let wanted = [
        ceiling[0].max(crate::display::MIN_SIZE[0].min(area.width)),
        ceiling[1].max(crate::display::MIN_SIZE[1].min(area.height)),
    ];
    if (wanted[0] - inner.width).abs() > 1.0 || (wanted[1] - inner.height).abs() > 1.0 {
        let _ = window.request_inner_size(winit::dpi::LogicalSize::new(wanted[0], wanted[1]));
    }

    let Ok(pos) = window.outer_position() else {
        return;
    };
    let pos = pos.to_logical::<f32>(scale);
    let x = pos.x.clamp(area.left, (area.left + area.width - wanted[0]).max(area.left));
    let y = pos.y.clamp(area.top, (area.top + area.height - wanted[1]).max(area.top));
    if (x - pos.x).abs() > 1.0 || (y - pos.y).abs() > 1.0 {
        window.set_outer_position(winit::dpi::LogicalPosition::new(x, y));
    }
}

/// The native handle behind the winit window, for the few things winit cannot do itself.
pub(crate) fn hwnd_of(window: &winit::window::Window) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(windows::Win32::Foundation::HWND(
            handle.hwnd.get() as *mut std::ffi::c_void,
        )),
        _ => None,
    }
}

/// Whether the compositor draws the window at all, independently of whether Windows counts it as
/// shown. A cloaked window that is shown puts nothing on screen, which is the one way to let
/// eframe show a window that has to stay hidden.
fn set_cloaked(window: &winit::window::Window, cloaked: bool) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };
    // A Win32 BOOL, which is an i32.
    let value = i32::from(cloaked);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            (&value as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        );
    }
}

/// Whether the title bar is wearing dark right now, read from the very attribute winit writes when
/// it themes the window (`WCA_USEDARKMODECOLORS`), through the getter twinned with winit's setter,
/// so the answer cannot drift from what winit last did. Half a microsecond, measured. `None` when
/// this Windows does not say, and then nothing can be concluded from it.
fn title_bar_is_dark(window: &winit::window::Window) -> Option<bool> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    #[repr(C)]
    struct Attribute {
        attrib: u32,
        data: *mut std::ffi::c_void,
        size: usize,
    }
    type Getter = unsafe extern "system" fn(HWND, *mut Attribute) -> windows::core::BOOL;
    const WCA_USEDARKMODECOLORS: u32 = 26;
    // Undocumented, like the setter winit relies on, so it is looked up rather than linked.
    static GETTER: std::sync::OnceLock<Option<Getter>> = std::sync::OnceLock::new();
    let getter = (*GETTER.get_or_init(|| unsafe {
        let user32 = GetModuleHandleW(windows::core::w!("user32.dll")).ok()?;
        let found = GetProcAddress(user32, windows::core::s!("GetWindowCompositionAttribute"))?;
        Some(std::mem::transmute::<unsafe extern "system" fn() -> isize, Getter>(found))
    }))?;
    let hwnd = hwnd_of(window)?;
    let mut dark = -1i32;
    let mut attribute = Attribute {
        attrib: WCA_USEDARKMODECOLORS,
        data: (&mut dark as *mut i32).cast(),
        size: std::mem::size_of::<i32>(),
    };
    if !unsafe { getter(hwnd, &mut attribute) }.as_bool() {
        return None;
    }
    match dark {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// The work area the window should be measured against: the monitor it is on, falling back to the
/// primary one only when that cannot be determined.
fn work_area_for(window: &winit::window::Window) -> Option<crate::display::WorkArea> {
    let scale = window.scale_factor() as f32;
    hwnd_of(window)
        .and_then(|hwnd| crate::display::work_area_of_window(hwnd, scale))
        .or_else(|| crate::display::work_area(scale))
}

/// Blends two colours, `t` of the way from `a` to `b`.
pub fn mix(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgb(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
    )
}

/// Black or white, whichever is readable on `background`.
///
/// The accent is the user's, not ours — it can be pale yellow or near-black, and a primary button
/// with hard-coded white text becomes unreadable the moment somebody picks the wrong one. Uses the
/// standard luminance weights rather than a plain average, because the eye is far more sensitive to
/// green than to blue.
pub fn readable_on(background: egui::Color32) -> egui::Color32 {
    let luminance = 0.2126 * background.r() as f32
        + 0.7152 * background.g() as f32
        + 0.0722 * background.b() as f32;
    if luminance > 140.0 {
        egui::Color32::from_rgb(0x10, 0x10, 0x14)
    } else {
        egui::Color32::WHITE
    }
}

/// What every tint is mixed *from*: the window, which is what the list sits on.
pub fn tint_base(is_light: bool) -> egui::Color32 {
    win_background(is_light)
}

/// The fill under a chosen row: Fluent's subtle fill, neutral rather than accented. The accent is
/// carried by the short bar beside it instead, so a trigger written in that same accent keeps its
/// contrast on the row. The `accent` argument stays for the callers that still pass it.
pub fn selection_tint(is_light: bool, _accent: egui::Color32) -> egui::Color32 {
    token(is_light, 0x2D2D2D, 0xEAEAEA, 0xC2B6F0, |c| c.window)
}

/// The wash under the row the pointer is over. Weaker than the selection, so "where my mouse is"
/// never competes with "what I have chosen".
pub fn hover_tint(is_light: bool) -> egui::Color32 {
    token(is_light, 0x292929, 0xEDEDED, 0xB6A9EC, |c| c.window)
}

/// The chosen row while the pointer is over it too.
pub fn selection_hover_tint(is_light: bool) -> egui::Color32 {
    token(is_light, 0x323232, 0xE4E4E4, 0xCABFF3, |c| c.window)
}

/// Fluent's critical colour: for errors and for what cannot be undone.
pub fn danger(is_light: bool) -> egui::Color32 {
    token(is_light, 0xFF99A4, 0xC42B1C, 0x700B1B, |c| c.text)
}

/// Fluent's caution colour — louder than a hint, quieter than an error, because nothing is wrong
/// yet. Used for "Pausado".
pub fn caution(is_light: bool) -> egui::Color32 {
    token(is_light, 0xFCE100, 0x9D5D00, 0x533100, |c| c.text)
}

/// Fluent's success colour. It means "running", and nothing else: the dot beside «Activo».
pub fn success(is_light: bool) -> egui::Color32 {
    token(is_light, 0x6CCB5F, 0x0F7B0F, 0x08430A, |c| c.text)
}

/// The InfoBar's fill when it only has news: Fluent's secondary card, a step off the page.
fn infobar_info(is_light: bool) -> egui::Color32 {
    token(is_light, 0x2D2D2D, 0xF6F6F6, 0xDDD2F7, |c| c.window)
}

/// The InfoBar's fill when something failed: Fluent's critical background, a tint and not a
/// siren, so the words on it read as plainly as anywhere else.
fn infobar_error(is_light: bool) -> egui::Color32 {
    token(is_light, 0x442726, 0xFDE7E9, 0xF0D4E4, |c| c.window)
}

pub struct KhipuApp {
    state: AppState,
    tray: Tray,
    /// Tray and menu events, delivered by the threads that wait on them rather than fetched by a
    /// timer. See [`crate::tray::spawn_event_pumps`].
    tray_events: TrayEvents,
    window: Option<Arc<winit::window::Window>>,
    last_theme_check: Instant,
    /// Listens for Windows appearance broadcasts. `None` if it could not be started, in which
    /// case `poll_theme` falls back to its timer.
    theme_watch: Option<crate::sysevents::ThemeWatch>,
    /// The search window Alt+Space opens. See [`crate::ui::search_view`].
    search: crate::ui::search_view::SearchWindow,
    /// The window that had the keyboard when the search window opened, which is where the
    /// expansion has to go back to.
    search_return_to: isize,
    /// While `now` is before this, the appearance is re-read every frame: Windows announces a
    /// change slightly before all the values behind it have settled.
    theme_recheck_until: Instant,
    is_light: bool,
    /// The accent the current palette was built from, so a change to it in Windows can be noticed
    /// without re-reading the registry from drawing code.
    last_accent: egui::Color32,
    /// Native small/large window icons are reselected when this window moves between DPI scales.
    last_icon_dpi: u32,
    start_hidden_pending: bool,
    /// Set when this frame is drawn for a window nobody can see, or one about to be put away: the
    /// round «+» is then told, after drawing, to rise again when the window opens.
    rewind_new_button: bool,
    /// Set by the tray's Quit item. The window's close handler normally cancels the close and hides
    /// to the tray instead; this is what tells it that this particular close is meant.
    quitting: bool,
    /// Holds each visible frame until the compositor's next pass. See [`crate::pacing`].
    pacer: crate::pacing::Pacer,
    /// Pause and resume picked from the tray while espanso was restarting, kept until it is back.
    held_menu: Vec<tray_icon::menu::MenuId>,
    /// The theme `sync_title_bar` last put on the title bar itself, and when.
    title_bar_set: Option<(winit::window::Theme, Instant)>,
    /// Whether the font set installed now has the Devanagari face. See
    /// [`AppState::needs_devanagari`].
    devanagari_on: bool,
}

/// How long a correction of the title bar's theme stands before `sync_title_bar` makes it again.
/// See there for why it has to be made more than once.
const TITLE_BAR_RECHECK: Duration = Duration::from_millis(250);

pub struct StartupContext {
    pub match_file: MatchFile,
    pub match_file_path: PathBuf,
    pub backups_dir: PathBuf,
    pub settings: Settings,
    pub settings_store: SettingsStore,
    pub ctl: EspansoCtl,
    pub exe_path: PathBuf,
    pub config_path: PathBuf,
    pub tray: Tray,
    pub start_hidden: bool,
    /// What already went wrong before the window even opened, so we can surface it as soon as the
    /// UI is visible instead of it being silently lost. Espanso's own failure to start goes after
    /// these, and `config_warning` after that, which is the order they happened in.
    pub startup_warnings: Vec<String>,
    pub config_warning: Option<String>,
    /// Set if the expansions file exists but could not be read. The model is the empty fallback,
    /// so saving stays refused until the app is restarted against a file it can parse.
    pub load_error: Option<String>,
    pub espanso_start: EspansoStart,
}

/// Espanso being found running, or started, on a thread of its own while the window is made.
pub struct EspansoStart {
    asking: Option<std::thread::JoinHandle<Result<(), String>>>,
    /// Why espanso could not be started, once [`Self::wait`] has the answer.
    pub failure: Option<String>,
    /// Whether espanso answered at startup. See [`AppState::espanso_confirmed`].
    pub confirmed: bool,
}

impl EspansoStart {
    pub fn spawn(ask: impl FnOnce() -> Result<(), String> + Send + 'static) -> Self {
        Self { asking: Some(std::thread::spawn(ask)), failure: None, confirmed: false }
    }

    /// Waits for the answer, the first time. Anything that restarts espanso calls this first, so
    /// that the restart never runs into a start still under way.
    pub fn wait(&mut self) -> &mut Self {
        if let Some(asking) = self.asking.take() {
            // `ensure_running` does not panic, and a panic would end the process anyway.
            let answer = asking.join().unwrap_or_else(|_| Err(String::new()));
            self.confirmed = answer.is_ok();
            self.failure = answer.err();
        }
        self
    }
}

impl KhipuApp {
    pub fn new(cc: &eframe::CreationContext<'_>, mut ctx: StartupContext) -> Self {
        refresh_contrast();
        let is_light = contrast_is_light().unwrap_or(ctx.settings.theme_mode.is_light());
        let window = cc.winit_window().cloned();
        let last_icon_dpi = window.as_deref().map_or(0, crate::icons::apply_window);
        // A size remembered on one machine travels with the folder to the next one, so a window
        // saved on a large monitor can arrive on a laptop taller than the screen. Correct it before
        // anything is drawn, and size the text to the screen that is actually here.
        let work_area = window
            .as_deref()
            .and_then(|w| crate::display::work_area(w.scale_factor() as f32))
            .or_else(|| crate::display::work_area(crate::display::system_scale()));
        if let Some(window) = window.as_deref() {
            if let Some(area) = work_area_for(window).or(work_area) {
                fit_window_to(window, area);
            }
        }
        // 1.0 when the screen cannot be measured, never more: the sizes in `apply_layout_style` are
        // Windows 11's own, so unscaled is already correct and a larger fallback would blow the
        // type up on precisely the machine we know least about.
        let text_scale = work_area.map_or(1.0, |a| crate::display::text_scale(a.height));
        apply_layout_style(&cc.egui_ctx, text_scale);
        // The preview build can draw itself at more pixels than the screen's scale gives it, for
        // images that have to stay sharp at 4K (the Store's screenshots): the whole window, type
        // and icons, rendered at that factor, not enlarged afterwards.
        if cfg!(feature = "preview") {
            if let Some(zoom) = std::env::var("EM_PREVIEW_ZOOM").ok().and_then(|z| z.parse::<f32>().ok()) {
                cc.egui_ctx.set_zoom_factor(zoom.clamp(1.0, 4.0));
            }
        }
        crate::theme::refresh_animations();
        apply_motion(&cc.egui_ctx);
        set_palette_for(ctx.settings.theme_mode);
        apply_theme(&cc.egui_ctx, window.as_deref(), is_light);
        // The title bar is the one strip of this window that Windows draws, and on Windows 11 it can
        // wear Mica — the wallpaper, blurred and tinted to the theme — as Explorer's and Settings'
        // do. Only the title bar shows it: egui paints the whole client area, opaque, over the rest.
        // Set once, because DWM keeps it on the window for life, hidden in the tray or not, and
        // tints it light or dark from the theme `apply_theme` has just given the window. Like every
        // Mica bar it goes solid while another window is active. A Windows older than build 22523
        // ignores the request, and the bar stays the solid colour it always was.
        if let Some(window) = window.as_deref() {
            use winit::platform::windows::{BackdropType, WindowExtWindows as _};
            window.set_system_backdrop(BackdropType::MainWindow);
        }
        let devanagari_on = ctx.settings.lang == Lang::Hi;
        let font_status = fonts::install(&cc.egui_ctx, devanagari_on);

        // Everything above ran while espanso was still being asked for — see `EspansoStart` — and
        // nothing below may: the shortcut can restart it, and the banner has to say if it failed.
        let espanso = ctx.espanso_start.wait();
        let mut warnings = std::mem::take(&mut ctx.startup_warnings);
        if let Some(e) = &espanso.failure {
            warnings.push(crate::i18n::fill(ctx.settings.t().startup_espanso_warning, &[("err", e)]));
        }
        warnings.extend(ctx.config_warning.take());
        let mut startup_warning = (!warnings.is_empty()).then(|| warnings.join("\n\n"));

        // Whoever owns Alt+Space also owns the search window, and only one of us can.
        //
        // With [`OWN_LAUNCHER`] off, espanso keeps the shortcut and its own bar, and this app
        // registers nothing at all — which is the one place ours could possibly get in the way.
        // With it on, espanso's bar is switched off *first* and only then is the shortcut claimed:
        // espanso holds the key while it is running, so asking for it first could only ever fail,
        // and the restart is what makes it let go. Either way the setting is written on every
        // start, so the two can never both be on.
        let wanted = if OWN_LAUNCHER { "OFF" } else { "ALT+SPACE" };
        match crate::config_patch::set_search_shortcut(&ctx.config_path, wanted) {
            Ok(true) => {
                espanso.confirmed =
                    ctx.ctl.restart_and_confirm(Duration::from_secs(6), ctx.settings.t()).is_ok();
            }
            Ok(false) => {}
            // Joins whatever main.rs already had to say, since the banner that carries it has not
            // been raised yet at this point. Silence here would mean the search bar quietly
            // belonging to nobody.
            //
            // Not added twice, though. This is the same file main.rs failed on a moment ago, so
            // when it is unreadable both attempts produce the identical paragraph, and a banner
            // that says the same thing twice reads as a bug in the banner.
            Err(e) => {
                let text = crate::i18n::fill(
                    ctx.settings.t().config_patch_warning,
                    &[("err", &e.to_string())],
                );
                startup_warning = Some(match startup_warning.take() {
                    Some(existing) if existing.contains(&text) => existing,
                    Some(existing) => format!("{existing}\n\n{text}"),
                    None => text,
                });
            }
        }
        let hotkey = OWN_LAUNCHER.then(|| {
            let chosen = ctx.settings.launcher_shortcut.as_deref();
            crate::hotkey::start(&cc.egui_ctx, chosen.and_then(crate::hotkey::Shortcut::from_config))
        });

        // Shared with Quit, which must not stop espanso under a restart that is still running.
        let reload = {
            let ctl = ctx.ctl.clone();
            let wake = cc.egui_ctx.clone();
            crate::reload::Reloader::new(
                move |t| ctl.restart_and_confirm(Duration::from_secs(6), t),
                move || wake.request_repaint(),
            )
        };
        // Built before the tray is handed over to the app: the pump has to recognise the Quit item
        // by its own id, so that it can end the program without waiting for a frame that, hidden in
        // the tray, may never come.
        let tray_events = crate::tray::spawn_event_pumps(
            &cc.egui_ctx,
            crate::tray::QuitPlan {
                id: ctx.tray.quit_id(),
                ctl: ctx.ctl.clone(),
                reload: reload.clone(),
            },
        );

        let mut state = AppState {
            match_file: ctx.match_file,
            match_file_path: ctx.match_file_path,
            backups_dir: ctx.backups_dir,
            settings: ctx.settings,
            settings_store: ctx.settings_store,
            ctl: ctx.ctl,
            view: View::List,
            settings_open: false,
            whats_new_open: false,
            whats_new_theme_before: None,
            settings_note: None,
            search: String::new(),
            banner: None,
            load_error: ctx.load_error,
            // Asked before `exe_path` is moved just below: the answer depends on which executable
            // this is, because a Run entry naming a different copy starts that one, not this one.
            autostart_enabled: autostart::is_enabled(&ctx.exe_path),
            exe_path: ctx.exe_path,
            selected: BTreeSet::new(),
            selection_anchor: None,
            paused: false,
            espanso_confirmed: ctx.espanso_start.confirmed,
            pause_toggle_requested: false,
            reload,
            reload_note: None,
            renaming_folder: None,
            pending_confirm: None,
            pending_transfer: None,
            pending_language_refresh: false,
            pending_theme_refresh: false,
            khipu_spotlight: None,
            onboarding_probe: String::new(),
            onboarding_expanded: false,
            espanso_version: None,
            espanso_version_asked: false,
            search_shortcut: hotkey
                .as_ref()
                .and_then(|watch| watch.current)
                .unwrap_or(crate::hotkey::DEFAULT),
            hotkey,
            shortcut_capture: None,
            shortcut_taken: None,
            list_revision: 0,
            list_cache: None,
            list_cache_key: None,
            devanagari_cache: Default::default(),
            dialog_owner: DialogOwner::new(window.as_deref()),
        };
        state.drop_orphan_folders();

        if let Some(warning) = startup_warning {
            state.set_error_banner(warning);
        }

        // First run gets the one screen that stands in for espanso's three-window wizard. Every
        // other run clears whatever an interrupted first run may have left behind, so its trial
        // expansion can never quietly become a permanent match nobody can see.
        // The preview build can be shown the first run again, in any saved language and theme, by
        // a file beside the exe — a real first run can only ever speak Windows' language.
        let replay = cfg!(feature = "preview")
            && std::env::current_exe()
                .ok()
                .and_then(|exe| Some(exe.parent()?.join("preview-onboarding")))
                .is_some_and(|flag| flag.exists());
        if state.settings.onboarding_done && !replay {
            state.clear_onboarding_match();
            // After an update, the few new things worth trying, once. The studio shows it only when
            // asked, so that every other capture is of the screen it was meant to be of.
            let studio = cfg!(feature = "preview");
            state.whats_new_open = if studio {
                ui::whats_new_view::studio_wants_it()
            } else {
                state.settings.whats_new_seen.as_deref() != Some(ui::whats_new_view::RELEASE)
            };
        } else {
            // Starting with Windows is on from the first run: an expander that is not running is
            // one that does nothing, and the last step of the welcome says where to turn it off.
            // A first run only — whoever already has a settings file has already chosen.
            if !replay {
                state.set_autostart(true);
            }
            state.begin_onboarding();
            state.view = View::Onboarding;
        }
        // Only ever true for somebody who has chosen Hindi: it is the one case where the face is
        // asked for at all. It used to be raised by a missing *symbols* font too, and raised
        // regardless of language — a red error about Devanagari at every start, on machines where
        // nobody had asked for it and nothing was actually wrong with what they could see.
        if font_status.devanagari_missing {
            state.set_error_banner(state.t().hindi_font_missing);
        }

        // Started in the tray after an update, the card waits behind a closed window: the dot on
        // the icon says so, until the window opens. Nothing jumps to the front by itself.
        if ctx.start_hidden && state.whats_new_open {
            ctx.tray.set_news(true, state.t());
        }

        Self {
            state,
            tray: ctx.tray,
            tray_events,
            window,
            last_theme_check: Instant::now(),
            theme_watch: crate::sysevents::ThemeWatch::start(cc.egui_ctx.clone()),
            search: Default::default(),
            search_return_to: 0,
            theme_recheck_until: Instant::now(),
            is_light,
            last_accent: read_system_accent(is_light),
            last_icon_dpi,
            start_hidden_pending: ctx.start_hidden,
            rewind_new_button: false,
            quitting: false,
            pacer: Default::default(),
            held_menu: Vec::new(),
            title_bar_set: None,
            devanagari_on,
        }
    }

    /// Applies a language change: swaps the font set (some scripts need a system font egui doesn't
    /// bundle) and re-labels the tray, neither of which `AppState` can reach on its own.
    fn apply_language_change(&mut self, ctx: &egui::Context) {
        // The font set is rebuilt, because one of its faces belongs to one language: the 5.3 MB
        // Devanagari collection is picked up when Hindi is chosen and let go again when it is not.
        // Everything else in the set is the same either way, so this is the only moment it can
        // change and the only moment the missing-font warning can become true or stop being true.
        self.devanagari_on = self.state.needs_devanagari();
        let status = fonts::install(ctx, self.devanagari_on);
        if status.devanagari_missing && self.state.settings.lang == Lang::Hi {
            self.state.set_error_banner(self.state.t().hindi_font_missing);
        }
        self.tray.relabel(self.state.t());
    }

    /// Picks the Devanagari face up the first frame something on screen needs it although the
    /// interface is not in Hindi — a month spelled in Hindi, an expansion written in it. Without
    /// it those came out as empty boxes. It is let go only with a language change: the font set is
    /// rebuilt then anyway, and rebuilding it back and forth on leaving the editor would only clear
    /// the glyph atlas for nothing.
    fn sync_devanagari(&mut self, ctx: &egui::Context) {
        if !self.devanagari_on && self.state.needs_devanagari() {
            self.devanagari_on = true;
            fonts::install(ctx, true);
            ctx.request_repaint();
        }
    }

    fn hide_to_tray(&self) {
        if let Some(window) = &self.window {
            window.set_visible(false);
        }
    }

    /// Shows the window from the tray — and makes sure it comes back somewhere it can be used.
    ///
    /// Three things can be wrong by the time this runs, none of them the user's doing: the size
    /// restored from `ui_state.ron` may have been recorded on a different machine, the position may
    /// be on a monitor that has since been unplugged, and the scale factor may have changed when
    /// the laptop was docked. Each of those on its own produces a window that is tiny, off-screen,
    /// or both — and a window with no taskbar button, in those states, is genuinely hard to get
    /// back. So the geometry is checked against the monitor it is actually on, every time, and
    /// corrected before anyone sees it.
    fn show_and_focus(&self) {
        let Some(window) = &self.window else {
            return;
        };
        // Only ever set by a start hidden in the tray; clearing both is harmless every other time.
        set_cloaked(window, false);
        self.tray.set_news(false, self.state.t());
        {
            use winit::platform::windows::WindowExtWindows as _;
            window.set_skip_taskbar(false);
        }
        window.set_visible(true);
        if let Some(area) = work_area_for(window) {
            fit_window_to(window, area);
        }
        window.focus_window();
        // winit's own focus call is `SetForegroundWindow`, which Windows ignores for a process that
        // does not own the last input event — and a click on a tray icon is input to the shell, not
        // to us. Without this the window reappears behind everything else on a busy desktop.
        if let Some(hwnd) = hwnd_of(window) {
            crate::display::bring_to_front(hwnd);
        }
    }

    /// Opens the search window when its shortcut is pressed, or closes it if it is already up, and
    /// passes what the shortcut box in Settings hears on to it.
    ///
    /// The window that had the keyboard is remembered here, before ours takes it — that is where
    /// the expansion has to end up, and by the time anything is chosen it is far too late to ask.
    fn poll_hotkey(&mut self, ctx: &egui::Context) {
        let Some(hotkey) = &self.state.hotkey else {
            return;
        };
        let mut pressed = false;
        for event in hotkey.take_events() {
            match event {
                crate::hotkey::Event::Pressed => pressed = true,
                crate::hotkey::Event::Held(mods) => {
                    if self.state.shortcut_capture.is_some() {
                        self.state.shortcut_capture = Some(mods);
                    }
                }
                crate::hotkey::Event::Captured(shortcut) => self.state.set_launcher_shortcut(shortcut),
                crate::hotkey::Event::Cancelled => self.state.shortcut_capture = None,
            }
        }
        if !pressed {
            return;
        }
        if self.search.is_open() {
            self.search.close(ctx);
            return;
        }
        let over = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        self.search_return_to = over.0 as isize;
        self.search.open(ctx, self.state.list_all(), self.state.t(), over);
    }

    /// The preview build opens the launcher for a screenshot when `preview-launcher.txt` appears
    /// beside the exe: its first line is what to have typed, its second where to put the window,
    /// `x,y` in screen pixels. Off-screen and never focused, unlike the shortcut's way in.
    #[cfg(feature = "preview")]
    fn studio_launcher(&mut self, ctx: &egui::Context) {
        let Some(file) = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("preview-launcher.txt"))) else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&file) else { return };
        let _ = std::fs::remove_file(&file);
        let mut lines = text.lines();
        let query = lines.next().unwrap_or("").trim();
        let mut xy = lines.next().unwrap_or("").split(',').filter_map(|n| n.trim().parse::<f32>().ok());
        let ppp = ctx.pixels_per_point();
        let at = egui::pos2(xy.next().unwrap_or(-3000.0) / ppp, xy.next().unwrap_or(100.0) / ppp);
        self.search.open_for_studio(ctx, self.state.list_all(), self.state.t(), query, at);
    }

    /// Keeps the search window alive, and performs the expansion once one is picked.
    ///
    /// The order matters: the window goes away, the keyboard goes back to where it came from, and
    /// only then is espanso asked to type. Espanso injects into whatever has focus at the moment it
    /// runs, so handing the focus back first is the whole trick. The window hides itself in its
    /// own pass, the one that saw the pick, so by the time this collects it it is already gone.
    fn show_search_window(&mut self, ctx: &egui::Context) {
        // Without the shortcut nothing can open it, and a window that cannot open is not worth
        // its GL surface.
        if self.state.hotkey.is_none() {
            return;
        }
        self.search.keep_alive(ctx);
        if let Some(trigger) = self.search.take_chosen() {
            self.hand_over_expansion(trigger);
        }
    }

    /// Gives the keyboard back to the window the search was opened over, and asks espanso to
    /// expand there.
    ///
    /// The trigger is typed out first, and it has to be. `RequestMatchExpansion` — the message
    /// behind `espanso match exec`, and the only expansion entry point espanso offers to anything
    /// outside itself — does not mean "type this expansion". It means "expand as though this
    /// trigger had just been typed", so espanso sends one backspace per character of the trigger
    /// to rub out what it assumes the person typed. Measured, not guessed: `:fe` eats exactly
    /// three characters and `:testing` exactly eight. Espanso's own search bar never did this
    /// because it picks a match by identity and tells its engine there was no trigger at all, but
    /// that door is internal to espanso; from out here the only way to be right is to make its
    /// assumption true. So the trigger goes in, espanso's backspaces take it out again, and what
    /// the document is left with is the replacement and nothing else — exactly what it would hold
    /// had the trigger been typed by hand. Espanso ignores injected keystrokes for its own
    /// detection, so this cannot set off a second expansion of its own.
    ///
    /// If the keyboard never actually arrives, nothing is typed and the expansion is still
    /// requested: a stray trigger left behind in someone's document is the worse outcome.
    ///
    /// The keyboard arriving and the typing being *refused* is a third case, and it goes the other
    /// way. Windows blocks injected input into a window owned by a higher-integrity process, and
    /// there the focus check passes while nothing is typed at all — so an expansion request would
    /// have espanso backspace over as many characters of the user's own document as the trigger is
    /// long. Nothing happening is the better failure.
    fn hand_over_expansion(&mut self, trigger: String) {
        let target = self.search_return_to;
        let previous = windows::Win32::Foundation::HWND(target as *mut _);
        if !previous.is_invalid() {
            crate::display::bring_to_front(previous);
        }

        let ctl = self.state.ctl.clone();
        let literal = self.state.literal_text(&trigger);
        std::thread::spawn(move || {
            let focused = target != 0
                && crate::display::wait_until_foreground(
                    target,
                    Duration::from_millis(40),
                    Duration::from_millis(600),
                );
            // Pasted whenever the text is known: typing it is what Notepad garbles. Only what
            // espanso alone can produce — scripts, forms, global vars — still goes through espanso.
            if let Some(text) = literal {
                if focused {
                    let mut pid = 0;
                    let window = windows::Win32::Foundation::HWND(target as *mut _);
                    unsafe {
                        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                            window,
                            Some(&mut pid),
                        )
                    };
                    crate::paste::paste(&text, pid);
                }
                return;
            }
            if focused && !crate::display::type_text(&trigger) {
                return;
            }
            let _ = ctl.match_exec(&trigger);
        });
    }

    fn drain_tray_events(&self) {
        loop {
            match self.tray_events.tray.try_recv() {
                Ok(TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }) => self.show_and_focus(),
                Ok(_) => {}
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return,
            }
        }
    }

    fn drain_menu_events(&mut self, ctx: &egui::Context) {
        let t = self.state.t();
        while let Ok(event) = self.tray_events.menu.try_recv() {
            if self.tray.is_quit(&event.id) {
                // Only the polite half of quitting happens here: let the close through, so eframe
                // shuts the window down in order and writes its saved size and position. Stopping
                // espanso and ending the process belong to the menu pump — see `tray::finish_quit`
                // — because those must happen whether or not this frame ever runs, and on a
                // machine hiding in the tray with nothing to repaint, it may not.
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            self.held_menu.push(event.id);
        }
        // A pause sent now would reach a daemon that is on its way out, or none at all. It waits;
        // the restart wakes a frame when it is done.
        if self.state.reload.busy() {
            return;
        }
        for id in std::mem::take(&mut self.held_menu) {
            match self.tray.handle_menu_id(&id, &self.state.ctl, t) {
                Some(Ok(())) => self.state.espanso_confirmed = true,
                Some(Err(message)) => {
                    self.state.espanso_confirmed = false;
                    self.state.set_error_banner(message);
                }
                None => {}
            }
        }
    }

    /// Keeps the window's title bar — which Windows draws, not us — matching the theme the rest of
    /// the window is using.
    ///
    /// Setting it once when the theme changes isn't enough: at startup the window is still being
    /// set up and winit applies the *system* theme to it after our call, which is how a light
    /// window could end up under a dark title bar. Comparing against what the window actually
    /// reports and correcting any mismatch fixes that case and any other.
    ///
    /// What the window reports is not what its title bar shows, though. winit only ever records the
    /// theme it picked itself — the system's, when the window is made and on every settings
    /// broadcast — and never one it is asked for, so with the app light over a dark Windows, or dark
    /// over a light one, the two never agree however often the bar is corrected. It used to be
    /// corrected on every frame, and each correction is two calls into the window manager: 0.65 ms
    /// of every frame, measured, three times what drawing the whole list takes. Now a correction
    /// stands for [`TITLE_BAR_RECHECK`] before the next one. The only thing that can undo it
    /// meanwhile is a settings broadcast, which winit answers by putting the system's theme back.
    ///
    /// And it is only made again once something has undone it: what the bar is wearing is read off
    /// the window first ([`title_bar_is_dark`]), which costs next to nothing. Where Windows will not
    /// say, the correction is remade every [`TITLE_BAR_RECHECK`] regardless, as it always was.
    fn sync_title_bar(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let wanted = if self.is_light {
            winit::window::Theme::Light
        } else {
            winit::window::Theme::Dark
        };
        if window.theme() == Some(wanted) {
            return;
        }
        if title_bar_is_dark(window) == Some(wanted == winit::window::Theme::Dark) {
            return;
        }
        let now = Instant::now();
        let fresh = self
            .title_bar_set
            .is_some_and(|(theme, at)| theme == wanted && now.duration_since(at) < TITLE_BAR_RECHECK);
        if !fresh {
            window.set_theme(Some(wanted));
            self.title_bar_set = Some((wanted, now));
        }
    }

    /// Keeps the palette in step with Windows' own appearance settings: light/dark, and the user's
    /// accent colour.
    ///
    /// Normally nothing here touches the registry at all. Windows announces both changes by
    /// broadcasting a message, and [`crate::sysevents::ThemeWatch`] is listening for it on a thread
    /// of its own; the check below is then a single atomic read, and the registry is only consulted
    /// on the frame where something actually changed. That also makes the change *immediate*
    /// instead of arriving up to two seconds late.
    ///
    /// The timer survives as a fallback for the case where that listener could not be created. It
    /// is better to update a couple of seconds late than to sit on a stale palette forever.
    fn poll_theme(&mut self, ctx: &egui::Context) {
        let now = Instant::now();

        if let Some(window) = self.window.as_deref() {
            if (window.scale_factor() * 96.0).round() as u32 != self.last_icon_dpi {
                self.last_icon_dpi = crate::icons::apply_window(window);
            }
        }

        if let Some(watch) = &self.theme_watch {
            if watch.take_changed() {
                // Windows announces the change before everything behind it has settled — the
                // accent palette in particular is written by a different component than the
                // light/dark flag. Reading once, immediately, can therefore pick up the old value
                // and then not look again for half a minute. So an announcement opens a short
                // window during which we keep re-reading.
                self.theme_recheck_until = now + Duration::from_millis(1500);
            }
        }

        if now < self.theme_recheck_until {
            // Keep frames coming while the window is open, so the re-reads actually happen.
            ctx.request_repaint_after(Duration::from_millis(150));
        } else {
            // How often to look when nothing has been announced. Two seconds if there is no
            // listener at all; a lazy half-minute when there is one, purely so a notification we
            // never received — a tool that edits the registry without telling anyone, a broadcast
            // lost while the machine was busy — repairs itself instead of leaving the palette wrong
            // until the app restarts. One registry read every 30 seconds is not a cost worth
            // optimising away, and it stops the event path being a single point of failure.
            let backstop = if self.theme_watch.is_some() {
                Duration::from_secs(30)
            } else {
                Duration::from_secs(2)
            };
            if self.last_theme_check.elapsed() < backstop {
                return;
            }
        }
        self.last_theme_check = now;

        if crate::theme::refresh_animations() {
            apply_motion(ctx);
        }

        if let Err(message) = self.tray.refresh_appearance(self.state.t()) {
            self.state.set_error_banner(message);
        }

        let mode = self.state.settings.theme_mode;
        let is_light = if mode.follows_system() {
            mode.is_light()
        } else {
            self.is_light
        };
        // A contrast theme switched on or off (or swapped for another) changes every colour at once,
        // and says nothing about light or dark through the flag read above.
        let contrast_changed = refresh_contrast();
        let is_light = contrast_is_light().unwrap_or(is_light);
        let accent_now = read_system_accent(is_light);
        if contrast_changed || is_light != self.is_light || accent_now != self.last_accent {
            self.is_light = is_light;
            self.last_accent = accent_now;
            apply_theme(ctx, self.window.as_deref(), is_light);
        }
    }

    fn show_banner(&mut self, ui: &mut egui::Ui) {
        let t = self.state.t();
        let Some(banner) = &self.state.banner else {
            return;
        };
        let (close, undo) = infobar(ui, banner, t);
        if undo {
            self.state.undo_from_banner();
        } else if close {
            self.state.banner = None;
        }
    }
}

/// One message in Windows' InfoBar, the full width of `ui`. Returns (closed, undo pressed).
///
/// Drawn at the top of the window, and inside Ajustes for what is said while it is open.
pub fn infobar(ui: &mut egui::Ui, banner: &Banner, t: &Strings) -> (bool, bool) {
    let is_light = !ui.visuals().dark_mode;
    // Windows' InfoBar: a quiet tint of the severity, the one line colour around it, and the
    // severity said twice — by the icon's colour and by the icon's shape — never by colour
    // alone. The words stay in the ordinary text colour, where they are easiest to read.
    let (fill, glyph, ink) = match banner.kind {
        BannerKind::Info => (infobar_info(is_light), ui::glyphs::Glyph::Info, accent(ui.visuals())),
        BannerKind::Error => (infobar_error(is_light), ui::glyphs::Glyph::Error, danger(is_light)),
    };
    let mut close = false;
    let mut undo = false;
    egui::Frame::default()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, hairline(is_light)))
        .corner_radius(ui::controls::RADIUS_SECTION)
        .inner_margin(egui::Margin { left: 14, right: 6, top: 6, bottom: 6 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // The buttons take their place first, and the message fills what is left of the
            // row, wrapping into as many lines as it needs.
            //
            // The obvious order was what stood here -- message, then buttons pushed to the
            // right -- and it cut long messages off at the window edge without a mark: a
            // label laid out along a horizontal row is offered unlimited width, so it never
            // wraps, and the frame just clips whatever overruns. These messages carry the
            // reason something failed, usually with Windows' own words at the end of the
            // sentence, which is exactly the part that went over the edge.
            ui.horizontal_top(|ui| {
                let (slot, _) = ui.allocate_exact_size(
                    egui::vec2(ui::glyphs::SIZE, ui::controls::FIELD_HEIGHT),
                    egui::Sense::hover(),
                );
                ui::glyphs::paint(ui, slot, glyph, ui::glyphs::SIZE, ink);
                ui.add_space(ui::controls::GAP_WIDE - ui.spacing().item_spacing.x);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    ui.spacing_mut().item_spacing.x = ui::controls::GAP_TIGHT;
                    if ui::controls::icon_button(ui, ui::glyphs::Glyph::Close, t.banner_close_tip, true)
                        .clicked()
                    {
                        close = true;
                    }
                    if banner.undo.is_some()
                        && ui::controls::subtle_button(ui, Some(ui::glyphs::Glyph::Undo), t.undo, None)
                            .clicked()
                    {
                        undo = true;
                    }
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        // Centred on the buttons' line while it is one line; from the top once
                        // it wraps, like the InfoBar's own message.
                        let line = ui.text_style_height(&egui::TextStyle::Body);
                        ui.add_space(((ui::controls::FIELD_HEIGHT - line) * 0.5).max(0.0));
                        ui.add(egui::Label::new(&banner.message).wrap());
                    });
                });
            });
        });
    (close, undo)
}

/// The preview build, opened behind everything for screenshots, takes its clicks from
/// `preview-clicks.txt` beside the exe: one `x,y` per line, in window pixels. Windows cannot
/// deliver them: off-screen, the real cursor is never over the window, every synthetic move is
/// followed by a leave, and winit drops a press that arrives without a position. Each click is
/// spread over frames as a hand would make it: move and press, release, then away.
pub(crate) fn studio_clicks(ctx: &egui::Context, raw_input: &mut egui::RawInput) {
    use std::collections::VecDeque;
    use std::sync::Mutex;
    static QUEUE: Mutex<VecDeque<Vec<egui::Event>>> = Mutex::new(VecDeque::new());

    raw_input.events.retain(|e| !matches!(e, egui::Event::PointerGone | egui::Event::PointerButton { .. }));
    let Ok(mut queue) = QUEUE.lock() else { return };
    let file = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("preview-clicks.txt")));
    let text = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
    if let (Some(file), Some(text)) = (file, text) {
        let _ = std::fs::remove_file(&file);
        let ppp = ctx.pixels_per_point();
        for line in text.lines() {
            // `t:` and the rest of the line is typed, into whatever has the focus.
            if let Some(typed) = line.strip_prefix("t:") {
                queue.push_back(vec![egui::Event::Text(typed.to_owned())]);
                continue;
            }
            // `k:Name` presses and releases one key, by egui's name for it: `k:Tab`, `k:Enter`;
            // `k:Ctrl+S` holds Ctrl through it.
            let chord = line.strip_prefix("k:").map(|name| {
                let name = name.trim();
                match name.strip_prefix("Ctrl+") {
                    // As Windows sends it: Ctrl, which egui also counts as its command key.
                    Some(rest) => (rest, egui::Modifiers::CTRL | egui::Modifiers::COMMAND),
                    None => (name, egui::Modifiers::NONE),
                }
            });
            if let Some((key, modifiers)) = chord.and_then(|(name, mods)| Some((egui::Key::from_name(name)?, mods))) {
                let event = |pressed| egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                };
                queue.push_back(vec![event(true)]);
                queue.push_back(vec![event(false)]);
                continue;
            }
            // `s:x,y,dy` turns the wheel over a point: a positive `dy` scrolls down, in pixels.
            if let Some(wheel) = line.strip_prefix("s:") {
                let n: Vec<f32> = wheel.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                if let [x, y, dy] = n[..] {
                    let pos = egui::pos2(x / ctx.pixels_per_point(), y / ctx.pixels_per_point());
                    queue.push_back(vec![egui::Event::PointerMoved(pos)]);
                    queue.push_back(vec![egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -dy / ctx.pixels_per_point()),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    }]);
                }
                continue;
            }
            let mut xy = line.split(',').filter_map(|n| n.trim().parse::<f32>().ok());
            let (Some(x), Some(y)) = (xy.next(), xy.next()) else { continue };
            let pos = egui::pos2(x / ppp, y / ppp);
            let button = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            queue.push_back(vec![egui::Event::PointerMoved(pos)]);
            queue.push_back(vec![button(true)]);
            queue.push_back(vec![button(false)]);
            queue.push_back(vec![]);
            queue.push_back(vec![]);
            queue.push_back(vec![egui::Event::PointerMoved(egui::pos2(2.0, raw_input.screen_rect.map_or(2.0, |r| r.max.y - 2.0)))]);
        }
    }
    if let Some(step) = queue.pop_front() {
        raw_input.events.extend(step);
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(50));
}

impl eframe::App for KhipuApp {
    /// The main window's focus comes from Windows on every frame, not from the last notice winit
    /// passed on: one lost notice used to leave every text box without its caret and highlight
    /// until the window was hidden and opened again. See [`crate::display::has_keyboard_focus`].
    /// The launcher's viewport keeps winit's answer: it is a window of its own, with its own focus.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if raw_input.viewport_id != egui::ViewportId::ROOT {
            return;
        }
        if cfg!(feature = "preview") && std::env::var_os("EM_PREVIEW_BACKGROUND").is_some() {
            studio_clicks(ctx, raw_input);
        }
        let hwnd = self.window.as_deref().and_then(hwnd_of);
        if let Some(focused) = hwnd.and_then(crate::display::has_keyboard_focus) {
            raw_input.focused = focused;
        }
        // The studio is never brought to the front, so without this no capture could show what a
        // focused field looks like.
        if cfg!(feature = "preview") && std::env::var_os("EM_PREVIEW_FOCUSED").is_some() {
            raw_input.focused = true;
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.window.as_ref().and_then(|w| w.is_visible()).unwrap_or(true) {
            self.pacer.wait_for_compositor();
        } else {
            self.rewind_new_button = true;
        }

        // `with_visible(false)` in main.rs is not enough on its own: eframe shows the root window
        // after painting its first frame whatever the builder said (`post_rendering`, eframe
        // 0.36.1), which is after this runs. So the window is cloaked before that frame reaches
        // the screen, and the hide goes out as a viewport command, which eframe applies after its
        // own show. Hiding it directly here, as this used to, was undone a moment later: starting
        // with Windows opened the window at every login.
        if self.start_hidden_pending {
            self.start_hidden_pending = false;
            self.rewind_new_button = true;
            if let Some(window) = &self.window {
                set_cloaked(window, true);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        self.drain_tray_events();
        self.drain_menu_events(ctx);
        self.poll_hotkey(ctx);
        #[cfg(feature = "preview")]
        self.studio_launcher(ctx);
        self.show_search_window(ctx);

        self.state.collect_reload();
        // The end of a timed pause waits for a restart in flight, like every other pause.
        if !self.state.reload.busy() {
            match self.tray.tick(&self.state.ctl, self.state.t()) {
                Some(Ok(())) => self.state.espanso_confirmed = true,
                Some(Err(message)) => {
                    self.state.espanso_confirmed = false;
                    self.state.set_error_banner(message);
                }
                None => {}
            }
        }

        // Settings changes that need more than `AppState` to take effect are applied here, once,
        // rather than from inside the settings screen's own rendering.
        if std::mem::take(&mut self.state.pending_language_refresh) {
            self.apply_language_change(ctx);
        }
        self.sync_devanagari(ctx);
        if std::mem::take(&mut self.state.pending_theme_refresh) {
            set_palette_for(self.state.settings.theme_mode);
            refresh_contrast();
            self.is_light = contrast_is_light().unwrap_or(self.state.settings.theme_mode.is_light());
            self.last_accent = read_system_accent(self.is_light);
            apply_theme(ctx, self.window.as_deref(), self.is_light);
        }

        self.poll_theme(ctx);
        self.sync_title_bar();

        // The folder picker is a modal too, and it opens from Ajustes rather than from the list,
        // so its Esc cannot live inside the list-only block below.
        // Each Esc that closes a dialog is spent on it: left in the queue, the list would read it
        // as well and close the inspector behind the dialog in the same keystroke.
        let esc = |ctx: &egui::Context| {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        };
        if self.state.pending_transfer.is_some() && esc(ctx) {
            self.state.cancel_pending_transfer();
        }

        // Esc backs out of a pending confirmation. The rest of what Esc means on the list — the
        // search, then the inspector, then the picking — is the list's own (`list_view::keyboard`),
        // and in the editor it is Cancelar. The confirmation is drawn over every screen, so this is.
        // egui reports it as a physical key, so it works the same on a keyboard that prints "Esc",
        // "Escape" or nothing at all.
        if self.state.pending_confirm.is_some() && esc(ctx) {
            self.state.cancel_pending_confirm();
        }
        // Ajustes last: the picker it opens sits in front of it and has already had its Esc. An
        // open drop-down (the language) takes its own Esc and closes, and Ajustes stays.
        if self.state.settings_open && !egui::Popup::is_any_open(ctx) && esc(ctx) {
            self.state.close_settings();
        }

        // The X hides to the tray rather than quitting — except when the tray's own Quit item is
        // what asked for the close, which is the one way out of the program.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && !crate::EXPERIMENTAL {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_to_tray();
            self.rewind_new_button = true;
        }

        // Hidden in the tray, the app now schedules nothing at all. Everything that used to need
        // finding out about arrives on its own: a tray click and a menu pick wake it through
        // `spawn_event_pumps`, a Windows appearance change through `sysevents`. The one exception is
        // a ten-minute pause, which ends at a time rather than on an event, so that alone books a
        // frame — and books it for the moment it is actually due.
        //
        // With the window open the slow heartbeat stays. A forced repaint there re-lays-out the
        // whole current view, so it is the expensive case, and two seconds is already lazy.
        let is_visible = self.window.as_ref().and_then(|w| w.is_visible()).unwrap_or(true);
        if is_visible {
            // Windows can leave this thread with no keyboard focus while the window is still
            // visible and active, and then nothing typed reaches the text box the person just
            // clicked. `restore_keyboard_focus` explains what that state is and why clicking
            // cannot get out of it; the reason it is checked from here, every frame the window is
            // up, is that the repair has to arrive without being asked for. It costs one syscall,
            // so with the window idle it lands on the heartbeat below, and the instant anything is
            // clicked or typed it lands on that frame instead.
            if let Some(hwnd) = self.window.as_deref().and_then(hwnd_of) {
                crate::display::restore_keyboard_focus(hwnd);
            }
            ctx.request_repaint_after(VISIBLE_POLL_INTERVAL);
        } else if let Some(until) = self.tray.timed_pause_deadline() {
            ctx.request_repaint_after(until.saturating_duration_since(Instant::now()));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // `show_collapsible` slides the panel in/out and correctly shrinks the space given to
        // everything below it as it animates, so the banner appearing or disappearing never
        // overlaps the rest of the interface — it always pushes it, never covers it.
        if self.state.banner.is_some() {
        let gutter = ui::controls::page_margin(ui.available_width());
        egui::Panel::top("banner")
            .show_separator_line(false)
            .frame(
                egui::Frame::default()
                    .fill(win_background(self.is_light))
                    .inner_margin(egui::Margin {
                        left: gutter,
                        right: gutter,
                        top: gutter.saturating_sub(4),
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                self.show_banner(ui);
            });
        }

        // The pause state belongs to the tray; the library only shows it. Copied in before the
        // frame is drawn and the request read back after, so a view never holds the tray and never
        // runs `espanso` itself. See [`AppState::paused`].
        self.state.paused = self.tray.is_paused();

        // The gutter is the first thing that gives on a narrow window — before any control is
        // allowed to shrink. See [`ui::controls::page_margin`].
        let gutter = ui::controls::page_margin(ui.available_width());
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(win_background(self.is_light))
                    .inner_margin(egui::Margin::symmetric(gutter, gutter.saturating_sub(4))),
            )
            .show(ui, |ui| match &self.state.view {
            View::List => ui::list_view::show(ui, &mut self.state),
            View::Edit(_) => ui::edit_form::show(ui, &mut self.state),
            View::Tips => ui::tips_view::show(ui, &mut self.state),
            View::Onboarding => ui::onboarding_view::show(ui, &mut self.state),
            View::FolderOptions(_) => ui::folder_view::show(ui, &mut self.state),
        });

        ui::list_view::show_move_modal(ui.ctx(), &mut self.state);
        ui::list_view::show_create_folder_modal(ui.ctx(), &mut self.state);
        ui::list_view::show_pending_confirm(ui.ctx(), &mut self.state);
        // Before the folder picker, which opens from it and has to be the one on top.
        ui::whats_new_view::show(ui.ctx(), &mut self.state);
        ui::settings_view::show(ui.ctx(), &mut self.state);
        ui::settings_view::show_folder_picker(ui.ctx(), &mut self.state);
        ui::glide::end_frame(ui.ctx());
        if std::mem::take(&mut self.rewind_new_button) {
            ui::list_view::rewind_new_button(ui.ctx());
        }

        // Left set while espanso restarts: see `drain_menu_events`.
        if !self.state.reload.busy() && std::mem::take(&mut self.state.pause_toggle_requested) {
            let t = self.state.t();
            let toggled = self.tray.toggle(&self.state.ctl, t);
            self.state.espanso_confirmed = toggled.is_ok();
            if let Err(e) = toggled {
                self.state.set_error_banner(e);
            }
            self.state.paused = self.tray.is_paused();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG's contrast ratio between two colours.
    fn contrast_ratio(a: egui::Color32, b: egui::Color32) -> f32 {
        let lum = |c: egui::Color32| {
            let ch = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// Every pair of the hidden theme that somebody has to read, held to the floors of Khipu's own
    /// palette (`planes/tema-oculto-paleta.png`) — 6.68 for text on the window, 5.01 for the
    /// quieter text, 14.96 for the primary button , 5.5 for a field's border — and to WCAG's 4.5 for every other piece of
    /// text, 3 for an outline. A colour nudged later cannot quietly go under.
    ///
    /// The palette is read through the real tokens, with the theme's flag up for the length of the
    /// test only. No other test reads a colour.
    #[test]
    fn khipu_palette_reads() {
        set_khipu_palette(true);
        let window = win_background(true);
        let card = win_card(true);
        let control = win_control(true);
        let text = win_text(true);
        let accent = read_system_accent(true);
        let pairs = [
            ("text / window", text, window, 6.68),
            ("text / card", text, card, 6.68),
            ("secondary / window", win_text_secondary(true), window, 5.01),
            ("secondary / card", win_text_secondary(true), card, 5.01),
            ("tertiary / window", text_tertiary(true), window, 4.5),
            ("tertiary / selection", text_tertiary(true), selection_tint(true, accent), 4.5),
            ("secondary / hover", win_text_secondary(true), hover_tint(true), 4.5),
            ("secondary / chosen + hover", win_text_secondary(true), selection_hover_tint(true), 4.5),
            ("text / field", text, control, 6.68),
            ("button ink / button", readable_on(accent), accent, 14.96),
            ("field border / field", line_strong(true), control, 5.5),
            ("field border / card", line_strong(true), card, 5.5),
            ("danger / window", danger(true), window, 4.5),
            ("danger / error bar", danger(true), infobar_error(true), 4.5),
            ("caution / window", caution(true), window, 4.5),
            ("success / window", success(true), window, 4.5),
            ("outline / card", khipu_outline(true), card, 3.0),
        ];
        set_khipu_palette(false);
        let outlines = [
            ("light outline / card", khipu_outline_off(true), 0xFBFBFB, 3.0),
            ("dark outline / card", khipu_outline_off(false), 0x2B2B2B, 3.0),
        ];
        for (name, fg, bg, floor) in pairs {
            let ratio = contrast_ratio(fg, bg);
            assert!(ratio >= floor, "{name}: {ratio:.2} < {floor}");
        }
        for (name, fg, bg, floor) in outlines {
            let ratio = contrast_ratio(fg, hex(bg));
            assert!(ratio >= floor, "{name}: {ratio:.2} < {floor}");
        }
    }

    /// The outline as the other two themes draw it, read once the flag is down again.
    fn khipu_outline_off(is_light: bool) -> egui::Color32 {
        token(is_light, 0xA99BE7, 0x8A78D6, 0x5B4878, |c| c.hotlight)
    }
}
