//! Responsibilities: decide, from what is on disk, whether this build may use a data root, before
//! anything is created, repaired, deleted or unlocked in it.
//! Not responsible for: decoding settings (`config`), running recovery, opening stores, or
//! publishing a compatibility stamp.
//!
//! How this could fail, and what guards it:
//! - A build that cannot understand an archive changes it. Every way into a root is listed in
//!   `vault-worker/tests/root_admission/operations.rs`, which hashes the whole root around a
//!   refusal.
//! - A root whose `config.json` is lost next to intact databases is taken for a first run and
//!   initialized over. `census` decides that from what is in the root, never from a default.
//! - The opposite: a root with nothing in it that matters is refused, and the user is locked out
//!   of a fresh start. Each row of the layout table says whether its file means an archive is
//!   here (`CensusPolicy`); what a finished wipe leaves, what a first start creates, what the
//!   scheduler writes, and files that are not PathKeep's must all answer no.
//! - A wipe or an install that a crash cut short loses its automatic recovery, because the folder
//!   has no `config.json` for a moment: a wipe removes it, PathKeep 0.5's install moved it aside,
//!   and this build's install has none before the staged one lands where there was none (an import
//!   into a new folder), or after a power cut during the rename over it on a file system without a
//!   journal. Their records are recognized as [`Interrupted`].
//! - An import into a new data folder fails, or a crash during one locks the user out, because
//!   its staging folder or a half-written journal is taken for an archive.
//! - The stamp a newer build wrote is rewritten by a save. `Compatibility` keeps its exact bytes.
//!
//! Tests: `vault-core/tests/root_admission_states.rs`, `root_stamp_codec.rs`.

use serde::Deserialize;
use serde_json::value::RawValue;
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};
use vault_layout::{ArtifactId, CensusPolicy, PathRule};

/// The root revision this build was written for: revision 1 adds the agent-history stores, and this
/// build creates them (`maintenance::creation`). A build may claim a revision only once it creates
/// everything that revision requires; one that claimed it earlier would admit a root it cannot
/// serve. Nothing at run time can change it.
pub const PRODUCT_ROOT_LEVEL: u64 = 1;

/// The stamp this build writes into an archive it brings up to its layout: its own revision, and
/// floors that keep every older build out (none of them knows the stores this revision adds).
pub(crate) fn written_compatibility() -> Compatibility {
    let raw = format!(
        r#"{{"revision":{PRODUCT_ROOT_LEVEL},"minReader":{PRODUCT_ROOT_LEVEL},"minWriter":{PRODUCT_ROOT_LEVEL}}}"#
    );
    Compatibility {
        stamp: Stamp {
            revision: PRODUCT_ROOT_LEVEL,
            min_reader: PRODUCT_ROOT_LEVEL,
            min_writer: PRODUCT_ROOT_LEVEL,
        },
        raw: RawValue::from_string(raw).expect("a JSON object"),
    }
}

/// The stamp an archive gets when it is replaced by a restore or an import: the stamp of the
/// archive it replaces (or this build's, when that one has none it can read), unless the incoming
/// archive's stamp is newer without lowering either floor. An older stamp never lowers the
/// archive's; two stamps that each raise something the other does not are refused rather than
/// merged into one nobody wrote.
pub(crate) fn compose(
    target: Option<&Compatibility>,
    source: Option<&Compatibility>,
) -> Result<Compatibility, String> {
    let written = written_compatibility();
    let base = target.unwrap_or(&written);
    let Some(source) = source else { return Ok(base.clone()) };
    let (b, s) = (base.stamp(), source.stamp());
    let at_least =
        s.revision >= b.revision && s.min_reader >= b.min_reader && s.min_writer >= b.min_writer;
    let at_most =
        s.revision <= b.revision && s.min_reader <= b.min_reader && s.min_writer <= b.min_writer;
    if at_least {
        Ok(source.clone())
    } else if at_most {
        Ok(base.clone())
    } else {
        Err(format!(
            "archive-compatibility-conflict: the archive being replaced has revision {} with floors {}/{}, the incoming one revision {} with floors {}/{}",
            b.revision, b.min_reader, b.min_writer, s.revision, s.min_reader, s.min_writer
        ))
    }
}

