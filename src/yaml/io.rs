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

//! Reading and writing `base.yml` — the user's own file, which espanso is watching at the same
//! time.
//!
//! And the file beside it that holds the expansions switched off, `_disabled.yml`: espanso skips
//! any file whose name starts with an underscore. One library in memory, two files on disk; see
//! [`save`] for the order they are written in.
//!
//! A save is: validate, serialize, re-parse the result as a self-check, back the old library up —
//! one folder per save, holding both files — write to a scratch file beside it, flush, then rename
//! over the top. The rename is what makes it safe
//! against espanso opening the file mid-write; the flush is what makes it safe against the power
//! going out, since NTFS journals the directory entry and not the contents.
//!
//! **Nothing in this module may write to the console.** There is none: the `windows_subsystem`
//! attribute at the top of `main.rs` builds this binary without one, so an `eprintln!` here is not
//! a quiet log, it is a failure nobody will ever be told about. Everything that degrades on the
//! way to a save that still went through is carried back in `SaveWarnings` for the caller to put
//! on screen. That rule has been broken three separate times; if you are about to report something
//! from here, add a field.

use super::model::{MatchEntry, MatchFile};
use crate::i18n::{fill, Strings};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum ValidationIssue {
    EmptyTrigger,
    DuplicateTrigger(String),
}

impl ValidationIssue {
    pub fn friendly_message(&self, t: &'static Strings) -> String {
        match self {
            ValidationIssue::EmptyTrigger => t.err_validation_empty.to_string(),
            ValidationIssue::DuplicateTrigger(trigger) => {
                fill(t.err_validation_duplicate, &[("name", trigger)])
            }
        }
    }
}

pub fn validate(mf: &MatchFile) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for entry in &mf.entries {
        if let MatchEntry::Simple(m) = entry {
            if m.trigger.trim().is_empty() {
                issues.push(ValidationIssue::EmptyTrigger);
                continue;
            }
            if !seen.insert(m.trigger.as_str()) {
                issues.push(ValidationIssue::DuplicateTrigger(m.trigger.clone()));
            }
        }
    }
    issues
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(serde_norway::Error),
}

impl LoadError {
    pub fn friendly_message(&self, t: &'static Strings) -> String {
        match self {
            LoadError::Io(e) => fill(t.err_read_file, &[("err", &e.to_string())]),
            LoadError::Parse(e) => fill(t.err_parse_yaml, &[("err", &e.to_string())]),
        }
    }
}

/// The file of switched-off expansions that goes with the `base.yml` at `path`.
pub fn off_path(path: &Path) -> PathBuf {
    path.with_file_name(super::model::OFF_FILE)
}

/// Reads the library: `base.yml` and, if there is one, the file of switched-off expansions beside
/// it.
///
/// Either file failing to read is a failure of the whole load, so the app refuses to save — a
/// `_disabled.yml` that could not be read would otherwise be written over with nothing. A missing
/// `base.yml` is reported as missing only when there is no file of switched-off expansions either:
/// with one, the library is simply all switched off.
pub fn load(path: &Path) -> Result<MatchFile, LoadError> {
    let off = match std::fs::read_to_string(off_path(path)) {
        Ok(text) => Some(MatchFile::from_str(&text).map_err(LoadError::Parse)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(LoadError::Io(e)),
    };
    let mut mf = match std::fs::read_to_string(path) {
        Ok(contents) => MatchFile::from_str(&contents).map_err(LoadError::Parse)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && off.is_some() => MatchFile::default(),
        Err(e) => return Err(LoadError::Io(e)),
    };
    if let Some(off) = off {
        mf.take_off_file(off);
    }
    Ok(mf)
}

#[derive(Debug)]
pub enum SaveError {
    Validation(Vec<ValidationIssue>),
    /// Our own serialization didn't round-trip through the parser — a bug on our side, not the
    /// user's. Better to refuse the write than risk corrupting their file.
    SelfCheckFailed(serde_norway::Error),
    Io(std::io::Error),
}

impl SaveError {
    pub fn friendly_message(&self, t: &'static Strings) -> String {
        match self {
            SaveError::Validation(issues) => issues
                .iter()
                .map(|i| i.friendly_message(t))
                .collect::<Vec<_>>()
                .join("\n"),
            SaveError::SelfCheckFailed(e) => fill(t.err_self_check, &[("err", &e.to_string())]),
            SaveError::Io(e) => fill(t.err_write_file, &[("err", &e.to_string())]),
        }
    }
}

/// What went wrong on the way to a save that still went through.
///
/// Neither of these is a reason to throw the user's edit away, and neither ever was. But both used
/// to be announced with `eprintln!`, into a console this binary does not have — see the rule at the
/// top of this module — so those lines went to a handle nobody can read. The user was left
/// believing a backup exists, and the backup is precisely what they reach for when something has
/// gone wrong.
#[derive(Debug, Default)]
pub struct SaveWarnings {
    /// The previous file could not be copied into `backups/` before being replaced.
    pub backup: Option<std::io::Error>,
    /// The bytes were written but Windows would not confirm they reached the disk. Expected on a
    /// USB stick or a network share, which this app is carried on.
    pub flush: Option<std::io::Error>,
}

impl SaveWarnings {
    /// The warnings as one block of text, in the order they happened, or `None` if there are none.
    pub fn message(&self, t: &'static Strings) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(e) = &self.backup {
            parts.push(fill(t.save_backup_failed, &[("err", &e.to_string())]));
        }
        if let Some(e) = &self.flush {
            parts.push(fill(t.save_flush_failed, &[("err", &e.to_string())]));
        }
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }
}

