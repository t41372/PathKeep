//! Responsibilities: project operation members and relative paths from the catalog.
//! Not responsible for: filesystem traversal, journal recovery or nested-format validation.

use crate::*;
use std::path::{Path, PathBuf};

/// Names the producer owning a variable set; this is not an unknown-file recognizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFamily {
    SearchGenerations,
    NativeSnapshots,
    LegacySnapshots,
    LegacyDatabaseTemps,
    MaintenanceStages,
    PreviousArchives,
    RootBackups,
    AtomicTemps,
    ReservedControls,
}
/// Existing name-selection rules; broad backup matching is intentionally retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamilyPattern {
    Children,
    Prefix(&'static str),
    Contains(&'static str),
    Extension(&'static str),
}
impl FamilyPattern {
    /// Reuses the producer's existing selection without granting any new ownership.
    pub fn matches(self, name: &str) -> bool {
        match self {
            Self::Children => true,
            Self::Prefix(prefix) => name.starts_with(prefix),
            Self::Contains(part) => name.contains(part),
            Self::Extension(extension) => name.ends_with(&format!(".{extension}")),
        }
    }
}
/// A fixed root-relative path or a producer-owned family below a fixed parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRule {
    Exact(&'static str),
    Family { parent: &'static str, family: FileFamily, pattern: FamilyPattern },
}
impl PathRule {
    /// Families resolve to their containing directory; their format owner resolves children.
    pub const fn relative(self) -> &'static str {
        match self {
            Self::Exact(path) => path,
            Self::Family { parent, .. } => parent,
        }
    }
}
/// Engine file families have different suffix coverage in different operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompanionFamily {
    Native,
    LegacySqlite,
}
/// These distinctions preserve current reset/install/size/recovery behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompanionUse {
    Install,
    Reset,
    Usage,
    Assess,
    Recovery,
}
impl CompanionFamily {
    /// Main files are represented by the empty suffix, keeping relative ordering unchanged.
    pub const fn suffixes(self, operation: CompanionUse) -> &'static [&'static str] {
        match (self, operation) {
            (_, CompanionUse::Usage) => &["", "-wal"],
            (Self::Native, CompanionUse::Install) => &["", "-wal", "-shm", "-tshm"],
            (Self::LegacySqlite, CompanionUse::Recovery | CompanionUse::Install) => {
                &["", "-wal", "-shm", "-journal"]
            }
            (_, CompanionUse::Reset | CompanionUse::Assess)
            | (Self::Native, CompanionUse::Recovery) => &["", "-wal", "-shm"],
        }
    }
}
/// Traversal cannot lose a declared identity because ALL and the enum are generated together.
pub fn artifacts() -> impl Iterator<Item = ArtifactSpec> {
    ArtifactId::ALL.into_iter().map(spec)
}
/// The selected operation decides membership; directory placement never does.
pub fn databases() -> impl Iterator<Item = DatabaseSpec> {
    artifacts().filter_map(|s| s.database)
}
/// The artifacts an archive of `layout` holds.
pub fn artifacts_in(layout: ArchiveLayout) -> impl Iterator<Item = ArtifactSpec> {
    artifacts().filter(move |s| s.introduced_in <= layout)
}
/// The stores an archive of `layout` opens, in no particular order (`startup_databases` orders).
pub fn databases_in(layout: ArchiveLayout) -> Vec<Db> {
    artifacts_in(layout)
        .filter(|s| matches!(s.lifecycle.open, OpenPolicy::Database(_)))
        .map(|s| s.database.expect("database open policy").db)
        .collect()
}
/// Resolves a default archive path without accessing the filesystem.
pub fn path(root: &Path, id: ArtifactId) -> PathBuf {
    root.join(spec(id).path.relative())
}
/// Provides stable database names to typed path views and independent consumers.
pub const fn database_path(db: Db) -> &'static str {
    spec(db_artifact(db)).path.relative()
}
/// Connection owners must start dependency workers before attached consumers.
pub fn startup_databases() -> Vec<Db> {
    let mut ordered: Vec<_> = artifacts()
        .filter_map(|s| match s.lifecycle.open {
            OpenPolicy::Database(order) => {
                Some((order, s.database.expect("database open policy").db))
            }
            _ => None,
        })
        .collect();
    ordered.sort_by_key(|(order, _)| *order);
    ordered.into_iter().map(|(_, db)| db).collect()
}
/// Only checkpoint-copy members participate in a Store snapshot.
pub fn snapshot_databases(layout: ArchiveLayout) -> impl Iterator<Item = Db> {
    artifacts_in(layout)
        .filter(|s| s.lifecycle.snapshot == SnapshotPolicy::CopyDatabase)
        .map(|s| s.database.unwrap().db)
}
/// Keyed mode copies exclude the disposable database and separately rebuilt index.
pub fn mode_change_databases(layout: ArchiveLayout) -> impl Iterator<Item = Db> {
    artifacts_in(layout)
        .filter(|s| s.lifecycle.mode_change == ModeChangePolicy::CopyDatabase)
        .map(|s| s.database.unwrap().db)
}
/// Preserves the header checker's limited coverage, including its scratch omission.
pub fn at_rest_databases(layout: ArchiveLayout) -> impl Iterator<Item = Db> {
    artifacts_in(layout)
        .filter(|s| s.lifecycle.at_rest == AtRestPolicy::NativeDatabase)
        .map(|s| s.database.unwrap().db)
}
/// Native installation includes every companion, while search is caller-selected.
pub fn installation_files(layout: ArchiveLayout, search: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for s in artifacts_in(layout) {
        match s.lifecycle.restore {
            RestorePolicy::RequiredDatabase | RestorePolicy::RecreateDatabase => {
                files.extend(
                    s.database
                        .unwrap()
                        .companions
                        .suffixes(CompanionUse::Install)
                        .iter()
                        .map(|suffix| PathBuf::from(format!("{}{suffix}", s.path.relative()))),
                );
            }
            RestorePolicy::Metadata => files.push(s.path.relative().into()),
            RestorePolicy::RebuildSearch if search => files.push(s.path.relative().into()),
            RestorePolicy::RebuildSearch | RestorePolicy::Preserve => {}
        }
    }
    files
}
/// Snapshot verification has stricter required membership than sealing existing files.
pub fn required_snapshot_files(
    layout: ArchiveLayout,
    encrypted: bool,
) -> impl Iterator<Item = &'static str> {
    artifacts_in(layout)
        .filter(move |s| match s.lifecycle.snapshot {
            SnapshotPolicy::CopyDatabase | SnapshotPolicy::SaveConfig => true,
            SnapshotPolicy::CopyEnvelope => encrypted,
            SnapshotPolicy::SealIfPresent | SnapshotPolicy::Omit => false,
        })
        .map(|s| s.path.relative())
}
/// Format-2 database membership also drives copy and integrity verification.
pub fn portable_databases(layout: ArchiveLayout) -> impl Iterator<Item = Db> {
    artifacts_in(layout)
        .filter(|s| matches!(s.lifecycle.portable, PortablePolicy::Database { .. }))
        .map(|s| s.database.unwrap().db)
}
/// Transport names can differ from live paths, especially for Ask's database.
pub fn portable_files(layout: ArchiveLayout) -> impl Iterator<Item = (ArtifactId, &'static str)> {
    artifacts_in(layout).filter_map(|s| match s.lifecycle.portable {
        PortablePolicy::Database { entry }
        | PortablePolicy::Envelope { entry }
        | PortablePolicy::SealedConfig { entry } => Some((s.id, entry)),
        PortablePolicy::ArtifactTree(_)
        | PortablePolicy::InArtifactTree
        | PortablePolicy::Omit(_)
        | PortablePolicy::SealedArtifacts { .. } => None,
    })
}
/// Only root artifact trees are packed; inherited child rows cannot double-copy them.
pub fn portable_roots() -> Vec<ArtifactId> {
    let mut roots: Vec<_> = artifacts()
        .filter_map(|s| match s.lifecycle.portable {
            PortablePolicy::ArtifactTree(order) => Some((order, s.id)),
            _ => None,
        })
        .collect();
    roots.sort_by_key(|(order, _)| *order);
    roots.into_iter().map(|(_, id)| id).collect()
}
/// Reproduces the existing export guard, including unprotected exports/logs/IPC locations.
pub fn export_target_is_managed(first: &str) -> bool {
    artifacts().any(|s| match s.lifecycle.export_target {
        ExportTargetPolicy::Deny => s.path.relative() == first,
        ExportTargetPolicy::DenyRootPrefix => family_pattern(s.id).matches(first),
        ExportTargetPolicy::Allow => false,
    })
}
/// A family carries its existing name rule in its own row.
pub const fn family_pattern(id: ArtifactId) -> FamilyPattern {
    match spec(id).path {
        PathRule::Family { pattern, .. } => pattern,
        PathRule::Exact(_) => panic!("artifact is not a file family"),
    }
}
/// Producers need the same prefix that cleanup selects, including legacy-compatible names.
pub const fn family_prefix(id: ArtifactId) -> &'static str {
    match family_pattern(id) {
        FamilyPattern::Prefix(prefix) => prefix,
        _ => panic!("family is not prefix-based"),
    }
}
/// Keeps deletion order stable without deriving it from enum discriminants.
pub fn wipe_targets() -> Vec<ArtifactId> {
    let mut targets: Vec<_> = artifacts()
        .filter_map(|s| match s.lifecycle.wipe {
            WipePolicy::Remove(order) => Some((order, s.id)),
            _ => None,
        })
        .collect();
    targets.sort_by_key(|(order, _)| *order);
    targets.into_iter().map(|(_, id)| id).collect()
}
/// Returns measured contributions without counting containers and their children twice.
pub fn usage_members(layout: ArchiveLayout) -> impl Iterator<Item = ArtifactSpec> {
    artifacts_in(layout).filter(|s| s.lifecycle.usage.bucket.is_some())
}
/// Retention's public order is independent of the broader layout order.
pub fn retention_members() -> Vec<(ArtifactId, RetentionBucket)> {
    let mut members: Vec<_> =
        artifacts().filter_map(|s| s.lifecycle.retention.map(|bucket| (s.id, bucket))).collect();
    members.sort_by_key(|(_, bucket)| *bucket as usize);
    members
}
/// Doctor compares registered schemas only for the roles it actually checks.
pub fn health_databases(
    layout: ArchiveLayout,
) -> impl Iterator<Item = (Db, &'static str, &'static str)> {
    artifacts_in(layout).filter_map(|s| match s.lifecycle.health {
        HealthPolicy::Database { name, label } => Some((s.database.unwrap().db, name, label)),
        HealthPolicy::NotChecked => None,
    })
}
/// Legacy conversion membership cannot accidentally acquire a newly added native role.
pub fn conversion_sources() -> impl Iterator<Item = (Db, ArtifactId)> {
    artifacts().filter_map(|s| match s.lifecycle.conversion {
        ConversionPolicy::Source { db, .. } => Some((db, s.id)),
        ConversionPolicy::Settings { .. } | ConversionPolicy::Rebuild | ConversionPolicy::Omit => {
            None
        }
    })
}
/// The currently disposable role is explicit in the table, not inferred from a folder.
pub fn scratch_database() -> Db {
    artifacts()
        .find(|s| s.lifecycle.mode_change == ModeChangePolicy::Recreate)
        .unwrap()
        .database
        .unwrap()
        .db
}

