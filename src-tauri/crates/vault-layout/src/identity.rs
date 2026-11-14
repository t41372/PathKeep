//! Responsibilities: stable resource identities and immutable cryptographic purpose labels.
//! Not responsible for: policies, key derivation or filesystem access.

macro_rules! identities {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $value:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(usize)]
        pub enum $name { $($variant = $value),+ }
        impl $name {
            /// Enumeration is emitted with the identities so no declared member can be omitted.
            pub const ALL: [Self; Self::COUNT] = [$(Self::$variant),+];
            /// Keeps storage slots synchronized with the declared identities.
            pub const COUNT: usize = [$(stringify!($variant)),+].len();
        }
    };
}
identities! {
    /// Stable database identities shared by storage consumers and lifecycle policies.
    Db { Archive = 0, SourceEvidence = 1, Intelligence = 2, Agent = 3, Scratch = 4 }
}
const _: () = {
    let mut index = 0;
    while index < Db::COUNT {
        assert!(Db::ALL[index] as usize == index);
        index += 1;
    }
};
identities! {
    /// Every archive artifact has a distinct identity and an exhaustive policy arm.
    ArtifactId {
        History = 0,
        SourceEvidence = 1,
        Intelligence = 2,
        Assistant = 3,
        Scratch = 4,
        Envelope = 5,
        PendingEnvelope = 6,
        Config = 7,
        Search = 8,
        Root = 9,
        ArchiveRoot = 10,
        DerivedRoot = 11,
        Audit = 12,
        Manifests = 13,
        Exports = 14,
        RawSnapshots = 15,
        Staging = 16,
        Quarantine = 17,
        Schedule = 18,
        Sidecars = 19,
        LegacySemantic = 20,
        TextBlobs = 21,
        Vectors = 22,
        Models = 23,
        Logs = 24,
        CrashReports = 25,
        Integrations = 26,
        Diagnostics = 27,
        Ipc = 28,
        OwnerEndpoint = 29,
        OwnerLock = 30,
        WriteLock = 31,
        InstallJournal = 32,
        StaleImportJournal = 33,
        LegacyRekeyJournal = 34,
        LegacyRestoreJournal = 35,
        EnvelopeTemp = 36,
        WipeIntent = 37,
        AppLockState = 38,
        AppLockPasscode = 39,
        Stronghold = 40,
        StrongholdSalt = 41,
        RustLog = 42,
        FrontendLog = 43,
        OwnerStartupLog = 44,
        RustPanic = 45,
        FrontendError = 46,
        LegacyHistory = 47,
        LegacyEvidence = 48,
        LegacyIntelligence = 49,
        LegacyAssistant = 50,
        LegacySearch = 51,
        NativeSnapshots = 52,
        LegacySnapshots = 53,
        MaintenanceStages = 54,
        PreviousArchives = 55,
        RootBackups = 56,
        AtomicTemps = 57,
        ReservedControls = 58,
        SqlcipherStage = 59,
        ReservedGit = 60,
        ImportRoots = 61,
        PortableArtifacts = 62,
        LegacyRekeyTemps = 63,
        LegacyRestoreTemps = 64,
        OtherLegacySnapshots = 65,
        ArchiveBackups = 66,
        McpIntegration = 67,
        SkillIntegration = 68,
        BrowserSnippetHost = 69,
        ScheduleLogs = 70,
        ScheduleAttempts = 71,
        SchedulerAudit = 72,


    }
}
macro_rules! key_purposes {
    ($($(#[$meta:meta])* $id:ident => $label:literal),+ $(,)?) => {
        /// What a sub-key is for. Each purpose gets key material no other purpose sees.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum KeyPurpose { $($(#[$meta])* $id),+ }
        impl KeyPurpose {
            /// Declares every purpose alongside its immutable label.
            pub const ALL: [Self; [$(stringify!($id)),+].len()] = [$(Self::$id),+];
            /// The HKDF info label. Changing one makes existing data unreadable.
            pub const fn label(self) -> &'static [u8] { match self { $(Self::$id => $label),+ } }
        }
    };
}
key_purposes! {
    /// `archive/history-vault.db`, opened with Turso's native encryption.
    ArchiveDatabase => b"pathkeep/v1/archive-database",
    /// `archive/source-evidence.db`.
    SourceEvidenceDatabase => b"pathkeep/v1/source-evidence-database",
    /// `derived/intelligence.db`.
    IntelligenceDatabase => b"pathkeep/v1/intelligence-database",
    /// `derived/agent.db`, the assistant's own conversations.
    AgentDatabase => b"pathkeep/v1/agent-database",
    /// `derived/scratch.db`, the ATTACHed database used instead of TEMP tables.
    ScratchDatabase => b"pathkeep/v1/scratch-database",
    /// The Tantivy search index, through `vault_crypto::EncryptedDirectory`.
    SearchIndex => b"pathkeep/v1/search-index",
    /// Turso's sort and hash spill files, through `vault_crypto::TempCryptIo`.
    TempFiles => b"pathkeep/v1/temp-files",
    /// Immutable audit and source-checkpoint files inside a portable bundle.
    BundleArtifacts => b"pathkeep/v1/bundle-artifacts",
}