/// Validates, serializes, re-parses as a self-check, backs up the previous file, and atomically
/// replaces it. Never truncates the live file in place, and never leaves a half-written one:
/// the bytes are on the disk before the rename that makes them the expansions file.
///
/// Returns what degraded on the way, for the caller to put on screen.
///
/// The library is two files (see [`MatchFile::split`]), and they cannot be replaced in one step.
/// So the order is chosen to make the moment between the two harmless: when an expansion is being
/// switched on, `base.yml` gains it before the other file loses it; when one is being switched
/// off, the other file gains it before `base.yml` loses it. Interrupted at any point, an expansion
/// is in both files or in its old one, never in neither — and [`load`] reads one that is in both
/// as on. The file of switched-off expansions is only rewritten when it changes, and removed when
/// it would be empty, so a library that never switches anything off never has one.
pub fn save(path: &Path, mf: &MatchFile, backups_dir: &Path) -> Result<SaveWarnings, SaveError> {
    let issues = validate(mf);
    if !issues.is_empty() {
        return Err(SaveError::Validation(issues));
    }

    let (base, off) = mf.split();
    let rendered = base
        .to_string_with_banner()
        .map_err(SaveError::SelfCheckFailed)?;
    MatchFile::from_str(&rendered).map_err(SaveError::SelfCheckFailed)?;
    let off_rendered = if off.entries.is_empty() && off.extra_top_level.is_empty() {
        None
    } else {
        let text = off.to_string_as_off_file().map_err(SaveError::SelfCheckFailed)?;
        MatchFile::from_str(&text).map_err(SaveError::SelfCheckFailed)?;
        Some(text)
    };

    let off_path = off_path(path);
    let off_on_disk = std::fs::read_to_string(&off_path).ok();
    let mut warnings = SaveWarnings::default();
    // One backup for the whole save, taken before either file is touched — see [`backup`].
    if let Err(e) = backup(&[path, &off_path], backups_dir, &mut warnings) {
        warnings.backup = Some(e);
    }
    if off_rendered == off_on_disk {
        write_one(path, &rendered, &mut warnings)?;
        return Ok(warnings);
    }
    // Does the switched-off file lose anything it has on disk now? Then something is being
    // switched on, and `base.yml` goes first.
    let losing = off_on_disk
        .as_deref()
        .and_then(|text| MatchFile::from_str(text).ok())
        .is_some_and(|old| {
            old.entries.iter().any(|e| {
                matches!(e, MatchEntry::Simple(m) if !off.entries.iter().any(|n| n.trigger_str() == m.trigger))
            })
        });
    if losing {
        write_one(path, &rendered, &mut warnings)?;
    }
    match &off_rendered {
        Some(text) => write_one(&off_path, text, &mut warnings)?,
        None => std::fs::remove_file(&off_path).map_err(SaveError::Io)?,
    }
    if !losing {
        write_one(path, &rendered, &mut warnings)?;
    }
    Ok(warnings)
}

