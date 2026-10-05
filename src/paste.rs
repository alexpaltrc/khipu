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

//! Putting a text into the window in front the way Win+V does: on the clipboard, then Ctrl+V,
//! then the clipboard handed back exactly as it was.
//!
//! Typing it is what does not work. Measured in Notepad 11.2607, bulk Unicode keystrokes lose most
//! of a 90-character text, and espanso's own typed injection turns all but the first word into the
//! last letter. Espanso itself pastes anything over 100 characters, and its answer to the same
//! Notepad report (espanso#2628) is `backend: Clipboard`.
//!
//! Three things are done more carefully than espanso does them, and they are the invariants here:
//!
//! - **Everything on the clipboard comes back**, not only its text: a copied image or a copied file
//!   survives a paste. The formats that are GDI handles rather than memory cannot be copied, and are
//!   not needed — Windows rebuilds a bitmap from the DIB that is kept. See [`skip`].
//! - **The pasted text stays out of the Win+V history** and out of the cloud clipboard, through the
//!   formats Windows defines for exactly that. The restored content is marked the same way, so the
//!   history does not get a second copy of something it already has.
//! - **The clipboard is restored when the paste has been read**, not after a guessed delay. The text
//!   is offered with delayed rendering, so Windows asks for it at the moment the target reads it —
//!   and that request is the signal. A request from a process that is not the target (a clipboard
//!   tool that ignores the exclusion formats) does not count, and a fixed wait covers that case.
//!
//! And one rule: if anything else is put on the clipboard while this is in progress, it is the
//! user's, and it is not overwritten with the old content.

use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardOwner,
    GetOpenClipboardWindow, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowThreadProcessId,
    MsgWaitForMultipleObjects, PeekMessageW, RegisterClassW, HWND_MESSAGE, MSG, PM_REMOVE,
    QS_ALLINPUT, WINDOW_EX_STYLE, WINDOW_STYLE, WM_RENDERFORMAT, WNDCLASSW,
};

const CF_UNICODETEXT: u32 = 13;

/// How long the target has to read the paste before the clipboard is given back anyway. Reached
/// only when it never reads it: an app with nothing to paste into, or one that ignores Ctrl+V.
const READ_TIMEOUT: Duration = Duration::from_secs(2);
/// The wait used instead when the read cannot be seen, because someone else rendered the text
/// first and the target's own read then comes from memory without asking. It is espanso's own
/// figure for every paste (`restore_clipboard_delay`, 300 ms), doubled.
const BLIND_WAIT: Duration = Duration::from_millis(600);
/// Another program may have the clipboard open for a moment; opening it is retried for this long.
const OPEN_TIMEOUT: Duration = Duration::from_millis(500);

/// One paste at a time: two would each save the other's text as "the user's clipboard".
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

thread_local! {
    /// The text the window procedure hands over when Windows asks for it, NUL-terminated.
    static PENDING: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    /// Set when the text was rendered: the process that asked, if Windows can say.
    static RENDERED: Cell<Option<Option<u32>>> = const { Cell::new(None) };
}

/// Pastes `text` into the foreground window, which belongs to process `target_pid`.
///
/// Blocks for as long as the paste takes, up to [`READ_TIMEOUT`]: call it off the interface
/// thread. Returns whether the paste was sent. A `false` leaves the clipboard as it was.
pub fn paste(text: &str, target_pid: u32) -> bool {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|poison| poison.into_inner());
    let Some(window) = (unsafe { create_window() }) else { return false };
    let sent = unsafe { paste_with(window, text, target_pid) };
    unsafe {
        let _ = DestroyWindow(window);
    }
    sent
}