/// Config saves scaffold only the existing set, including independently configured DB parents.
pub fn scaffolding() -> Vec<(ArtifactId, bool)> {
    let mut entries: Vec<_> = artifacts()
        .filter_map(|s| match s.lifecycle.open {
            OpenPolicy::EnsureContainer(order) => Some((order, s.id, false)),
            OpenPolicy::Database(_) => {
                s.database.unwrap().scaffold_parent.map(|order| (order, s.id, true))
            }
            OpenPolicy::NotOpened => None,
        })
        .collect();
    entries.sort_by_key(|(order, _, _)| *order);
    entries.into_iter().map(|(_, id, parent)| (id, parent)).collect()
}
/// Current launch/reconcile/discovery deliberately use one authoritative native header.
pub fn primary_database() -> Db {
    databases().find(|db| db.primary).unwrap().db
}
/// Rewrap membership stays separate from mode-change membership and historical copies.
pub fn password_envelope() -> ArtifactId {
    artifacts().find(|s| s.lifecycle.password_change == PasswordChangePolicy::Rewrap).unwrap().id
}
/// Sealing syncs the distinct installation parents in their established order.
pub fn snapshot_directories(layout: ArchiveLayout) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for file in installation_files(layout, false) {
        let parent = file.parent().unwrap().to_owned();
        if !directories.contains(&parent) {
            directories.push(parent);
        }
    }
    directories
}

