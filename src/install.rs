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

//! Where everything lives: beside the executable, or in the app's own corner of the user profile.
//!
//! One binary, two ways of being on a machine. **Portable**: unzipped anywhere, and everything —
//! espanso, its folders, ours — sits beside the executable, as it always has. **Installed**, as an
//! MSIX package from the Microsoft Store: Windows puts the executable in a folder nobody may write
//! to, replaces that folder whole on every update, and deletes it on uninstall. So nothing the user
//! owns can live there. Their expansions, our settings and espanso's runtime go to the package's
//! `LocalState` folder instead, which Windows keeps across updates.
//!
//! The invariant: **every path in the program comes from [`Layout`]**, and the two layouts differ
//! only in where `data` is. Below `data` the shape is the same in both — `.espanso`,
//! `.espanso-runtime`, `.espanso-manager` — so nothing downstream has to know which one it is in.
//!
//! espanso is told where `data` is through its own variables, `ESPANSO_CONFIG_DIR` and
//! `ESPANSO_RUNTIME_DIR` (read by espanso 2.4.1 in `get_path_override`, and handed on by its daemon
//! to the worker it starts). Without them, an installed espanso would look for a `.espanso` beside
//! itself, find none, and quietly fall back to `%APPDATA%\espanso` — a second, empty library.
//!
//! What does **not** go here: anything about the start-at-login switch (`autostart`), or about
//! talking to espanso once it knows where to look (`espanso_ctl`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use windows::core::PWSTR;
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFamilyName;

/// Subfolder holding everything that is not the executable: `espansod.exe`, its DLLs, the two
/// `.espanso` folders, ours, and the text files. Named the way Windows names such a folder, so
/// that a person who has never seen this program reads it as "not for me" without being told.
///
/// The installed package keeps the same layout, so both find espanso the same way.
pub const RESOURCES_DIR: &str = "Program Files";

/// Inside [`RESOURCES_DIR`] of the installed package only: a pristine `.espanso` and
/// `.espanso-runtime`, copied into `data` the first time. Not named `.espanso` itself on purpose —
/// espanso treats a `.espanso` beside its own executable as "portable mode", and in the package
/// that folder is read-only.
const SEED_DIR: &str = "seed";

pub struct Layout {
    /// Where `espansod.exe` and its DLLs are. Read-only in the installed package.
    pub resources: PathBuf,
    /// Where `.espanso`, `.espanso-runtime` and `.espanso-manager` are. Always writable.
    pub data: PathBuf,
    /// Set when running from an MSIX package.
    pub packaged: bool,
}

impl Layout {
    pub fn espansod(&self) -> PathBuf {
        self.resources.join("espansod.exe")
    }

    pub fn espanso_config(&self) -> PathBuf {
        self.data.join(".espanso")
    }

    pub fn espanso_runtime(&self) -> PathBuf {
        self.data.join(".espanso-runtime")
    }

    pub fn manager_dir(&self) -> PathBuf {
        self.data.join(".espanso-manager")
    }

    /// The variables every run of `espansod` needs. None in the portable layout: there espanso finds
    /// its folders beside itself, and setting them would only override the same answer.
    pub fn espanso_env(&self) -> Vec<(&'static str, PathBuf)> {
        if !self.packaged {
            return Vec::new();
        }
        vec![
            ("ESPANSO_CONFIG_DIR", self.espanso_config()),
            ("ESPANSO_RUNTIME_DIR", self.espanso_runtime()),
        ]
    }

    /// Fills `data` with whatever it lacks from the package's seed. Installed layout only.
    ///
    /// Only missing files are copied, never one that exists: the user's `base.yml` and espanso's
    /// `default.yml` as we patched it must survive every start. That same rule is what makes this
    /// safe to run on every start, not only the first — an update that ships a new icon gets it in.
    ///
    /// The two folders espanso is pointed at are created even when the seed is missing, because
    /// espanso refuses to start when `ESPANSO_CONFIG_DIR` names a folder that does not exist.
    pub fn seed(&self) -> std::io::Result<()> {
        if !self.packaged {
            return Ok(());
        }
        std::fs::create_dir_all(self.espanso_config())?;
        std::fs::create_dir_all(self.espanso_runtime())?;
        copy_missing(&self.resources.join(SEED_DIR), &self.data)
    }
}

/// The folder every other path in this program is derived from, in the portable layout.
///
/// Two layouts have to work at once. The tidy one ships everything except the executable inside
/// [`RESOURCES_DIR`], so unzipping produces exactly one thing to double-click. The old one — the
/// one already on other people's machines — has the executable sitting among nineteen DLLs, and
/// the documented way to update it is "replace only `Espanso Manager.exe`". That instruction has to
/// keep being true: a new binary dropped into an old folder finds no subfolder, falls through to
/// the executable's own directory, and carries on exactly as before.
///
/// The subfolder is only believed if it holds something of ours. A bare directory that happens to
/// share the name is not enough, because being wrong here does not fail loudly — it would silently
/// look for the user's expansions in a folder that has none, and offer to start them a fresh empty
/// one.
fn resolve_base_dir(exe_dir: PathBuf) -> PathBuf {
    let nested = exe_dir.join(RESOURCES_DIR);
    let is_ours = nested.join("espansod.exe").is_file()
        || nested.join(".espanso").is_dir()
        || nested.join(".espanso-manager").is_dir();
    if is_ours {
        nested
    } else {
        exe_dir
    }
}

pub fn resolve(exe_path: &Path) -> Layout {
    let exe_dir = exe_path
        .parent()
        .expect("the executable should have a containing folder")
        .to_path_buf();
    let resources = resolve_base_dir(exe_dir);
    match package_family_name() {
        Some(family) => Layout { data: local_state(family), resources, packaged: true },
        None => Layout { data: resources.clone(), resources, packaged: false },
    }
}

