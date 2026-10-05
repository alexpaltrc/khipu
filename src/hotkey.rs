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

//! The global shortcut that opens the search window, and the capture that changes it.
//!
//! Espanso has a search bar of its own on the same shortcut, and it is good — but it is espanso's
//! window, drawn by espanso's toolkit, and there is no seam to reach into. So this app takes the
//! shortcut over and draws its own, then hands the chosen expansion back to espanso to actually
//! insert (see [`crate::espanso_ctl::EspansoCtl::match_exec`]). We own the face; espanso keeps the
//! part that is genuinely hard.
//!
//! **One thread owns everything here.** Windows delivers a hot key to the thread that registered
//! it, and calls a low-level hook on the thread that installed it, so the registration, the mouse
//! hook and the capture hooks all live on one thread parked in `GetMessageW`, costing nothing at
//! rest. The interface talks to it through [`HotkeyWatch`]: commands go in through a channel and
//! a posted message that wakes the thread; presses and capture progress come back as [`Event`]s,
//! each with a repaint so the frame loop sees it at once.
//!
//! **A shortcut is a key or a mouse button, with modifiers.** A key is registered with
//! `RegisterHotKey`, which is first come, first served: if another program owns it, that is said,
//! and the shortcut that worked before stays. Windows has no such registration for mouse buttons,
//! so those are watched by a `WH_MOUSE_LL` hook, installed only while a button is the shortcut.
//!
//! **Capturing swallows the keys.** While the Settings box is listening, a keyboard hook takes
//! every key before anything else sees it — Alt+Space does not open anything, Win does not open
//! Start — and keeps swallowing until the last key it took has been let go, so no window is left
//! holding the release of a press it never saw.

use eframe::egui;
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use crate::i18n::Lang;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_NONAME;

/// Arbitrary; only has to be unique within this process.
const HOTKEY_ID: i32 = 0xE59A;

/// `RegisterHotKey`'s modifier bits, which are also what [`Shortcut::mods`] holds.
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;

const VK_SPACE: u32 = 0x20;
const VK_ESCAPE: u32 = 0x1B;

/// Posted to the thread to say a command is waiting in its channel.
const WM_COMMAND_READY: u32 = 0x8000 + 0x51; // WM_APP + n
/// Posted by the mouse hook: the shortcut's button was pressed. Handled in the loop rather than in
/// the hook, so the hook returns at once.
const WM_MOUSE_PRESSED: u32 = 0x8000 + 0x52;
/// Posted by the capture hooks once the capture is over and every key it took is released.
const WM_CAPTURE_DRAINED: u32 = 0x8000 + 0x53;

/// The shortcut the app starts with, and the one the reset button goes back to.
pub const DEFAULT: Shortcut = Shortcut { mods: MOD_ALT, input: Input::Key(VK_SPACE) };

/// Tried in order when the shortcut is the default one.
///
/// Alt+Space is popular — PowerToys Run and most launcher apps take it by default. Rather than
/// losing the window to whoever booted first, the second choice adds Shift, which is nearly always
/// free. Whichever one is won gets *named* in the tips screen, so the app never tells anybody to
/// press something that does nothing. A shortcut somebody chose has no second choice: if it is
/// taken, that is what they are told.
pub const CANDIDATES: [Shortcut; 2] =
    [DEFAULT, Shortcut { mods: MOD_ALT | MOD_SHIFT, input: Input::Key(VK_SPACE) }];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Middle,
    X1,
    X2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    /// A virtual-key code.
    Key(u32),
    Mouse(Button),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shortcut {
    /// `MOD_*` bits.
    pub mods: u32,
    pub input: Input,
}