/// Existing producers sometimes need only a leaf name beside a caller-supplied database.
pub const fn file_name(id: ArtifactId) -> &'static str {
    let path = spec(id).path.relative();
    let bytes = path.as_bytes();
    let mut index = bytes.len();
    while index > 0 {
        if bytes[index - 1] == b'/' {
            // ASCII slash ends at a UTF-8 boundary; byte slicing is const on supported Rust.
            return match std::str::from_utf8(bytes.split_at(index).1) {
                Ok(name) => name,
                Err(_) => panic!("invalid path boundary"),
            };
        }
        index -= 1;
    }
    path
}

/// Each supported transport member has one name shared by pack, preview, unpack and key reading.
pub const fn portable_entry(id: ArtifactId) -> &'static str {
    match spec(id).lifecycle.portable {
        PortablePolicy::Database { entry }
        | PortablePolicy::Envelope { entry }
        | PortablePolicy::SealedConfig { entry }
        | PortablePolicy::SealedArtifacts { entry } => entry,
        PortablePolicy::ArtifactTree(_)
        | PortablePolicy::InArtifactTree
        | PortablePolicy::Omit(_) => panic!("artifact has no bundle entry"),
    }
}
/// Preview notes are selected by policy, not by guessing which omitted paths need explaining.
pub fn import_exclusions() -> Vec<(ArtifactId, ImportExclusion)> {
    let mut entries: Vec<_> = artifacts()
        .filter_map(|s| match s.lifecycle.portable {
            PortablePolicy::Omit(Some(reason)) => Some((s.id, reason)),
            _ => None,
        })
        .collect();
    entries.sort_by_key(|(_, reason)| *reason as usize);
    entries
}
/// Format-2 import intentionally replaces the complete derived tree and artifact roots.
pub fn portable_installation_files(layout: ArchiveLayout) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for s in artifacts_in(layout) {
        match s.lifecycle.move_import {
            ImportPolicy::NativeEntry => {
                if let Some(database) = s.database {
                    files.extend(
                        database
                            .companions
                            .suffixes(CompanionUse::Install)
                            .iter()
                            .map(|suffix| PathBuf::from(format!("{}{suffix}", s.path.relative()))),
                    );
                } else {
                    files.push(s.path.relative().into());
                }
            }
            ImportPolicy::ReplaceContainer => files.push(s.path.relative().into()),
            ImportPolicy::CoveredByContainer | ImportPolicy::Preserve => {}
        }
    }
    files
}

