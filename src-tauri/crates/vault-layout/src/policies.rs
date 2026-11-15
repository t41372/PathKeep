//! Responsibilities: make every artifact's lifecycle answers mandatory and typed.
//! Not responsible for: performing operations or interpreting file contents.

use crate::Db;

/// Whether losing this artifact loses facts that cannot be rebuilt from retained data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentClass {
    Original,
    Rebuildable,
    Control,
}
/// Identifies whose data an artifact contains, independently of its directory name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataScope {
    Browsing,
    Assistant,
    Shared,
    NestedArchive,
}
/// Startup work and its established ordering, without executing it here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPolicy {
    Database(u8),
    EnsureContainer(u8),
    NotOpened,
}
/// Checkpoint copies differ from files merely sealed when already present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotPolicy {
    CopyDatabase,
    SaveConfig,
    CopyEnvelope,
    SealIfPresent,
    Omit,
}
/// Controls the native replacement set; search is an optional installation member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestorePolicy {
    RequiredDatabase,
    RecreateDatabase,
    Metadata,
    RebuildSearch,
    Preserve,
}
/// Layout identity is metadata for existing stores, not an on-disk config encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveLayout {
    BrowsingV1,
}
/// Existing preview reasons have fixed meanings and presentation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportExclusion {
    Rebuilt,
    MachineSecrets,
    Schedule,
}
/// Import replaces some containers whole, independently of what export includes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportPolicy {
    NativeEntry,
    ReplaceContainer,
    CoveredByContainer,
    Preserve,
}
/// Explicit format-2 transport decisions, including inherited artifact-tree membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortablePolicy {
    Database { entry: &'static str },
    Envelope { entry: &'static str },
    SealedConfig { entry: &'static str },
    ArtifactTree(u8),
    InArtifactTree,
    Omit(Option<ImportExclusion>),
    SealedArtifacts { entry: &'static str },
}
/// Password changes rewrap only the live envelope; historical copies retain their credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordChangePolicy {
    Rewrap,
    Preserve,
}
/// Preserving an artifact here does not promise that its bytes match the new encryption mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeChangePolicy {
    CopyDatabase,
    Recreate,
    Rebuild,
    ReplaceMetadata,
    Preserve,
}
/// Coverage of the current header checker, not a claim of full-root cryptographic verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtRestPolicy {
    NativeDatabase,
    Unchecked,
    PlainKnownGap,
    AsCopied,
    Metadata,
    Runtime,
}
/// Parent ownership and explicit exclusions preserve the existing broad deletion behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WipePolicy {
    ArchiveChildrenExceptLocks,
    Remove(u8),
    CoveredByParent,
    RootBackups,
    CoveredByRootBackups,
    PreserveRuntime,
    PreserveDiagnostics,
    PreserveUnselectedControl,
    /// Removed once everything else is gone: the wipe's own record, then the control. The control
    /// stays until the very end so that a release which cannot read this archive never finds a
    /// folder without one while data is still in it.
    RemoveAfterCompletion,
}
/// Existing storage DTO categories; original data can live in a category called derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageBucket {
    Archive,
    SourceEvidence,
    Intelligence,
    Agent,
    Search,
    Manifests,
    Snapshots,
    Exports,
    Staging,
    Quarantine,
    Semantic,
    TextBlobs,
}
/// Measurements intentionally retain their different companion-file coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    MainAndWal,
    Tree,
    TreeOrFile,
    None,
}
/// One measured contribution; containers and descendants must not both contribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsagePolicy {
    pub bucket: Option<UsageBucket>,
    pub measure: Measure,
}
/// Only these buckets are offered by the existing retention command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetentionBucket {
    Snapshots,
    Exports,
    Staging,
    Quarantine,
}
impl RetentionBucket {
    /// Preserves the public request/response identifiers.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Snapshots => "snapshots",
            Self::Exports => "exports",
            Self::Staging => "staging",
            Self::Quarantine => "quarantine",
        }
    }
}
/// Doctor labels stay stable while the checked membership follows the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthPolicy {
    Database { name: &'static str, label: &'static str },
    NotChecked,
}
/// Export destination restrictions are separate from ownership and deletion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportTargetPolicy {
    Deny,
    Allow,
    DenyRootPrefix,
}
/// Older SQLite sources are distinct artifacts, never native-store aliases during opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversionPolicy {
    Settings { entry: &'static str },
    Source { db: Db, canonical: bool, primary: bool, required_in_bundle: bool },
    Rebuild,
    Omit,
}

/// What finding this artifact means in a data folder that has no `config.json`.
///
/// A folder without a control is a first run, a damaged archive, or an operation a crash cut
/// short. Only what lies in it can tell which, and guessing wrong either initializes next to an
/// archive whose control was lost or locks the user out of a fresh start. Every artifact answers
/// for itself, so that a file some operation leaves behind cannot be forgotten here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CensusPolicy {
    /// Part of an archive, or a copy of one. A control was lost.
    Archive,
    /// May lie in a folder that holds no archive: locks, logs, scheduler records, scratch
    /// space, things that are not PathKeep's.
    NotArchive,
    /// The answer of the artifact that contains it.
    Inherit,
    /// "Delete all data" was cut short after it removed the control.
    WipeRecord,
    /// An install was cut short while the control was moved aside.
    InstallRecord,
    /// The same, in a journal of PathKeep 0.3.
    OldFormatRecord,
}

macro_rules! lifecycle_operations {
    ($($variant:ident => $field:ident: $ty:ty),+ $(,)?) => {
        /// A closed list of operations whose policy answers cannot be omitted.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum LifecycleOperation { $($variant),+ }
        /// Every catalog row supplies all of these fields; there are no policy defaults.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct LifecyclePolicies { $(pub $field: $ty),+ }
    };
}
lifecycle_operations! {
    Open => open: OpenPolicy,
    Snapshot => snapshot: SnapshotPolicy,
    Restore => restore: RestorePolicy,
    Portable => portable: PortablePolicy,
    ModeChange => mode_change: ModeChangePolicy,
    AtRest => at_rest: AtRestPolicy,
    Wipe => wipe: WipePolicy,
    Usage => usage: UsagePolicy,
    Retention => retention: Option<RetentionBucket>,
    Health => health: HealthPolicy,
    ExportTarget => export_target: ExportTargetPolicy,
    Conversion => conversion: ConversionPolicy,
    InitializeEnvelope => prevents_new_envelope: bool,
    PasswordChange => password_change: PasswordChangePolicy,
    MoveImport => move_import: ImportPolicy,
    Census => census: CensusPolicy,
}
