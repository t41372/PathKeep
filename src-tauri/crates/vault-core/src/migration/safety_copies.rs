//! Responsibilities: say which entries of `raw-snapshots/` hold safety copies, the ones the restore
//! list offers: `maintenance/`, where PathKeep keeps its own, and every folder holding a copy
//! PathKeep 0.3 made (`*.sqlite`, as the converter's listing finds them). A move file leaves them on
//! the computer that made them, each being a restore point of that computer the size of the whole
//! archive (the user's decision, 2026-11-15). An import replaces the rest of `raw-snapshots/` (the
//! browsers' history files a backup copied) entry by entry, keeps this computer's copies where they
//! are, and adds the copies a file of PathKeep 0.5 or earlier brings beside them, each once.
//!
//! Not responsible for: listing or restoring a copy (`maintenance::snapshot`, the converter's
//! `list_legacy_snapshots`), the install and its recovery (`maintenance::install`), what else an
//! import replaces (the `vault_layout` import policies).
//!
//! Failure modes: a copy packed into a move file; this computer's copies moved into the "before
//! import" copy, where the restore list does not show them and freeing that copy deletes them, as
//! when a folder that could not be looked at is taken for one without a copy; a copy the file brings
//! put over a different one of the same name, or dropped as if it were that one; the same copy added
//! twice, or again by a second import of the same file; an old-format copy separated from the files
//! SQLite keeps beside it, taken for another because SQLite rebuilt its index, taken for the same
//! one although its log differs, or put beside a log left under the name it gets; a browser's
//! history file kept as if it were a copy, or left out of the move; a copy read in full although no
//! copy here has its name.
//!
//! Tests: `maintenance::move_copies_tests`.
use anyhow::{Context, Result};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};
use vault_layout::ArtifactId as A;

/// `raw-snapshots`, relative to the archive folder.
fn raw_snapshots() -> &'static Path {
    Path::new(vault_layout::spec(A::RawSnapshots).path.relative())
}

/// The entries of the `raw-snapshots/` folder at `dir` that hold safety copies. A folder that holds
/// one is left alone as a whole: what else is in it is kept with it, never taken for a browser's
/// history file.
pub(crate) fn homes(dir: &Path) -> Result<BTreeSet<OsString>> {
    let native = vault_layout::spec(A::NativeSnapshots).path.relative();
    let native = Path::new(native).strip_prefix(raw_snapshots())?.as_os_str().to_owned();
    let mut homes = BTreeSet::new();
    for name in names(dir)? {
        if name == native || holds_old_format_copy(&dir.join(&name))? {
            homes.insert(name);
        }
    }
    Ok(homes)
}

/// What an export leaves out of the artifact tree `id`: entries of its folder at `dir`.
pub(crate) fn left_behind(id: A, dir: &Path) -> Result<BTreeSet<OsString>> {
    match id {
        A::RawSnapshots => homes(dir),
        _ => Ok(BTreeSet::new()),
    }
}

/// What an import installs in `raw-snapshots/`, each relative to the archive folder:
///
/// - every entry of the folder, here or in the stage, that is not a safety copy: replaced by the
///   stage's, or taken away when the stage has none of that name;
/// - every copy the stage brings that this computer does not have yet. One this computer has, the
///   same in every byte, is left in the stage; one whose name a different copy here has is
///   renamed in the stage first.
///
/// This computer's copies are not touched.
pub(crate) fn installation(live_root: &Path, stage_root: &Path) -> Result<Vec<PathBuf>> {
    let raw = raw_snapshots();
    let (live, staged) = (live_root.join(raw), stage_root.join(raw));
    let homes: BTreeSet<OsString> = homes(&live)?.union(&homes(&staged)?).cloned().collect();
    let mut replaced = BTreeSet::new();
    for side in [&live, &staged] {
        for name in names(side)? {
            if !homes.contains(&name) {
                replaced.insert(raw.join(name));
            }
        }
    }
    let mut files: Vec<PathBuf> = replaced.into_iter().collect();
    for home in &homes {
        files.extend(added(live_root, stage_root, &raw.join(home))?);
    }
    Ok(files)
}