/// The layout of the archives this build creates: a new archive, a converted one, an archive
/// brought up to date when it is opened.
pub const WRITTEN_LAYOUT: vault_layout::ArchiveLayout =
    vault_layout::ArchiveLayout::of_revision(PRODUCT_ROOT_LEVEL);

/// The only shape of a stamped `archiveMode` this build can read.
const ENVELOPE_FORMAT: u64 = 2;

/// Why a root was not admitted. The strings are a contract with the UI and with scripts that
/// read the worker's stderr.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    ReaderTooOld,
    WriterTooOld,
    EnvelopeTooNew,
    ControlMissing,
    ControlUnreadable,
    ControlInvalid,
    CompatibilityInvalid,
}

impl Reason {
    /// Every reason, for what has to handle each: the desktop gives each its own error code.
    pub const ALL: [Self; 7] = [
        Self::ReaderTooOld,
        Self::WriterTooOld,
        Self::EnvelopeTooNew,
        Self::ControlMissing,
        Self::ControlUnreadable,
        Self::ControlInvalid,
        Self::CompatibilityInvalid,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::ReaderTooOld => "archive-reader-too-old",
            Self::WriterTooOld => "archive-writer-too-old",
            Self::EnvelopeTooNew => "archive-config-format-too-new",
            Self::ControlMissing => "archive-control-missing",
            Self::ControlUnreadable => "archive-control-unreadable",
            Self::ControlInvalid => "archive-control-invalid",
            Self::CompatibilityInvalid => "archive-compatibility-invalid",
        }
    }

    /// A newer PathKeep can open the root as it is. Every other reason means the root's own
    /// control is damaged or gone and nothing can open it until that is repaired.
    pub const fn needs_newer_build(self) -> bool {
        matches!(self, Self::ReaderTooOld | Self::WriterTooOld | Self::EnvelopeTooNew)
    }
}

/// A root this build must leave exactly as it found it.
#[derive(Clone, Debug)]
pub struct Refusal {
    pub reason: Reason,
    pub root: PathBuf,
    /// The stamp that was read, when the control got that far.
    pub stamp: Option<Stamp>,
    pub detail: String,
}

impl Refusal {
    fn new(root: &Path, reason: Reason, detail: impl Into<String>) -> Self {
        Self { reason, root: root.to_owned(), stamp: None, detail: detail.into() }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason.code(), self.detail)
    }
}

impl std::error::Error for Refusal {}

/// The three numbers of a stamp, for reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    pub revision: u64,
    pub min_reader: u64,
    pub min_writer: u64,
}

/// A validated stamp together with the bytes it was read from. A root with a higher revision
/// that this build may still write keeps those bytes on every save, whatever they contain.
#[derive(Clone, Debug)]
pub struct Compatibility {
    stamp: Stamp,
    raw: Box<RawValue>,
}

impl Compatibility {
    pub fn stamp(&self) -> Stamp {
        self.stamp
    }

    /// Exactly what was on disk, including key order, spacing and keys this build does not know.
    pub fn raw(&self) -> &RawValue {
        &self.raw
    }
}

/// What `archiveMode` in a control file says, in either form.
#[derive(Clone, Debug)]
pub struct Control {
    /// `None` for the bare value v0.4.0 and v0.5.0 write.
    pub compatibility: Option<Compatibility>,
    /// The JSON string inside a stamped `archiveMode`. `config` decodes it with the same rules
    /// as the bare value.
    pub(crate) stamped_mode: Option<Box<RawValue>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionDecision {
    Admitted,
    ReaderTooOld,
    WriterTooOld,
}

/// The whole version rule. `revision` plays no part: a root with a higher revision whose floors
/// this build meets is one its writer declared this build still safe for.
pub const fn decide(level: u64, min_reader: u64, min_writer: u64) -> VersionDecision {
    if level < min_reader {
        VersionDecision::ReaderTooOld
    } else if level < min_writer {
        VersionDecision::WriterTooOld
    } else {
        VersionDecision::Admitted
    }
}

/// A record a crash left behind while `config.json` was, by design of the operation, not there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interrupted {
    /// "Delete all data": PathKeep 0.5 removed the control first; this build's wipe only loses it
    /// to a power cut during the rename over it. The user confirmed the deletion.
    Wipe,
    /// A restore, import or encryption change that brings a control, before the new one lands; or
    /// the bring-up of a released archive whose stamped control a power cut took.
    Install,
    /// The same, left by PathKeep 0.3 in its own journals. The conversion's launch recovery
    /// finishes or reports these.
    OldFormat,
}