/// Older readers use fixed historical canonical coverage, not today's native role set.
pub fn legacy_canonical_files() -> impl Iterator<Item = ArtifactId> {
    artifacts()
        .filter(|s| {
            matches!(s.lifecycle.conversion, ConversionPolicy::Source { canonical: true, .. })
        })
        .map(|s| s.id)
}
/// The old format authenticates its password against this primary source before optional files.
pub fn legacy_primary() -> ArtifactId {
    artifacts()
        .find(|s| matches!(s.lifecycle.conversion, ConversionPolicy::Source { primary: true, .. }))
        .unwrap()
        .id
}
/// Format-1 required membership is independent of native format-2 transport membership.
pub fn legacy_bundle_required() -> impl Iterator<Item = &'static str> {
    artifacts()
        .filter(|s| {
            matches!(
                s.lifecycle.conversion,
                ConversionPolicy::Source { required_in_bundle: true, .. }
            )
        })
        .map(|s| s.path.relative())
}
/// Only folders containing old database aliases are swept after conversion.
pub fn legacy_database_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for s in artifacts().filter(|s| {
        matches!(
            s.lifecycle.conversion,
            ConversionPolicy::Source { .. } | ConversionPolicy::Rebuild
        )
    }) {
        let parent = Path::new(s.path.relative()).parent().unwrap().to_owned();
        if !directories.contains(&parent) {
            directories.push(parent);
        }
    }
    directories
}
/// Protocol-owned temporary and snapshot suffixes remain attached to their producer family.
pub const fn family_extension(id: ArtifactId) -> &'static str {
    match family_pattern(id) {
        FamilyPattern::Extension(extension) => extension,
        _ => panic!("family is not extension-based"),
    }
}
impl CompanionFamily {
    /// Preserves the filename filter used by old-format bundle cleanup.
    pub fn matches_database_name(self, name: &str, operation: CompanionUse) -> bool {
        let extension = match self {
            Self::Native => "db",
            Self::LegacySqlite => "sqlite",
        };
        self.suffixes(operation)
            .iter()
            .any(|suffix| name.ends_with(&format!(".{extension}{suffix}")))
    }
}

/// Legacy settings remain tied to their old transport name when newer bundles evolve.
pub const fn legacy_config_entry() -> &'static str {
    match spec(ArtifactId::Config).lifecycle.conversion {
        ConversionPolicy::Settings { entry } => entry,
        _ => panic!("configuration requires a legacy settings contract"),
    }
}

/// Files the operating system's file manager drops into any folder a person opens or copies:
/// Finder's `.DS_Store` and `._name` companions, Explorer's `desktop.ini` and `Thumbs.db`, KDE's
/// `.directory`. They can appear at every level of the data folder, belong to no artifact, and
/// never mean that an archive is there.
pub fn is_os_metadata(name: &str) -> bool {
    matches!(name, ".DS_Store" | ".localized" | "desktop.ini" | "Thumbs.db" | ".directory")
        || name.starts_with("._")
}