/// The copies the stage brings in `home` (relative to the archive folder) that this computer does
/// not have, as install entries. A copy here is the same one when it has the copy's name, or a name
/// an earlier import gave it (`Unit::named`), and holds the same: the same file imported again adds
/// nothing. A copy is read only to compare it with one here under such a name; one none of whose
/// names is taken here is installed unread.
fn added(live_root: &Path, stage_root: &Path, home: &Path) -> Result<Vec<PathBuf>> {
    let (here, staged) = (live_root.join(home), stage_root.join(home));
    let held = names(&here)?;
    let mut entries = Vec::new();
    for unit in units(&staged)? {
        let mut numbers: Vec<u32> = held.iter().filter_map(|name| unit.number_of(name)).collect();
        numbers.sort_unstable();
        let mut known = false;
        for n in std::iter::once(1).chain(numbers) {
            if unit.held_at(&here, n)? && unit.same_at(&here, n, &staged)? {
                known = true;
                break;
            }
        }
        if known {
            continue;
        }
        let mut n = 1;
        while !(unit.free_at(&here, n)? && (n == 1 || unit.free_at(&staged, n)?)) {
            n += 1;
        }
        if n > 1 {
            unit.rename(&staged, n)?;
        }
        entries.extend(unit.present.iter().map(|suffix| home.join(unit.member(n, suffix))));
    }
    Ok(entries)
}

/// A copy in a home folder: an old-format copy is its database file and whichever of the files
/// SQLite keeps beside it are there; anything else, a native copy's folder included, is one name.
struct Unit {
    /// The database file of an old-format copy, or the one name.
    name: OsString,
    old_format: bool,
    /// What the folder holds of it, as the suffixes of `name` (`companion_suffixes`): the database
    /// file or the one name (empty) first.
    present: Vec<&'static str>,
}

impl Unit {
    /// The suffixes of the names this copy is made of; one, empty, for a copy that is one name.
    fn suffixes(&self) -> &'static [&'static str] {
        if self.old_format { companion_suffixes() } else { &[""] }
    }

    /// The copy's name numbered `n`: its own for 1, then `name-2`, `name-3`, … before the
    /// extension of an old-format copy (`old-2.sqlite`), so the files SQLite keeps beside it keep
    /// going with it (`old-2.sqlite-wal`).
    fn named(&self, n: u32) -> OsString {
        if n == 1 {
            return self.name.clone();
        }
        let (stem, extension) = self.split();
        format!("{stem}-{n}{extension}").into()
    }

    /// The name numbered `n` of the file of this copy with `suffix`.
    fn member(&self, n: u32, suffix: &str) -> OsString {
        let mut member = self.named(n);
        member.push(suffix);
        member
    }

    /// The number in `name` when it is this copy's name numbered 2 or more (`named`).
    fn number_of(&self, name: &OsString) -> Option<u32> {
        let (stem, extension) = self.split();
        let number = name.to_str()?.strip_prefix(&stem)?.strip_prefix('-')?;
        let number = number.strip_suffix(&extension)?;
        let n: u32 = number.parse().ok()?;
        (n >= 2 && n.to_string() == number).then_some(n)
    }

    fn split(&self) -> (String, String) {
        let name = self.name.to_string_lossy().into_owned();
        let at = if self.old_format {
            name.find(&format!(".{}", old_format_extension())).unwrap_or(name.len())
        } else {
            name.len()
        };
        (name[..at].to_owned(), name[at..].to_owned())
    }

    /// Whether `dir` holds this copy under its name numbered `n`: its database file, for an
    /// old-format copy.
    fn held_at(&self, dir: &Path, n: u32) -> Result<bool> {
        Ok(exists(&dir.join(self.named(n)))?)
    }

    /// Whether nothing of a copy is under the name numbered `n` in `dir`: for an old-format copy,
    /// none of its database's files, as a log left there would be read into a copy put beside it.
    fn free_at(&self, dir: &Path, n: u32) -> Result<bool> {
        for suffix in self.suffixes() {
            if exists(&dir.join(self.member(n, suffix)))? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether the copy under the name numbered `n` in `here` holds what this one in `staged` holds.
    /// For an old-format copy, the same database: the same file, and the same log, and the same
    /// journal, or none, where an empty one is none, as SQLite reads it. Not the `-shm` file, an
    /// index SQLite builds again from the log when it opens the database, written by every open,
    /// read-only ones too: one says nothing about which copy this is.
    fn same_at(&self, here: &Path, n: u32, staged: &Path) -> Result<bool> {
        for suffix in self.suffixes().iter().filter(|suffix| **suffix != "-shm") {
            let held = content(&here.join(self.member(n, suffix)), !suffix.is_empty())?;
            if held != content(&staged.join(self.member(1, suffix)), !suffix.is_empty())? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Renames the copy in the stage to its name numbered `n`, each of its files.
    fn rename(&self, staged: &Path, n: u32) -> Result<()> {
        for suffix in &self.present {
            let (from, to) = (self.member(1, suffix), self.member(n, suffix));
            crate::durable_io::retry_while_held(|| {
                fs::rename(staged.join(&from), staged.join(&to))
            })
            .with_context(|| {
                format!("renaming the copy {} in the stage", Path::new(&from).display())
            })?;
        }
        Ok(())
    }
}

/// The copies in a home folder.
fn units(dir: &Path) -> Result<Vec<Unit>> {
    let names: BTreeSet<OsString> = names(dir)?.into_iter().collect();
    let mut grouped = BTreeSet::new();
    let mut units = Vec::new();
    for name in names.iter().filter(|name| is_old_format_copy(Path::new(name))) {
        let unit = Unit { name: name.clone(), old_format: true, present: Vec::new() };
        let present: Vec<&'static str> = companion_suffixes()
            .iter()
            .copied()
            .filter(|suffix| names.contains(&unit.member(1, suffix)))
            .collect();
        grouped.extend(present.iter().map(|suffix| unit.member(1, suffix)));
        units.push(Unit { present, ..unit });
    }
    units.extend(names.into_iter().filter(|name| !grouped.contains(name)).map(|name| Unit {
        name,
        old_format: false,
        present: vec![""],
    }));
    Ok(units)
}

/// Whether the folder at `path` holds a copy PathKeep 0.3 made, as the converter's listing finds
/// them: a folder (a link to one too) with a `*.sqlite` entry in it. One that cannot be looked at (a
/// disk error, a permission) stops the move: taken for one without a copy, it would be replaced, its
/// copies moved into the "before import" copy, or packed into a move file.
fn holds_old_format_copy(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    }
    Ok(names(path)?.iter().any(|name| is_old_format_copy(Path::new(name))))
}

fn is_old_format_copy(name: &Path) -> bool {
    name.extension().is_some_and(|extension| extension == old_format_extension())
}

fn old_format_extension() -> &'static str {
    vault_layout::family_extension(A::OtherLegacySnapshots)
}

fn companion_suffixes() -> &'static [&'static str] {
    vault_layout::CompanionFamily::LegacySqlite.suffixes(vault_layout::CompanionUse::Recovery)
}