/// Whether this process was started from an MSIX package. Asked once; it cannot change.
pub fn is_packaged() -> bool {
    package_family_name().is_some()
}

fn package_family_name() -> Option<&'static str> {
    static FAMILY: OnceLock<Option<String>> = OnceLock::new();
    FAMILY
        .get_or_init(|| {
            // Two calls, the usual way: the first asks how long the name is. An unpackaged process
            // gets APPMODEL_ERROR_NO_PACKAGE instead, which is the answer "portable".
            let mut len = 0u32;
            if unsafe { GetCurrentPackageFamilyName(&mut len, None) } != ERROR_INSUFFICIENT_BUFFER {
                return None;
            }
            let mut buf = vec![0u16; len as usize];
            if unsafe { GetCurrentPackageFamilyName(&mut len, Some(PWSTR(buf.as_mut_ptr()))) }
                != ERROR_SUCCESS
            {
                return None;
            }
            // `len` counts the terminating zero.
            Some(String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]))
        })
        .as_deref()
}

/// `%LOCALAPPDATA%\Packages\<family>\LocalState`: the folder Windows gives every package for its
/// own files, kept across updates and removed on uninstall. Not `%APPDATA%`: a packaged process
/// writing there is silently redirected into a private copy, so the path would lie about where the
/// files really are.
fn local_state(family: &str) -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| {
        let profile = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        profile.join("AppData").join("Local")
    });
    local.join("Packages").join(family).join("LocalState")
}

/// Was this run started by Windows at login, through the package's startup task?
///
/// The portable copy learns this from `--tray`, which it writes into its own Run entry. A startup
/// task cannot carry arguments, so the installed copy has to ask how it was activated.
pub fn launched_at_login() -> bool {
    if !is_packaged() {
        return false;
    }
    use windows::ApplicationModel::Activation::ActivationKind;
    windows::ApplicationModel::AppInstance::GetActivatedEventArgs()
        .and_then(|args| args.Kind())
        .is_ok_and(|kind| kind == ActivationKind::StartupTask)
}

/// Copies every file under `from` into the same place under `to`, unless it is already there.
/// A missing `from` is not an error: there is simply nothing to add.
fn copy_missing(from: &Path, to: &Path) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(from) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    std::fs::create_dir_all(to)?;
    for entry in entries {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_missing(&entry.path(), &target)?;
        } else if !target.exists() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("espansomanager-basedir-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_old_folder_without_the_subfolder_stays_on_the_executables_own_directory() {
        let dir = scratch("old-layout");
        assert_eq!(resolve_base_dir(dir.clone()), dir);
    }

    #[test]
    fn a_tidied_folder_resolves_into_the_subfolder() {
        let dir = scratch("tidy-layout");
        let nested = dir.join(RESOURCES_DIR);
        std::fs::create_dir_all(nested.join(".espanso")).unwrap();
        assert_eq!(resolve_base_dir(dir), nested);
    }

    /// An empty directory of the same name must not capture the lookup: doing so would send the
    /// program off to read expansions from a folder that has none, which fails silently rather
    /// than loudly.
    #[test]
    fn a_folder_that_only_shares_the_name_is_not_mistaken_for_ours() {
        let dir = scratch("name-clash");
        std::fs::create_dir_all(dir.join(RESOURCES_DIR)).unwrap();
        assert_eq!(resolve_base_dir(dir.clone()), dir);
    }

    /// The daemon alone is enough, because that is what a folder looks like before the app has
    /// ever been run in it — the `.espanso-manager` folder does not exist until the first save.
    #[test]
    fn the_daemon_on_its_own_is_enough_to_recognise_the_subfolder() {
        let dir = scratch("daemon-only");
        let nested = dir.join(RESOURCES_DIR);
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("espansod.exe"), b"not really an exe").unwrap();
        assert_eq!(resolve_base_dir(dir), nested);
    }

    /// `cargo test` runs unpackaged, so this is the portable answer: data beside espanso, and no
    /// variables to override what espanso already finds there.
    #[test]
    fn a_test_run_is_portable_and_sets_no_espanso_variables() {
        let dir = scratch("portable-resolve");
        let layout = resolve(&dir.join("Espanso Manager.exe"));
        assert!(!layout.packaged);
        assert_eq!(layout.data, layout.resources);
        assert!(layout.espanso_env().is_empty());
    }

    /// The seed fills gaps and nothing else: the user's own library is never replaced by the
    /// starter one, however many times the app starts.
    #[test]
    fn the_seed_never_overwrites_what_is_already_there() {
        let dir = scratch("seed");
        let seed = dir.join("seed");
        std::fs::create_dir_all(seed.join(".espanso").join("match")).unwrap();
        std::fs::write(seed.join(".espanso").join("match").join("base.yml"), "starter").unwrap();
        std::fs::create_dir_all(seed.join(".espanso-runtime")).unwrap();
        std::fs::write(seed.join(".espanso-runtime").join("normalv2.ico"), "icon").unwrap();

        let data = dir.join("data");
        std::fs::create_dir_all(data.join(".espanso").join("match")).unwrap();
        std::fs::write(data.join(".espanso").join("match").join("base.yml"), "mine").unwrap();

        copy_missing(&seed, &data).unwrap();
        let read = |p: PathBuf| std::fs::read_to_string(p).unwrap();
        assert_eq!(read(data.join(".espanso").join("match").join("base.yml")), "mine");
        assert_eq!(read(data.join(".espanso-runtime").join("normalv2.ico")), "icon");
        // A seed that is not there is nothing to add, not a failure.
        copy_missing(&dir.join("absent"), &data).unwrap();
    }
}