/// Replaces one file atomically with `rendered`. The backup was taken by the caller, once for the
/// whole save.
fn write_one(path: &Path, rendered: &str, warnings: &mut SaveWarnings) -> Result<(), SaveError> {
    // The rename is what makes this safe against a reader — espanso is watching this folder and
    // may open the file at any instant — but it is not, on its own, safe against the power going
    // out. NTFS journals the directory entry, not the file's contents, so a rename can survive a
    // hard reset while the bytes it points at have not been written yet: the expansions file
    // comes back empty. The flush is what closes that window.
    //
    // A failed flush is not a reason to throw the edit away, though. On a USB stick or a network
    // share — and this app is carried on both — FlushFileBuffers can refuse where the write
    // itself was fine. Same policy as the backup above: say so and carry on.
    let tmp_path = path.with_extension("yml.tmp");
    {
        let mut file = std::fs::File::create(&tmp_path).map_err(SaveError::Io)?;
        file.write_all(rendered.as_bytes()).map_err(SaveError::Io)?;
        if let Err(e) = file.sync_all() {
            warnings.flush = Some(e);
        }
    } // The handle closes here: renaming a file still open can fail with a sharing violation.

    // And if the rename fails, the temporary file does not stay behind. It would be sitting in
    // espanso's own match folder, which espanso reads.
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(SaveError::Io(e));
    }
    Ok(())
}

/// How many saves keep their backup.
const BACKUPS_KEPT: usize = 20;

/// One backup per save: a folder named after the moment, holding the library as it was — each of
/// `files` that exists, copied whole. Restoring is copying them back into the match folder.
///
/// One folder and not one copy per file, because the library is two files and a save can rewrite
/// both: two loose copies per save read as two backups, halved how many saves the cap kept, and
/// left whoever was restoring to work out which `_disabled.yml` went with which `base.yml`. Both
/// files go in even when only one is about to change, so every backup is the whole library on its
/// own.
fn backup(files: &[&Path], backups_dir: &Path, warnings: &mut SaveWarnings) -> std::io::Result<()> {
    let present: Vec<&Path> = files.iter().copied().filter(|p| p.exists()).collect();
    if present.is_empty() {
        // The first save ever: there is no previous library to keep.
        return Ok(());
    }
    std::fs::create_dir_all(backups_dir)?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Two saves within the same second each get their own folder; reusing the name would let the
    // second overwrite the only copy of what the first replaced.
    let mut folder: PathBuf = backups_dir.join(ts.to_string());
    let mut n = 2;
    while folder.exists() {
        folder = backups_dir.join(format!("{ts}-{n}"));
        n += 1;
    }
    std::fs::create_dir(&folder)?;
    for path in present {
        let Some(name) = path.file_name() else { continue };
        let copy = folder.join(name);
        std::fs::copy(path, &copy)?;
        // The backup is the only remaining copy of the previous file the moment the rename in
        // `write_one` lands, so it gets the same flush — and reopened for writing, because
        // FlushFileBuffers needs GENERIC_WRITE and a read handle would be refused outright.
        // Recorded rather than returned: failing here must not skip the pruning below, or the cap
        // quietly stops holding.
        //
        // It goes into `flush` and not into `backup`, because the backup exists — `copy` above
        // succeeded. What is unconfirmed is whether it reached the disk, which is word for word
        // what that warning says. `get_or_insert` so the live file's own flush, a few lines later
        // on the same drive and for the same reason, is the one the user is told about if both
        // fail.
        if let Err(e) = std::fs::OpenOptions::new()
            .write(true)
            .open(&copy)
            .and_then(|f| f.sync_all())
        {
            warnings.flush.get_or_insert(e);
        }
    }
    prune_old_backups(backups_dir, BACKUPS_KEPT)
}