/// What a root is, for a build that may use it.
#[derive(Clone, Debug)]
pub enum RootState {
    Admitted(Control),
    /// No control and nothing that says an archive was ever here.
    Uninitialized,
    /// No control, and a record that explains why. The operation's own recovery puts a control
    /// back or finishes the deletion; until then the root reads as not initialized.
    Interrupted(Interrupted),
}

/// Reads the control file's `archiveMode` and applies the version rule. Touches nothing.
pub fn parse_control(root: &Path, bytes: &[u8]) -> Result<Control, Refusal> {
    let control =
        parse_envelope(bytes).map_err(|(reason, detail)| Refusal::new(root, reason, detail))?;
    let Some(stamp) = control.compatibility.as_ref().map(Compatibility::stamp) else {
        return Ok(control);
    };
    let (reason, floor) = match decide(PRODUCT_ROOT_LEVEL, stamp.min_reader, stamp.min_writer) {
        VersionDecision::Admitted => return Ok(control),
        VersionDecision::ReaderTooOld => (Reason::ReaderTooOld, stamp.min_reader),
        VersionDecision::WriterTooOld => (Reason::WriterTooOld, stamp.min_writer),
    };
    Err(Refusal {
        reason,
        root: root.to_owned(),
        stamp: Some(stamp),
        detail: format!(
            "this archive needs PathKeep format level {floor}; this build is level {PRODUCT_ROOT_LEVEL}"
        ),
    })
}

/// The grammar of `archiveMode`, without the version rule: what the codec needs to read a stamp
/// and write it back. Decoding a control is not permission to use its root.
pub(crate) fn parse_envelope(bytes: &[u8]) -> Result<Control, (Reason, String)> {
    #[derive(Deserialize)]
    struct Probe<'a> {
        #[serde(rename = "archiveMode", borrow, default)]
        archive_mode: Option<&'a RawValue>,
    }
    #[derive(Deserialize)]
    struct Head {
        #[serde(rename = "formatVersion")]
        format_version: u64,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Envelope<'a> {
        #[serde(rename = "formatVersion")]
        _format_version: u64,
        #[serde(borrow)]
        mode: &'a RawValue,
        #[serde(borrow)]
        compatibility: &'a RawValue,
    }
    #[derive(Deserialize)]
    struct Floors {
        revision: u64,
        #[serde(rename = "minReader")]
        min_reader: u64,
        #[serde(rename = "minWriter")]
        min_writer: u64,
    }

    let invalid = |error: serde_json::Error| {
        (Reason::ControlInvalid, format!("parsing config json: {error}"))
    };
    // serde reads a struct from an array as well as from an object. No PathKeep wrote an array.
    let is_object = |text: &str| text.trim_start().starts_with('{');
    if !std::str::from_utf8(bytes).is_ok_and(is_object) {
        return Err((Reason::ControlInvalid, "parsing config json: not a JSON object".into()));
    }
    let probe: Probe = serde_json::from_slice(bytes).map_err(invalid)?;
    let Some(mode) = probe.archive_mode.filter(|raw| raw.get().starts_with('{')) else {
        // Absent, a bare mode, or something `config` will reject with the reader's own message.
        return Ok(Control { compatibility: None, stamped_mode: None });
    };
    let head: Head = serde_json::from_str(mode.get()).map_err(invalid)?;
    if head.format_version > ENVELOPE_FORMAT {
        return Err((
            Reason::EnvelopeTooNew,
            format!(
                "archiveMode has format {}; this build reads {ENVELOPE_FORMAT}",
                head.format_version
            ),
        ));
    }
    if head.format_version < ENVELOPE_FORMAT {
        return Err((
            Reason::ControlInvalid,
            format!("archiveMode has format {}, which no PathKeep wrote", head.format_version),
        ));
    }
    let envelope: Envelope = serde_json::from_str(mode.get()).map_err(invalid)?;
    if !is_object(envelope.compatibility.get()) {
        return Err((Reason::CompatibilityInvalid, "compatibility is not a JSON object".into()));
    }
    let floors: Floors = serde_json::from_str(envelope.compatibility.get())
        .map_err(|error| (Reason::CompatibilityInvalid, error.to_string()))?;
    let stamp = Stamp {
        revision: floors.revision,
        min_reader: floors.min_reader,
        min_writer: floors.min_writer,
    };
    if stamp.min_reader == 0
        || stamp.min_reader > stamp.min_writer
        || stamp.min_writer > stamp.revision
    {
        return Err((
            Reason::CompatibilityInvalid,
            format!(
                "need 1 <= minReader <= minWriter <= revision, found {}, {}, {}",
                stamp.min_reader, stamp.min_writer, stamp.revision
            ),
        ));
    }
    Ok(Control {
        compatibility: Some(Compatibility { stamp, raw: envelope.compatibility.to_owned() }),
        stamped_mode: Some(envelope.mode.to_owned()),
    })
}