impl Shortcut {
    /// How `settings.json` keeps it: "Alt+Space", "Ctrl+Mouse4", "F13". Always English, so the
    /// file means the same thing whatever the interface language was when it was written.
    pub fn to_config(self) -> String {
        let mut out = mods_text(self.mods, Lang::En);
        out.push_str(&match self.input {
            Input::Mouse(Button::Middle) => "Mouse3".to_string(),
            Input::Mouse(Button::X1) => "Mouse4".to_string(),
            Input::Mouse(Button::X2) => "Mouse5".to_string(),
            Input::Key(VK_SPACE) => "Space".to_string(),
            Input::Key(vk) if is_letter_or_digit(vk) => char::from(vk as u8).to_string(),
            Input::Key(vk) if is_function_key(vk) => format!("F{}", vk - 0x6F),
            Input::Key(vk) => format!("VK{vk:02X}"),
        });
        out
    }

    /// Reads back what [`to_config`](Self::to_config) wrote. Anything else is `None`, and the app
    /// then starts with the default rather than with something half understood.
    pub fn from_config(text: &str) -> Option<Shortcut> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let last = parts.pop()?;
        let mut mods = 0;
        for part in parts {
            mods |= match part {
                "Ctrl" => MOD_CONTROL,
                "Shift" => MOD_SHIFT,
                "Alt" => MOD_ALT,
                "Win" => MOD_WIN,
                _ => return None,
            };
        }
        let input = match last {
            "Mouse3" => Input::Mouse(Button::Middle),
            "Mouse4" => Input::Mouse(Button::X1),
            "Mouse5" => Input::Mouse(Button::X2),
            "Space" => Input::Key(VK_SPACE),
            _ if last.len() == 1 && is_letter_or_digit(u32::from(last.as_bytes()[0])) => {
                Input::Key(u32::from(last.as_bytes()[0]))
            }
            _ if last.starts_with('F') => {
                let n: u32 = last[1..].parse().ok()?;
                (1..=24).contains(&n).then_some(Input::Key(0x6F + n))?
            }
            _ if last.starts_with("VK") => {
                Input::Key(u32::from_str_radix(&last[2..], 16).ok().filter(|vk| *vk < 0x100)?)
            }
            _ => return None,
        };
        let shortcut = Shortcut { mods, input };
        shortcut.is_usable().then_some(shortcut)
    }

    /// Written the way Windows writes it in menus: "Alt+Espacio", "Ctrl+Ratón 4".
    pub fn label(self, lang: Lang) -> String {
        let mut out = mods_text(self.mods, lang);
        out.push_str(&input_name(self.input, lang));
        out
    }

    /// Whether this is something a person can be given as a shortcut without it taking away a key
    /// they type with.
    ///
    /// A key that writes something — a letter, a digit, Space, punctuation — needs Ctrl, Alt or
    /// Win with it: on its own, or with only Shift, it would stop typing that character anywhere
    /// in Windows. Keys that write nothing (F1 to F24, the media and browser keys, Pause) and the
    /// extra mouse buttons can stand alone. Modifiers, Esc, Enter, Tab and Backspace never can be
    /// the key.
    pub fn is_usable(self) -> bool {
        match self.input {
            Input::Mouse(_) => true,
            Input::Key(vk) => {
                if is_modifier(vk) || matches!(vk, VK_ESCAPE | 0x0D | 0x09 | 0x08) || vk < 0x08 {
                    false
                } else if writes_nothing(vk) {
                    true
                } else {
                    self.mods & (MOD_CONTROL | MOD_ALT | MOD_WIN) != 0
                }
            }
        }
    }
}

/// "Ctrl+Mayús+…", in the order the rest of the app writes them, with Win first, as Windows does.
pub fn mods_text(mods: u32, lang: Lang) -> String {
    let mut out = String::new();
    if mods & MOD_WIN != 0 {
        out.push_str("Win+");
    }
    if mods & MOD_CONTROL != 0 {
        out.push_str("Ctrl+");
    }
    if mods & MOD_SHIFT != 0 {
        out.push_str(if matches!(lang, Lang::Es) { "Mayús+" } else { "Shift+" });
    }
    if mods & MOD_ALT != 0 {
        out.push_str("Alt+");
    }
    out
}

