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

//! «No expandir en»: the programs where espanso keeps quiet, and the open ones to pick them from.
//!
//! Espanso does this itself, with an app-specific config: a file in its `config` folder whose
//! `filter_exec` names the programs and whose `enable: false` switches it off there. This module
//! owns exactly one such file, [`FILE_NAME`], written whole from `settings.json` every time the
//! list changes and removed when the list is empty. It never reads, edits or renames any other file
//! in that folder: those are the user's own, and they stay as they were.
//!
//! Precedence is the one thing to know about it. Espanso uses the *first* app-specific config whose
//! filters match, in the order the folder lists them, and on Windows that order is alphabetical —
//! hence the `00-` in front of the name. A program chosen here is off, whatever another file says
//! about it.
//!
//! Exclusion is by program, because that is what espanso can see: the path of the executable with
//! the keyboard. A browser counts whole, every page in it.
//!
//! Not here: when espanso restarts to pick the file up, which is [`crate::app::AppState`]'s call.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// The one file in espanso's `config` folder that is ours to write.
pub const FILE_NAME: &str = "00-khipu-excluded-apps.yml";

/// A program espanso stays off in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcludedApp {
    /// The executable's file name, as Windows spells it (`brave.exe`). What the filter matches.
    pub exe: String,
    /// What the program calls itself (`Brave Browser`). Only for showing.
    pub name: String,
}

/// The file's contents for `apps`, or `None` when there is nothing to exclude.
fn contents(apps: &[ExcludedApp]) -> Option<String> {
    if apps.is_empty() {
        return None;
    }
    let names: Vec<String> = apps.iter().map(|app| escape(&app.exe)).collect();
    // The file name at the end of the full path, in any case: `(?:^|\\)` keeps `rave.exe` from
    // matching `brave.exe`. Single-quoted YAML, so the backslashes are the regex's own.
    let filter = format!("(?i)(?:^|\\\\)(?:{})$", names.join("|")).replace('\'', "''");
    Some(format!(
        "# Written by Khipu from Settings > Don't expand in. It is rewritten whenever that list\n\
         # changes, so change the list there; your own files in this folder are never touched.\n\
         filter_exec: '{filter}'\n\
         enable: false\n"
    ))
}

/// A file name as a regex that matches only itself.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Brings the file in `config_dir` in line with `apps`. Says whether anything changed, which is
/// when espanso has to restart to see it.
pub fn write(config_dir: &Path, apps: &[ExcludedApp]) -> std::io::Result<bool> {
    let path = config_dir.join(FILE_NAME);
    let current = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let wanted = contents(apps);
    if current == wanted {
        return Ok(false);
    }
    match wanted {
        Some(text) => std::fs::write(&path, text)?,
        None => std::fs::remove_file(&path)?,
    }
    Ok(true)
}

/// The programs with a window on screen now, by name, one each — what «Añadir» offers.
///
/// A window counts if it is one somebody would call a program: visible, not cloaked (another
/// desktop's, or a suspended Store app's), not owned by another window, not a tool palette, and
/// titled. Khipu itself is left out, and so is the host of Store apps, which is not the app.
pub fn open_apps() -> Vec<ExcludedApp> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;

    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> windows::core::BOOL {
        if let Some(path) = program_of(hwnd) {
            unsafe { (*(found.0 as *mut Vec<String>)).push(path) };
        }
        true.into()
    }

    let mut paths: Vec<String> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut paths as *mut Vec<String> as isize));
    }

    let own = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    let mut seen = std::collections::BTreeSet::new();
    let mut apps: Vec<ExcludedApp> = paths
        .into_iter()
        .filter_map(|path| {
            let exe = Path::new(&path).file_name()?.to_string_lossy().into_owned();
            let key = exe.to_lowercase();
            if Some(&key) == own.as_ref() || key == "applicationframehost.exe" || !seen.insert(key) {
                return None;
            }
            let name = description(&path).unwrap_or_else(|| {
                Path::new(&exe).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(exe.clone())
            });
            Some(ExcludedApp { exe, name })
        })
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

/// The full path of the program behind `hwnd`, if it is a window [`open_apps`] counts.
fn program_of(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
        GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
    };

    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || GetWindowTextLengthW(hwnd) == 0 {
            return None;
        }
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
            return None;
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }
        let mut cloaked = 0u32;
        let asked = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        );
        if asked.is_ok() && cloaked != 0 {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let named = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        named.ok()?;
        Some(String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

/// What the program at `path` calls itself: the description in its version information, which is
/// the name the Task Manager shows.
fn description(path: &str) -> Option<String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

    let path = HSTRING::from(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut info = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(path.as_ptr()), None, size, info.as_mut_ptr().cast()).ok()?;

        let mut value: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let query = HSTRING::from("\\VarFileInfo\\Translation");
        if !VerQueryValueW(info.as_ptr().cast(), PCWSTR(query.as_ptr()), &mut value, &mut len).as_bool()
            || len < 4
        {
            return None;
        }
        let pair = value as *const u16;
        let (lang, codepage) = (*pair, *pair.add(1));

        let query = HSTRING::from(format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription"));
        if !VerQueryValueW(info.as_ptr().cast(), PCWSTR(query.as_ptr()), &mut value, &mut len).as_bool()
            || len == 0
        {
            return None;
        }
        let text = std::slice::from_raw_parts(value as *const u16, len as usize);
        let text = String::from_utf16_lossy(text);
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(exe: &str) -> ExcludedApp {
        ExcludedApp { exe: exe.into(), name: exe.into() }
    }

    #[test]
    fn the_filter_names_each_program_whole_and_in_any_case() {
        let text = contents(&[app("brave.exe"), app("Code - Insiders.exe")]).unwrap();
        assert!(text.contains(r"filter_exec: '(?i)(?:^|\\)(?:brave\.exe|Code \- Insiders\.exe)$'"));
        assert!(text.contains("enable: false"));
    }

    #[test]
    fn a_quote_in_a_name_does_not_end_the_yaml_string() {
        let text = contents(&[app("it's.exe")]).unwrap();
        assert!(text.contains("it''s\\.exe"));
    }

    #[test]
    fn the_file_comes_and_goes_with_the_list_and_nothing_else_is_touched() {
        let dir = std::env::temp_dir().join(format!("khipu-exclusions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let theirs = dir.join("brave.yml");
        std::fs::write(&theirs, "filter_exec: brave\nbackend: clipboard\n").unwrap();

        assert!(write(&dir, &[app("brave.exe")]).unwrap());
        assert!(!write(&dir, &[app("brave.exe")]).unwrap(), "same list, nothing to restart for");
        assert!(dir.join(FILE_NAME).exists());
        assert!(write(&dir, &[]).unwrap());
        assert!(!dir.join(FILE_NAME).exists());
        assert!(!write(&dir, &[]).unwrap());
        assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "filter_exec: brave\nbackend: clipboard\n");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ours_is_read_before_any_name_a_person_would_give_theirs() {
        for theirs in ["1.yml", "app.yml", "Brave.yml", "default.yml", "_x.yml"] {
            assert!(FILE_NAME.to_lowercase() < theirs.to_lowercase(), "{theirs}");
        }
    }
}