/// The entries of the folder at `dir` that belong to PathKeep, without the files a file manager
/// drops into folders; none when there is no such folder.
fn names(dir: &Path) -> Result<Vec<OsString>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", dir.display())),
    };
    let mut names = Vec::new();
    for entry in entries {
        let name = entry.with_context(|| format!("reading {}", dir.display()))?.file_name();
        if !vault_layout::is_os_metadata(&name.to_string_lossy()) {
            names.push(name);
        }
    }
    Ok(names)
}

/// What the file or folder at `path` holds; `None` when nothing is there, or, with `empty_is_none`,
/// an empty file.
fn content(path: &Path, empty_is_none: bool) -> Result<Option<String>> {
    if empty_is_none && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.len() == 0) {
        return Ok(None);
    }
    crate::content_identity::of(path).with_context(|| format!("reading {}", path.display()))
}

fn exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    // A folder that cannot be looked at is not one without a copy in it (see
    // `holds_old_format_copy`).
    #[test]
    fn a_folder_that_cannot_be_looked_at_is_not_taken_for_one_without_a_copy() {
        let raw = tempfile::tempdir().unwrap();
        let bucket = raw.path().join("periodic");
        fs::create_dir(&bucket).unwrap();
        fs::write(bucket.join("old.sqlite"), b"SQLite format 3\0").unwrap();
        let mode = fs::metadata(raw.path()).unwrap().permissions().mode();
        // Readable, so its entries are listed, but not searchable, so none of them can be looked at.
        fs::set_permissions(raw.path(), fs::Permissions::from_mode(0o600)).unwrap();
        let homes = super::homes(raw.path());
        fs::set_permissions(raw.path(), fs::Permissions::from_mode(mode)).unwrap();
        assert!(homes.is_err(), "{homes:?}");
    }
}