fn is_letter_or_digit(vk: u32) -> bool {
    (0x30..=0x39).contains(&vk) || (0x41..=0x5A).contains(&vk)
}

fn is_function_key(vk: u32) -> bool {
    (0x70..=0x87).contains(&vk)
}

/// Shift, Ctrl, Alt and Win, left, right or either.
fn is_modifier(vk: u32) -> bool {
    matches!(vk, 0x10..=0x12 | 0xA0..=0xA5 | 0x5B | 0x5C)
}

/// The `MOD_*` bit a modifier key stands for, or 0.
fn modifier_bit(vk: u32) -> u32 {
    match vk {
        0x10 | 0xA0 | 0xA1 => MOD_SHIFT,
        0x11 | 0xA2 | 0xA3 => MOD_CONTROL,
        0x12 | 0xA4 | 0xA5 => MOD_ALT,
        0x5B | 0x5C => MOD_WIN,
        _ => 0,
    }
}

/// Keys that type nothing, and so can be a shortcut on their own: F1–F24, Pause, Scroll Lock,
/// Print Screen and the browser, media and launch keys.
fn writes_nothing(vk: u32) -> bool {
    is_function_key(vk) || matches!(vk, 0x13 | 0x91 | 0x2C | 0xA6..=0xB7)
}

fn input_name(input: Input, lang: Lang) -> String {
    let es = matches!(lang, Lang::Es);
    let pick = |en: &str, es_text: &str| if es { es_text } else { en }.to_string();
    match input {
        Input::Mouse(Button::Middle) => pick("Middle click", "Clic central"),
        Input::Mouse(Button::X1) => pick("Mouse 4", "Ratón 4"),
        Input::Mouse(Button::X2) => pick("Mouse 5", "Ratón 5"),
        Input::Key(vk) => match vk {
            VK_SPACE => pick("Space", "Espacio"),
            _ if is_letter_or_digit(vk) => char::from(vk as u8).to_string(),
            _ if is_function_key(vk) => format!("F{}", vk - 0x6F),
            0x13 => pick("Pause", "Pausa"),
            0x91 => pick("Scroll Lock", "Bloq Despl"),
            0x2C => pick("Print Screen", "Impr Pant"),
            0x2D => pick("Insert", "Insert"),
            0x2E => pick("Del", "Supr"),
            0x24 => pick("Home", "Inicio"),
            0x23 => pick("End", "Fin"),
            0x21 => pick("Page Up", "Re Pág"),
            0x22 => pick("Page Down", "Av Pág"),
            0x25 => "←".to_string(),
            0x26 => "↑".to_string(),
            0x27 => "→".to_string(),
            0x28 => "↓".to_string(),
            0x60..=0x69 => format!("Num {}", vk - 0x60),
            0xA6 => pick("Back", "Atrás"),
            0xA7 => pick("Forward", "Adelante"),
            0xA8 => pick("Refresh", "Actualizar"),
            0xA9 => pick("Stop", "Detener"),
            0xAA => pick("Search", "Buscar"),
            0xAB => pick("Favorites", "Favoritos"),
            0xAC => pick("Home page", "Página de inicio"),
            0xAD => pick("Mute", "Silenciar"),
            0xAE => pick("Volume down", "Bajar volumen"),
            0xAF => pick("Volume up", "Subir volumen"),
            0xB0 => pick("Next track", "Pista siguiente"),
            0xB1 => pick("Previous track", "Pista anterior"),
            0xB2 => pick("Stop media", "Detener reproducción"),
            0xB3 => pick("Play/Pause", "Reproducir/Pausar"),
            0xB4 => pick("Mail", "Correo"),
            0xB5 => pick("Media", "Multimedia"),
            0xB6 => pick("App 1", "Aplicación 1"),
            0xB7 => pick("App 2", "Aplicación 2"),
            _ => layout_key_name(vk).unwrap_or_else(|| format!("0x{vk:02X}")),
        },
    }
}