/// Keeps the newest `keep` backups. Each folder is one; so is each loose `.bak` file left by
/// builds before 0.0.5, which age out under the same cap instead of lingering forever.
fn prune_old_backups(backups_dir: &Path, keep: usize) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(backups_dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    if entries.len() > keep {
        for e in &entries[..entries.len() - keep] {
            let path = e.path();
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::model::SimpleMatch;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("khipu-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn one_match(trigger: &str) -> MatchFile {
        MatchFile {
            entries: vec![MatchEntry::Simple(SimpleMatch {
                trigger: trigger.to_string(),
                replace: "hola".to_string(),
                vars: vec![],
                label: None,
            })],
            extra_top_level: Default::default(),
            ..Default::default()
        }
    }

    #[test]
    fn a_save_leaves_a_readable_file_and_no_scratch_file_beside_it() {
        let dir = scratch("save");
        let path = dir.join("base.yml");

        save(&path, &one_match(":uno"), &dir.join("backups")).unwrap();

        let reloaded = load(&path).unwrap();
        assert_eq!(reloaded.entries.len(), 1);
        assert_eq!(reloaded.entries[0].trigger_str(), ":uno");
        // The temporary file the atomic replace goes through must never be left in the folder
        // espanso watches.
        assert!(!path.with_extension("yml.tmp").exists());
    }

    #[test]
    fn saving_over_an_existing_file_replaces_it_and_keeps_a_backup() {
        let dir = scratch("backup");
        let path = dir.join("base.yml");
        let backups = dir.join("backups");

        save(&path, &one_match(":uno"), &backups).unwrap();
        save(&path, &one_match(":dos"), &backups).unwrap();

        assert_eq!(load(&path).unwrap().entries[0].trigger_str(), ":dos");
        assert!(!path.with_extension("yml.tmp").exists());

        let kept: Vec<_> = std::fs::read_dir(&backups).unwrap().flatten().collect();
        assert_eq!(kept.len(), 1, "the first version should have been kept");
        let previous = std::fs::read_to_string(kept[0].path().join("base.yml")).unwrap();
        assert!(previous.contains(":uno"), "backup holds the previous file");
    }

    /// The point of the backup folder: a save that rewrites both files — switching something off —
    /// still leaves one backup, and that one holds the whole library as it was.
    #[test]
    fn a_save_that_touches_both_files_leaves_one_backup_with_both() {
        let dir = scratch("onebackup");
        let path = dir.join("base.yml");
        let backups = dir.join("backups");
        let mut mf = two_matches();
        mf.off.insert(":dos".to_string());
        save(&path, &mf, &backups).unwrap();

        mf.off.clear();
        mf.off.insert(":uno".to_string());
        save(&path, &mf, &backups).unwrap();

        let kept: Vec<_> = std::fs::read_dir(&backups).unwrap().flatten().collect();
        assert_eq!(kept.len(), 1, "one save, one backup");
        let folder = kept[0].path();
        assert!(folder.is_dir());
        let base = std::fs::read_to_string(folder.join("base.yml")).unwrap();
        let off = std::fs::read_to_string(folder.join(crate::yaml::model::OFF_FILE)).unwrap();
        assert!(base.contains(":uno") && !base.contains(":dos"), "base.yml as it was");
        assert!(off.contains(":dos") && !off.contains(":uno"), "and its switched-off file with it");
    }

    /// Saves within the same second are the normal case when dragging things around; each keeps
    /// its own backup rather than the second overwriting the first.
    #[test]
    fn saves_in_the_same_second_keep_separate_backups() {
        let dir = scratch("samesecond");
        let path = dir.join("base.yml");
        let backups = dir.join("backups");
        save(&path, &one_match(":uno"), &backups).unwrap();
        save(&path, &one_match(":dos"), &backups).unwrap();
        save(&path, &one_match(":tres"), &backups).unwrap();
        let kept = std::fs::read_dir(&backups).unwrap().count();
        assert_eq!(kept, 2, "the two saves that replaced something each kept a backup");
    }

    /// The cap counts saves, and the loose `.bak` files of older builds age out under it.
    #[test]
    fn pruning_counts_folders_and_old_loose_files_alike() {
        let dir = scratch("prune");
        let backups = dir.join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        std::fs::write(backups.join("base.yml.1.bak"), "old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        for i in 0..3 {
            let folder = backups.join(format!("10{i}"));
            std::fs::create_dir(&folder).unwrap();
            std::fs::write(folder.join("base.yml"), "x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        prune_old_backups(&backups, 2).unwrap();
        let mut left: Vec<String> = std::fs::read_dir(&backups)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        left.sort();
        assert_eq!(left, ["101", "102"]);
    }

    /// Validation runs before anything touches the disk, so a rejected save leaves the stored file
    /// exactly as it was.
    #[test]
    fn a_rejected_save_does_not_touch_the_stored_file() {
        let dir = scratch("validation");
        let path = dir.join("base.yml");
        let backups = dir.join("backups");
        save(&path, &one_match(":uno"), &backups).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();

        let mut duplicated = one_match(":dup");
        duplicated.entries.push(duplicated.entries[0].clone());
        let result = save(&path, &duplicated, &backups);

        assert!(matches!(result, Err(SaveError::Validation(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        assert!(!path.with_extension("yml.tmp").exists());
    }

    #[test]
    fn a_save_with_nothing_to_report_reports_nothing() {
        let dir = scratch("quiet");
        let path = dir.join("base.yml");
        let warnings = save(&path, &one_match(":uno"), &dir.join("backups")).unwrap();
        assert!(warnings.backup.is_none());
        assert!(warnings.flush.is_none());
    }

    #[test]
    fn a_backup_that_cannot_be_made_is_handed_back_rather_than_printed() {
        let dir = scratch("nobackup");
        let path = dir.join("base.yml");
        // There has to be a previous file for a backup to be attempted at all.
        save(&path, &one_match(":uno"), &dir.join("backups")).unwrap();

        // A backups folder that can never be created: its parent is a file.
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();

        let warnings = save(&path, &one_match(":dos"), &blocker.join("backups")).unwrap();
        assert!(
            warnings.backup.is_some(),
            "the failure has to come back to the caller"
        );
        // And the edit still went through, which is the whole reason it is a warning and not an
        // error.
        let reloaded = load(&path).unwrap();
        assert_eq!(reloaded.entries[0].trigger_str(), ":dos");
    }

    fn two_matches() -> MatchFile {
        let mut mf = one_match(":uno");
        mf.entries.extend(one_match(":dos").entries);
        mf
    }

    #[test]
    fn switching_off_moves_the_expansion_to_the_file_espanso_skips_and_back() {
        let dir = scratch("off");
        let path = dir.join("base.yml");
        let backups = dir.join("backups");
        let mut mf = two_matches();
        save(&path, &mf, &backups).unwrap();
        assert!(!off_path(&path).exists(), "nothing off, no second file");

        mf.off.insert(":dos".to_string());
        save(&path, &mf, &backups).unwrap();
        let base = std::fs::read_to_string(&path).unwrap();
        let off = std::fs::read_to_string(off_path(&path)).unwrap();
        assert!(base.contains(":uno") && !base.contains(":dos"));
        assert!(off.contains(":dos") && !off.contains(":uno"));
        assert!(off_path(&path).file_name().unwrap().to_string_lossy().starts_with('_'));

        let reloaded = load(&path).unwrap();
        assert_eq!(reloaded.entries.len(), 2);
        assert!(reloaded.is_off(":dos") && !reloaded.is_off(":uno"));

        mf.off.clear();
        save(&path, &mf, &backups).unwrap();
        assert!(!off_path(&path).exists(), "an empty second file is removed, not left empty");
        assert!(std::fs::read_to_string(&path).unwrap().contains(":dos"));
    }

    #[test]
    fn a_library_that_is_all_switched_off_still_loads() {
        let dir = scratch("alloff");
        let path = dir.join("base.yml");
        std::fs::write(off_path(&path), "matches:\n  - trigger: \":uno\"\n    replace: hola\n").unwrap();
        let mf = load(&path).unwrap();
        assert_eq!(mf.entries.len(), 1);
        assert!(mf.is_off(":uno"));
    }

    /// What a save interrupted between its two files leaves: the expansion in both. The copy that
    /// fires wins; an identical one in the other file is the leftover and goes, a different one
    /// is someone's text and stays.
    #[test]
    fn an_expansion_in_both_files_is_on_and_only_a_different_copy_is_kept() {
        let dir = scratch("both");
        let path = dir.join("base.yml");
        std::fs::write(&path, "matches:\n  - trigger: \":uno\"\n    replace: hola\n").unwrap();
        std::fs::write(
            off_path(&path),
            "matches:\n  - trigger: \":uno\"\n    replace: hola\n  - trigger: \":uno\"\n    replace: otra\n",
        )
        .unwrap();
        let mf = load(&path).unwrap();
        assert_eq!(mf.entries.len(), 1);
        assert!(!mf.is_off(":uno"));
        assert_eq!(mf.off_kept.len(), 1);

        save(&path, &mf, &dir.join("backups")).unwrap();
        let off = std::fs::read_to_string(off_path(&path)).unwrap();
        assert!(off.contains("otra") && !off.contains("hola"));
    }
}