unsafe fn paste_with(window: HWND, text: &str, target_pid: u32) -> bool {
    // Windows wants CRLF on the clipboard; a bare LF arrives as nothing at all in some editors.
    let normalized = text.replace("\r\n", "\n").replace('\n', "\r\n");
    PENDING.with(|p| *p.borrow_mut() = normalized.encode_utf16().chain(Some(0)).collect());
    RENDERED.with(|r| r.set(None));

    if !open(window) {
        return false;
    }
    let saved = snapshot();
    // No data: Windows sends `WM_RENDERFORMAT` when somebody actually reads it. Its answer cannot
    // say whether that worked — success here *is* a null handle, which the binding reports as an
    // error — so the clipboard is asked instead.
    let offered = EmptyClipboard().is_ok() && {
        let _ = SetClipboardData(CF_UNICODETEXT, None);
        IsClipboardFormatAvailable(CF_UNICODETEXT).is_ok()
    };
    if offered {
        mark_private();
    }
    let _ = CloseClipboard();
    if !offered {
        restore(window, &saved);
        return false;
    }

    if !send_ctrl_v() {
        // Refused outright — the target runs elevated. Nothing will read the text.
        restore(window, &saved);
        return false;
    }

    let start = Instant::now();
    let mut deadline = start + READ_TIMEOUT;
    loop {
        pump();
        match RENDERED.with(Cell::get) {
            // Read by the target, or by a reader Windows cannot name. Either way the paste is done.
            Some(None) => break,
            Some(Some(pid)) if pid == target_pid => break,
            // Read by somebody else first. The target's own read will now come from memory without
            // a word to us, so the only thing left is to give it time.
            Some(Some(_)) => deadline = deadline.min(start + BLIND_WAIT),
            None => {}
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let left = (deadline - now).as_millis() as u32;
        MsgWaitForMultipleObjects(None, false, left.max(1), QS_ALLINPUT);
    }

    restore(window, &saved);
    true
}

/// Opens the clipboard as `window`, waiting out whoever has it open right now. The reader that has
/// just been handed the text is one of them: its `CloseClipboard` is what this waits for.
unsafe fn open(window: HWND) -> bool {
    let deadline = Instant::now() + OPEN_TIMEOUT;
    loop {
        if OpenClipboard(Some(window)).is_ok() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Formats whose data is a GDI handle or a pointer, not a block of memory: nothing to copy.
/// The private ranges are handles their owner frees and nobody else can interpret.
fn skip(format: u32) -> bool {
    matches!(format, 2 | 3 | 9 | 14 | 0x80 | 0x82 | 0x83 | 0x8E | 0x200..=0x3FF)
}

/// Copies every memory format on the open clipboard. Delayed formats are rendered by their owner
/// as they are asked for, which is the same thing a paste of them would do.
unsafe fn snapshot() -> Vec<(u32, Vec<u8>)> {
    let mut saved = Vec::new();
    let mut format = 0;
    loop {
        format = EnumClipboardFormats(format);
        if format == 0 {
            return saved;
        }
        if skip(format) {
            continue;
        }
        let Ok(handle) = GetClipboardData(format) else { continue };
        let memory = HGLOBAL(handle.0);
        let size = GlobalSize(memory);
        let data = GlobalLock(memory);
        if data.is_null() {
            continue;
        }
        saved.push((format, std::slice::from_raw_parts(data as *const u8, size).to_vec()));
        let _ = GlobalUnlock(memory);
    }
}

/// Puts `bytes` on the open clipboard under `format`. The clipboard owns the memory once this
/// succeeds; until then it is ours to free.
unsafe fn put(format: u32, bytes: &[u8]) -> bool {
    let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) else { return false };
    let data = GlobalLock(memory);
    if data.is_null() {
        let _ = GlobalFree(Some(memory));
        return false;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), data as *mut u8, bytes.len());
    let _ = GlobalUnlock(memory);
    if SetClipboardData(format, Some(HANDLE(memory.0))).is_err() {
        let _ = GlobalFree(Some(memory));
        return false;
    }
    true
}

/// Asks Windows to keep what is on the open clipboard out of the Win+V history, out of the cloud
/// clipboard, and away from clipboard monitors. Documented under "Clipboard Formats".
unsafe fn mark_private() {
    let zero = 0u32.to_ne_bytes();
    for (name, bytes) in [
        (w!("ExcludeClipboardContentFromMonitorProcessing"), &zero[..]),
        (w!("CanIncludeInClipboardHistory"), &zero[..]),
        (w!("CanUploadToCloudClipboard"), &zero[..]),
    ] {
        let format = RegisterClipboardFormatW(name);
        if format != 0 {
            put(format, bytes);
        }
    }
}

/// Gives the clipboard back — unless it is no longer ours, in which case whatever is on it now was
/// put there by somebody after the paste began, and is newer than what was saved.
unsafe fn restore(window: HWND, saved: &[(u32, Vec<u8>)]) {
    if GetClipboardOwner().ok() != Some(window) {
        return;
    }
    if !open(window) {
        return;
    }
    if EmptyClipboard().is_ok() {
        for (format, bytes) in saved {
            put(*format, bytes);
        }
        if !saved.is_empty() {
            mark_private();
        }
    }
    let _ = CloseClipboard();
}

/// Ctrl down, V down, V up, Ctrl up, in one call so nothing typed meanwhile lands between them.
fn send_ctrl_v() -> bool {
    let key = |vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    };
    let v = VIRTUAL_KEY(u16::from(b'V'));
    let events = [
        key(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        key(v, KEYBD_EVENT_FLAGS(0)),
        key(v, KEYEVENTF_KEYUP),
        key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let sent = unsafe { SendInput(&events, std::mem::size_of::<INPUT>() as i32) };
    sent as usize == events.len()
}

/// Delivers whatever is waiting, including the `WM_RENDERFORMAT` that a reader is blocked on.
unsafe fn pump() {
    let mut msg = MSG::default();
    while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
        DispatchMessageW(&msg);
    }
}

/// A message-only window: it owns the clipboard while the paste is on it, and receives the request
/// for the text. Created on the thread that pumps it.
unsafe fn create_window() -> Option<HWND> {
    let hinstance = GetModuleHandleW(None).ok()?;
    let class_name = w!("Khipu Paste");
    let class = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        ..Default::default()
    };
    // Registered by the first paste; every later one finds it already there, which is fine.
    RegisterClassW(&class);
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class_name,
        PCWSTR::null(),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        Some(hinstance.into()),
        None,
    )
    .ok()
}

unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_RENDERFORMAT && wparam.0 as u32 == CF_UNICODETEXT {
        let bytes = PENDING.with(|p| {
            p.borrow().iter().flat_map(|unit| unit.to_ne_bytes()).collect::<Vec<u8>>()
        });
        // The clipboard is already open by the reader: `SetClipboardData` without opening it is
        // exactly what this message expects.
        put(CF_UNICODETEXT, &bytes);
        let reader = GetOpenClipboardWindow().ok().map(|window| {
            let mut pid = 0;
            GetWindowThreadProcessId(window, Some(&mut pid));
            pid
        });
        RENDERED.with(|r| r.set(Some(reader)));
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}