/// What the keyboard layout calls a key: for punctuation, the character on it ("Ñ", "´").
fn layout_key_name(vk: u32) -> Option<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyNameTextW, MapVirtualKeyW, MAPVK_VK_TO_VSC};
    let scan = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) };
    if scan == 0 {
        return None;
    }
    let mut buffer = [0u16; 64];
    let len = unsafe { GetKeyNameTextW((scan << 16) as i32, &mut buffer) };
    (len > 0).then(|| String::from_utf16_lossy(&buffer[..len as usize]))
}

/// What the thread reports.
pub enum Event {
    /// The shortcut was pressed.
    Pressed,
    /// While capturing: the modifiers held right now, to show as they are pressed.
    Held(u32),
    /// The capture is over, with this.
    Captured(Shortcut),
    /// The capture is over, with nothing: Esc.
    Cancelled,
}

enum Command {
    /// Claim the first of these that is free, giving up whatever is held now; if none is, keep
    /// what is held now. Replies with the one won.
    Claim(Vec<Shortcut>, Sender<Option<Shortcut>>),
    BeginCapture,
    EndCapture,
}

pub struct HotkeyWatch {
    events: Receiver<Event>,
    commands: Sender<Command>,
    thread: u32,
    /// What actually opens the window now, if anything does.
    pub current: Option<Shortcut>,
}

impl HotkeyWatch {
    /// Everything that has happened since the last time this was asked.
    pub fn take_events(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    /// Makes the first of `wanted` that is free the shortcut, and says which one that was. `None`
    /// means none was, and the one held before is still the shortcut.
    pub fn claim(&mut self, wanted: &[Shortcut]) -> Option<Shortcut> {
        let (reply, answer) = std::sync::mpsc::channel();
        self.send(Command::Claim(wanted.to_vec(), reply));
        // The thread answers in well under a millisecond; the wait is bounded only so that a
        // thread that somehow died cannot hang the interface.
        let won = answer.recv_timeout(Duration::from_secs(1)).ok().flatten();
        if won.is_some() {
            self.current = won;
        }
        won
    }

    pub fn begin_capture(&self) {
        self.send(Command::BeginCapture);
    }

    pub fn end_capture(&self) {
        self.send(Command::EndCapture);
    }

    fn send(&self, command: Command) {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
        if self.commands.send(command).is_ok() {
            let _ = unsafe { PostThreadMessageW(self.thread, WM_COMMAND_READY, WPARAM(0), LPARAM(0)) };
        }
    }
}

/// Stops the window underneath from reading our shortcut as a bare tap of its modifier.
///
/// Alt+Space is a chord, but the window that has the keyboard only ever sees half of it:
/// `RegisterHotKey` swallows the Space, so from that window's point of view Alt went down and, a
/// moment later, Alt came up with nothing at all in between. That is the definition of "Alt was
/// tapped", and a Windows app answers it by activating its menu bar. From then on everything
/// espanso types into that window is read as a menu shortcut instead of as text — in Notepad the
/// letter "e" is the settings gear, which is exactly why picking almost any expansion opened
/// Notepad's settings and typed nothing at all. Win does the same with the Start menu.
///
/// One keystroke is enough to fix it, sent while the modifier is still held: the chord stops being
/// "Alt on its own", so the release means nothing. `VK_NONAME` is Microsoft's own reserved no-op
/// code — no keyboard can produce it, so nothing has a reason to be listening for it.
fn break_modifier_tap() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    };

    let event = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_NONAME,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let events = [event(KEYBD_EVENT_FLAGS(0)), event(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&events, std::mem::size_of::<INPUT>() as i32) };
}

/// A capture in progress, or finished and waiting for its keys to come up.
struct Capture {
    held: u32,
    /// Keys whose press was swallowed, so their release must be too. One bit per virtual key.
    swallowed: [u64; 4],
    /// The same for the one mouse button that ended the capture.
    swallowed_button: Option<Button>,
    done: bool,
}

impl Capture {
    fn drained(&self) -> bool {
        self.swallowed == [0; 4] && self.swallowed_button.is_none()
    }
}