fn control_path(root: &Path) -> PathBuf {
    vault_layout::path(root, ArtifactId::Config)
}

/// The control file's bytes, or `None` when there is no such file. A file that is there and
/// cannot be read is never treated as absent.
pub(crate) fn read_control(root: &Path) -> Result<Option<Vec<u8>>, Refusal> {
    match fs::read(control_path(root)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Refusal::new(root, Reason::ControlUnreadable, error.to_string())),
    }
}

/// The check before any write: a control that is on disk must be one this build may use.
///
/// A root without a control passes. Whether such a root is a first run or a damaged archive is
/// [`inspect`]'s question, asked where the answer changes what happens: when the config is read,
/// at start, and before initialization. Staging folders and test roots have stores before they
/// have a control, and nothing here may stop them.
///
/// "May use" covers the whole file: a control whose settings this build cannot decode is as
/// unusable as one with a stamp it is too old for, and is found out here, not after the first
/// folder has been created.
pub fn guard(root: &Path) -> Result<Option<Control>, Refusal> {
    read_control(root)?
        .map(|bytes| {
            let control = parse_control(root, &bytes)?;
            crate::config::check_settings(root, &bytes, &control)?;
            Ok(control)
        })
        .transpose()
}

/// Which stores the archive at `root` holds, from its control. The bare `archiveMode` of 0.4 and
/// 0.5, or no control at all, is the released layout; a stamp's revision says which one else.
pub fn root_layout(root: &Path) -> Result<vault_layout::ArchiveLayout, Refusal> {
    Ok(layout_of(guard(root)?.as_ref()))
}

/// The layout a control declares. See [`root_layout`].
pub fn layout_of(control: Option<&Control>) -> vault_layout::ArchiveLayout {
    control
        .and_then(|control| control.compatibility.as_ref())
        .map_or(vault_layout::ArchiveLayout::BrowsingV1, |compatibility| {
            vault_layout::ArchiveLayout::of_revision(compatibility.stamp().revision)
        })
}

/// The check before a restore or an import replaces a root's archive. A damaged or missing
/// control is what those operations repair, so only a control that needs a newer PathKeep stops
/// them: that one is intact, and replacing it would put an older archive over a newer one.
pub fn refuse_newer(root: &Path) -> Result<(), Refusal> {
    match guard(root) {
        Err(refusal) if refusal.reason.needs_newer_build() => Err(refusal),
        _ => Ok(()),
    }
}

/// Classifies a root without changing it.
pub fn inspect(root: &Path) -> Result<RootState, Refusal> {
    match guard(root)? {
        Some(control) => Ok(RootState::Admitted(control)),
        None => census(root),
    }
}

/// [`inspect`] for readers of the config: the control, or `None` when there is none to read.
///
/// An interrupted wipe or install reads as `None`, like a first run. That is what every release
/// has returned for it, and the recoveries are built on it: the wipe is finished at start and
/// before initialization, a native install when the session opens, an old-format one when the
/// shell asks for the upgrade assessment. A reader does not act on the record itself; the
/// operation that wrote it may be running in this process right now.
pub fn require_control(root: &Path) -> Result<Option<Control>, Refusal> {
    match inspect(root)? {
        RootState::Admitted(control) => Ok(Some(control)),
        RootState::Uninitialized | RootState::Interrupted(_) => Ok(None),
    }
}