/// What the hooks need, which they cannot be handed: a hook procedure is a bare function. Only the
/// thread that owns the hooks ever touches it, since that is the thread Windows calls them on.
struct HookState {
    thread: u32,
    events: Sender<Event>,
    ctx: egui::Context,
    /// The shortcut's button, when the shortcut is a mouse button.
    trigger: Option<(u32, Button)>,
    /// The shortcut's button went down and was swallowed; its release must be too.
    trigger_down: bool,
    capture: Option<Capture>,
}

thread_local! {
    static HOOK: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

fn tell_to(events: &Sender<Event>, ctx: &egui::Context, event: Event) {
    let _ = events.send(event);
    ctx.request_repaint();
}

fn post_to(thread: u32, message: u32) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;
    let _ = unsafe { PostThreadMessageW(thread, message, WPARAM(0), LPARAM(0)) };
}

fn bit(swallowed: &mut [u64; 4], vk: u32, on: bool) -> bool {
    let (word, mask) = ((vk as usize >> 6) & 3, 1u64 << (vk & 63));
    let was = swallowed[word] & mask != 0;
    if on {
        swallowed[word] |= mask;
    } else {
        swallowed[word] &= !mask;
    }
    was
}

/// The modifiers held right now, read from the keyboard's state.
fn held_now() -> u32 {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    let down = |vk: i32| unsafe { GetAsyncKeyState(vk) } < 0;
    let mut mods = 0;
    if down(0x11) {
        mods |= MOD_CONTROL;
    }
    if down(0x10) {
        mods |= MOD_SHIFT;
    }
    if down(0x12) {
        mods |= MOD_ALT;
    }
    if down(0x5B) || down(0x5C) {
        mods |= MOD_WIN;
    }
    mods
}

unsafe extern "system" fn keyboard_hook(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, KBDLLHOOKSTRUCT, LLKHF_UP};

    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let up = info.flags.0 & LLKHF_UP.0 != 0;
        // Keys sent by a program count the same as keys pressed by hand, on purpose: the software
        // behind a mouse or keyboard with extra buttons makes those buttons send a key, F13 or the
        // like, and that is exactly what somebody setting this shortcut is pressing. Only our own
        // no-op key is left out (see [`break_modifier_tap`]).
        let swallow = info.vkCode != u32::from(VK_NONAME.0)
            && HOOK.with_borrow_mut(|state| {
                let Some(state) = state.as_mut() else { return false };
                let HookState { thread, events, ctx, capture, .. } = state;
                let Some(capture) = capture.as_mut() else { return false };
                let tell = |event| tell_to(events, ctx, event);
                let post = |message| post_to(*thread, message);
                let vk = info.vkCode & 0xFF;
                if up {
                    // Only what this capture took the press of: anything else was already down
                    // before it began, and its owner is waiting for the release.
                    let was = bit(&mut capture.swallowed, vk, false);
                    if was && !capture.done {
                        capture.held &= !modifier_bit(vk);
                        let held = capture.held;
                        tell(Event::Held(held));
                    }
                    if was && capture.done && capture.drained() {
                        post(WM_CAPTURE_DRAINED);
                    }
                    return was;
                }
                if capture.done {
                    // Over, and only waiting for releases; a new press is somebody typing again.
                    return false;
                }
                bit(&mut capture.swallowed, vk, true);
                if is_modifier(vk) {
                    capture.held |= modifier_bit(vk);
                    let held = capture.held;
                    tell(Event::Held(held));
                } else if vk == VK_ESCAPE {
                    capture.done = true;
                    tell(Event::Cancelled);
                } else {
                    let shortcut = Shortcut { mods: capture.held, input: Input::Key(vk) };
                    // Something that would take away a key people type with is not taken; the
                    // box goes on listening.
                    if shortcut.is_usable() {
                        capture.done = true;
                        tell(Event::Captured(shortcut));
                    }
                }
                true
            });
        if swallow {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_hook(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, MSLLHOOKSTRUCT, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_XBUTTONDOWN,
        WM_XBUTTONUP, XBUTTON1,
    };

    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let x_button = if (info.mouseData >> 16) as u16 == XBUTTON1 { Button::X1 } else { Button::X2 };
        let message = wparam.0 as u32;
        let (button, up) = match message {
            WM_MBUTTONDOWN => (Some(Button::Middle), false),
            WM_MBUTTONUP => (Some(Button::Middle), true),
            WM_XBUTTONDOWN => (Some(x_button), false),
            WM_XBUTTONUP => (Some(x_button), true),
            // Left and right clicks, the wheel and movement always go through: clicking anywhere
            // else is how a capture is abandoned.
            _ => (None, false),
        };
        let swallow = match button {
            Some(button) => HOOK.with_borrow_mut(|state| {
                let Some(state) = state.as_mut() else { return false };
                let HookState { thread, events, ctx, trigger, trigger_down, capture } = state;
                let tell = |event| tell_to(events, ctx, event);
                let post = |message| post_to(*thread, message);
                if let Some(capture) = capture.as_mut() {
                    if up {
                        if capture.swallowed_button == Some(button) {
                            capture.swallowed_button = None;
                            if capture.done && capture.drained() {
                                post(WM_CAPTURE_DRAINED);
                            }
                            return true;
                        }
                        return false;
                    }
                    if capture.done {
                        return false;
                    }
                    capture.done = true;
                    capture.swallowed_button = Some(button);
                    let shortcut = Shortcut { mods: capture.held, input: Input::Mouse(button) };
                    tell(Event::Captured(shortcut));
                    return true;
                }
                let Some((mods, wanted)) = *trigger else { return false };
                if wanted != button {
                    return false;
                }
                if up {
                    return std::mem::take(trigger_down);
                }
                if held_now() != mods {
                    return false;
                }
                *trigger_down = true;
                post(WM_MOUSE_PRESSED);
                true
            }),
            _ => false,
        };
        if swallow {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// The hooks this thread has installed, kept to what is needed right now.
#[derive(Default)]
struct Hooks {
    keyboard: Option<windows::Win32::UI::WindowsAndMessaging::HHOOK>,
    mouse: Option<windows::Win32::UI::WindowsAndMessaging::HHOOK>,
}

impl Hooks {
    fn sync(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL};
        let (want_keyboard, want_mouse) = HOOK.with_borrow(|state| {
            let state = state.as_ref();
            let capturing = state.is_some_and(|s| s.capture.is_some());
            (capturing, capturing || state.is_some_and(|s| s.trigger.is_some()))
        });
        let module = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
            .ok()
            .map(|m| m.into());
        let set = |slot: &mut Option<_>, want: bool, id, proc_: windows::Win32::UI::WindowsAndMessaging::HOOKPROC| {
            match (slot.is_some(), want) {
                (false, true) => *slot = unsafe { SetWindowsHookExW(id, proc_, module, 0) }.ok(),
                (true, false) => {
                    if let Some(hook) = slot.take() {
                        let _ = unsafe { UnhookWindowsHookEx(hook) };
                    }
                }
                _ => {}
            }
        };
        set(&mut self.keyboard, want_keyboard, WH_KEYBOARD_LL, Some(keyboard_hook));
        set(&mut self.mouse, want_mouse, WH_MOUSE_LL, Some(mouse_hook));
    }
}

/// Makes `shortcut` the one that opens the window. False if another program has it.
fn take(shortcut: Shortcut) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, HOT_KEY_MODIFIERS, MOD_NOREPEAT};
    match shortcut.input {
        Input::Key(vk) => unsafe {
            RegisterHotKey(None, HOTKEY_ID, HOT_KEY_MODIFIERS(shortcut.mods) | MOD_NOREPEAT, vk).is_ok()
        },
        Input::Mouse(button) => {
            HOOK.with_borrow_mut(|state| {
                if let Some(state) = state.as_mut() {
                    state.trigger = Some((shortcut.mods, button));
                }
            });
            true
        }
    }
}