/// Decides what a root without a control is. Every file is judged by the layout table's own
/// answer for it (`CensusPolicy`); nothing here keeps a list.
fn census(root: &Path) -> Result<RootState, Refusal> {
    let paths = crate::project_paths_with_root(root);
    let present = |policy: CensusPolicy| {
        vault_layout::artifacts()
            .filter(move |spec| spec.lifecycle.census == policy)
            .any(|spec| fs::symlink_metadata(vault_layout::path(root, spec.id)).is_ok())
    };
    // This build's intent holds the control the user confirmed and the emptied one it writes. A
    // wipe never removes the control before it, but a power cut during a write in the folder can,
    // on a file system without a journal: beside the trash it names, which binds it to this
    // folder, the wipe then settles as it would with either.
    if crate::archive::wipe::intent_belongs_here(&paths) {
        return Ok(RootState::Interrupted(Interrupted::Wipe));
    }
    if present(CensusPolicy::WipeRecord) {
        // The marker is written whole or not at all, so one that does not decode was not left by
        // a wipe. Without a control there is nothing else to say what the user asked for.
        return if crate::archive::wipe::legacy_marker_is_readable(&paths) {
            Ok(RootState::Interrupted(Interrupted::Wipe))
        } else {
            Err(Refusal::new(
                root,
                Reason::ControlMissing,
                "the delete-all-data record is unreadable",
            ))
        };
    }
    // PathKeep 0.3's journals describe an archive in the old format. Next to a native archive
    // they are stale, and its missing control is a missing control.
    let native =
        vault_layout::path(root, vault_layout::db_artifact(vault_layout::primary_database()))
            .exists();
    if present(CensusPolicy::InstallRecord) {
        // With a `stage` it is this format's journal. Without one, PathKeep 0.3's import wrote
        // it. Anything that is not a JSON object was written by neither.
        let journal = fs::read(vault_layout::path(root, ArtifactId::InstallJournal))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        return match journal {
            Some(journal) if journal.get("stage").is_some() => {
                Ok(RootState::Interrupted(Interrupted::Install))
            }
            Some(journal) if journal.is_object() && !native => {
                Ok(RootState::Interrupted(Interrupted::OldFormat))
            }
            _ => Err(Refusal::new(
                root,
                Reason::ControlMissing,
                "the install record is unreadable or belongs to an older format",
            )),
        };
    }
    if present(CensusPolicy::OldFormatRecord) && !native {
        return Ok(RootState::Interrupted(Interrupted::OldFormat));
    }
    // This build's install record is archive data like the stores: it keeps the control in place,
    // so without one the control was lost. Unless the install brings the control: then recovery
    // decides from what is there, and stops before anything moves where it is not the record's.
    if crate::maintenance::install::brings_control(root) {
        return Ok(RootState::Interrupted(Interrupted::Install));
    }
    // The record of a bring-up holds the stamped control it writes: without a control, a power
    // cut took it during that write, and recovery writes it, where the folder the record names
    // still holds the stores it made there, which binds the record to this folder.
    if crate::maintenance::creation::puts_control_back(&paths) {
        return Ok(RootState::Interrupted(Interrupted::Install));
    }
    match archive_files_until(root, true)?.into_iter().next() {
        None => Ok(RootState::Uninitialized),
        Some(path) => Err(Refusal::new(
            root,
            Reason::ControlMissing,
            format!("config.json is missing but {path} exists"),
        )),
    }
}

/// Every file under `root` that means an archive is there, by the rule a folder without
/// `config.json` is judged by. "Delete all data" uses it as a check that does not depend on its
/// own list of targets.
pub(crate) fn archive_files(root: &Path) -> Result<Vec<String>, Refusal> {
    archive_files_until(root, false)
}

fn archive_files_until(root: &Path, first: bool) -> Result<Vec<String>, Refusal> {
    let unreadable =
        |error: io::Error| Refusal::new(root, Reason::ControlUnreadable, error.to_string());
    let mut found = Vec::new();
    let mut pending = match fs::read_dir(root) {
        Ok(entries) => vec![(entries, String::new())],
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(found),
        Err(error) => return Err(unreadable(error)),
    };
    while let Some((entries, parent)) = pending.pop() {
        for entry in entries {
            let entry = entry.map_err(unreadable)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = if parent.is_empty() { name } else { format!("{parent}/{name}") };
            // Not followed: a link is judged by where it lies, like a file.
            if entry.file_type().map_err(unreadable)?.is_dir() {
                pending.push((fs::read_dir(entry.path()).map_err(unreadable)?, relative));
            } else if is_archive(&relative) {
                found.push(relative);
                if first {
                    return Ok(found);
                }
            }
        }
    }
    Ok(found)
}