fn release() {
    use windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
    let _ = unsafe { UnregisterHotKey(None, HOTKEY_ID) };
    HOOK.with_borrow_mut(|state| {
        if let Some(state) = state.as_mut() {
            state.trigger = None;
        }
    });
}

/// Gives the keyboard's raw input back to Windows. winit asks for it when its event loop is made,
/// for an event egui never reads, and while a process holds it Windows does not call that same
/// process's low-level keyboard hook when one of its windows is in front — which is exactly when
/// Settings listens for a new shortcut. Measured: with any other window in front the hook was
/// called, with ours never. The mouse's raw input stays, since egui reads it. Has to run after the
/// event loop exists, and only once: winit asks again only if told to listen anew, which nothing
/// here does.
fn give_back_raw_keyboard() {
    use windows::Win32::UI::Input::{RegisterRawInputDevices, RAWINPUTDEVICE, RIDEV_REMOVE};
    let keyboard = RAWINPUTDEVICE {
        usUsagePage: 0x01, // generic desktop
        usUsage: 0x06,     // keyboard
        dwFlags: RIDEV_REMOVE,
        hwndTarget: Default::default(),
    };
    let _ = unsafe { RegisterRawInputDevices(&[keyboard], std::mem::size_of::<RAWINPUTDEVICE>() as u32) };
}

/// Starts the thread and claims `preferred`, or the first of [`CANDIDATES`] that is free if it is
/// not. The thread stays even when nothing could be claimed, so a shortcut can still be chosen in
/// Settings.
pub fn start(ctx: &egui::Context, preferred: Option<Shortcut>) -> HotkeyWatch {
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, PeekMessageW, MSG, PM_NOREMOVE, WM_HOTKEY, WM_USER};

    give_back_raw_keyboard();

    let (events_tx, events) = std::sync::mpsc::channel();
    let (commands, inbox) = std::sync::mpsc::channel::<Command>();
    let (ready_tx, ready) = std::sync::mpsc::channel();
    let ctx = ctx.clone();
    let wanted: Vec<Shortcut> = match preferred {
        Some(shortcut) if shortcut != DEFAULT => std::iter::once(shortcut).chain(CANDIDATES).collect(),
        _ => CANDIDATES.to_vec(),
    };

    // Registered on the thread that pumps for it: Windows delivers WM_HOTKEY to the queue of the
    // thread that made the call, and calls a low-level hook on the thread that installed it.
    std::thread::spawn(move || {
        let mut msg = MSG::default();
        // A thread has no message queue until it asks for one, and `PostThreadMessageW` to a
        // thread without one is lost.
        unsafe {
            let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
        }
        let thread = unsafe { GetCurrentThreadId() };
        HOOK.with_borrow_mut(|state| {
            *state = Some(HookState {
                thread,
                events: events_tx,
                ctx,
                trigger: None,
                trigger_down: false,
                capture: None,
            })
        });
        let mut hooks = Hooks::default();
        let won = wanted.into_iter().find(|shortcut| take(*shortcut));
        hooks.sync();
        let _ = ready_tx.send((thread, won));
        let mut current = won;

        let tell = |event: Event| HOOK.with_borrow(|state| state.as_ref().map(|s| tell_to(&s.events, &s.ctx, event)));
        let capturing = || HOOK.with_borrow(|state| state.as_ref().is_some_and(|s| s.capture.is_some()));

        // `GetMessageW` blocks until something arrives, so this thread costs nothing at rest.
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
            match msg.message {
                WM_HOTKEY if msg.wParam.0 as i32 == HOTKEY_ID && !capturing() => {
                    // First, and on this thread, because it has to reach the window underneath
                    // while that window still has the keyboard — a moment later it will be ours.
                    break_modifier_tap();
                    tell(Event::Pressed);
                }
                WM_MOUSE_PRESSED => {
                    break_modifier_tap();
                    tell(Event::Pressed);
                }
                WM_CAPTURE_DRAINED => {
                    HOOK.with_borrow_mut(|state| {
                        if let Some(state) = state.as_mut() {
                            if state.capture.as_ref().is_some_and(|c| c.done && c.drained()) {
                                state.capture = None;
                            }
                        }
                    });
                    hooks.sync();
                }
                WM_COMMAND_READY => {
                    for command in inbox.try_iter() {
                        match command {
                            Command::Claim(list, reply) => {
                                release();
                                let won = list.into_iter().find(|shortcut| take(*shortcut));
                                match won {
                                    Some(shortcut) => current = Some(shortcut),
                                    // Nothing new could be had: back to the one held before, which
                                    // was free a moment ago.
                                    None => {
                                        if let Some(previous) = current {
                                            take(previous);
                                        }
                                    }
                                }
                                let _ = reply.send(won);
                            }
                            Command::BeginCapture => HOOK.with_borrow_mut(|state| {
                                if let Some(state) = state.as_mut() {
                                    if state.capture.is_none() {
                                        state.capture = Some(Capture {
                                            held: 0,
                                            swallowed: [0; 4],
                                            swallowed_button: None,
                                            done: false,
                                        });
                                    }
                                }
                            }),
                            Command::EndCapture => HOOK.with_borrow_mut(|state| {
                                if let Some(state) = state.as_mut() {
                                    if let Some(capture) = state.capture.as_mut() {
                                        capture.done = true;
                                        if capture.drained() {
                                            state.capture = None;
                                        }
                                    }
                                }
                            }),
                        }
                    }
                    hooks.sync();
                }
                _ => {}
            }
        }
    });

    let (thread, current) = ready.recv().unwrap_or((0, None));
    HotkeyWatch { events, commands, thread, current }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips() {
        let all = [
            DEFAULT,
            CANDIDATES[1],
            Shortcut { mods: MOD_CONTROL, input: Input::Mouse(Button::X1) },
            Shortcut { mods: 0, input: Input::Mouse(Button::Middle) },
            Shortcut { mods: 0, input: Input::Key(0x7C) },
            Shortcut { mods: MOD_WIN | MOD_ALT, input: Input::Key(0x4B) },
            Shortcut { mods: MOD_CONTROL | MOD_ALT, input: Input::Key(0xC0) },
        ];
        for shortcut in all {
            assert_eq!(Shortcut::from_config(&shortcut.to_config()), Some(shortcut));
        }
        assert_eq!(DEFAULT.to_config(), "Alt+Space");
        assert_eq!(Shortcut::from_config("Alt+Shift+Space"), Some(CANDIDATES[1]));
        assert_eq!(Shortcut::from_config("F13").map(|s| s.to_config()), Some("F13".to_string()));
    }

    #[test]
    fn nothing_that_takes_away_typing() {
        assert_eq!(Shortcut::from_config("A"), None);
        assert_eq!(Shortcut::from_config("Shift+A"), None);
        assert_eq!(Shortcut::from_config("Space"), None);
        assert_eq!(Shortcut::from_config("Alt+Esc"), None);
        assert_eq!(Shortcut::from_config("Nonsense+A"), None);
        assert_eq!(Shortcut::from_config(""), None);
        assert!(Shortcut::from_config("Mouse4").is_some());
        assert!(Shortcut::from_config("Shift+F24").is_some());
    }

    #[test]
    fn labels_in_each_language() {
        assert_eq!(DEFAULT.label(Lang::Es), "Alt+Espacio");
        assert_eq!(DEFAULT.label(Lang::En), "Alt+Space");
        assert_eq!(CANDIDATES[1].label(Lang::Es), "Mayús+Alt+Espacio");
        let mouse = Shortcut { mods: MOD_CONTROL, input: Input::Mouse(Button::X2) };
        assert_eq!(mouse.label(Lang::Es), "Ctrl+Ratón 5");
    }
}