/// Whether one file means an archive is here: the table's answer for the file itself, else for
/// each folder above it, up to the data folder, whose own answer covers what nothing else names.
fn is_archive(relative: &str) -> bool {
    let mut path = relative;
    // The temporary file of an interrupted durable write lies next to whatever was being written.
    // The table names that family only at the top of the folder; its answer holds everywhere.
    let name = path.rsplit('/').next().unwrap_or(path);
    // Finder and Explorer write their own files into any folder a person opens, `archive/`
    // included. Onboarding has a button that opens the data folder.
    if vault_layout::is_os_metadata(name) {
        return false;
    }
    if vault_layout::family_pattern(ArtifactId::AtomicTemps).matches(name) {
        match vault_layout::spec(ArtifactId::AtomicTemps).lifecycle.census {
            CensusPolicy::NotArchive => return false,
            CensusPolicy::Inherit => {}
            _ => return true,
        }
    }
    loop {
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        let mut answers = vault_layout::artifacts()
            .filter(|spec| match spec.path {
                PathRule::Exact(exact) => exact == path,
                PathRule::Family { parent: of, pattern, .. } => {
                    of == parent && pattern.matches(name)
                }
            })
            .map(|spec| spec.lifecycle.census)
            .filter(|answer| *answer != CensusPolicy::Inherit)
            .peekable();
        if answers.peek().is_some() {
            // Two families can claim one name (`.pk-x.bak-1`). The one that says archive wins.
            return answers.any(|answer| answer != CensusPolicy::NotArchive);
        }
        if path.is_empty() {
            return false;
        }
        path = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_layout::{FamilyPattern, WipePolicy};

    // A reason added to the enum must be added here (or this does not compile) and to `ALL` in
    // the same place, or the assertion fails: the desktop classifies only the reasons in `ALL`.
    #[test]
    fn all_lists_every_reason_once() {
        let place = |reason: Reason| match reason {
            Reason::ReaderTooOld => 0,
            Reason::WriterTooOld => 1,
            Reason::EnvelopeTooNew => 2,
            Reason::ControlMissing => 3,
            Reason::ControlUnreadable => 4,
            Reason::ControlInvalid => 5,
            Reason::CompatibilityInvalid => 6,
        };
        let places: Vec<usize> = Reason::ALL.iter().map(|reason| place(*reason)).collect();
        assert_eq!(places, (0..Reason::ALL.len()).collect::<Vec<_>>());
        assert_eq!(Reason::ALL.len(), place(Reason::CompatibilityInvalid) + 1);
    }

    // "Delete all data" leaves these rows in place on purpose. If one of them, or anything
    // inside it, counted as an archive, every completed wipe would end in a folder that can
    // never be set up again. The two answers sit in different columns of the table, so nothing
    // but this test keeps them from contradicting each other when a row is added.
    #[test]
    fn nothing_a_wipe_keeps_counts_as_an_archive() {
        for spec in vault_layout::artifacts() {
            if !matches!(
                spec.lifecycle.wipe,
                WipePolicy::PreserveRuntime
                    | WipePolicy::PreserveDiagnostics
                    | WipePolicy::PreserveUnselectedControl
            ) {
                continue;
            }
            let path = match spec.path {
                PathRule::Exact(path) => path.to_owned(),
                PathRule::Family { parent, pattern, .. } => {
                    let name = match pattern {
                        FamilyPattern::Children => "x".to_owned(),
                        FamilyPattern::Prefix(prefix) => format!("{prefix}x"),
                        FamilyPattern::Contains(part) => format!("a{part}x"),
                        FamilyPattern::Extension(extension) => format!("x.{extension}"),
                    };
                    if parent.is_empty() { name } else { format!("{parent}/{name}") }
                }
            };
            // The row itself when it is a file, and a file inside it when it is a folder.
            for candidate in [path.clone(), format!("{path}/x").trim_start_matches('/').to_owned()]
            {
                assert!(
                    candidate.is_empty() || !is_archive(&candidate),
                    "{:?} is kept by a wipe, yet {candidate} would count as an archive",
                    spec.id
                );
            }
        }
    }
}
