//! Responsibilities: one compiled build classification and its machine-resource identities.
//! Not responsible for: archive formats, filesystem I/O, native integration calls or packaging.
use std::{ffi::OsString, fmt, path::PathBuf};

/// Only release packaging sets this at compile time; runtime environment cannot change CURRENT.
pub const DESIGNATION_ENV: &str = "PATHKEEP_DISTRIBUTION";
/// Account names remain stable inside either service namespace.
pub const ARCHIVE_KEY_ACCOUNT: &str = "database-key";
/// Provider identities are appended without changing the released account prefix.
pub const PROVIDER_KEY_ACCOUNT_PREFIX: &str = "ai-provider::";
/// An explicit archive location is independent of the build's machine-resource namespace.
pub const ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";

/// An optimized local binary is still development unless packaging explicitly designated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildClass {
    Development,
    Distribution,
}
/// Stable defaults shared by the desktop and its worker; production values are release contracts.
#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub qualifier: &'static str,
    pub organization: &'static str,
    pub application: &'static str,
    pub bundle_identifier: &'static str,
    pub keyring_service: &'static str,
    pub schedule_label: &'static str,
    pub login_item_label: &'static str,
    pub autostart_name: &'static str,
    pub windows_upgrade_code: &'static str,
    pub legacy_schedule_labels: &'static [&'static str],
}
const PRODUCTION: Identity = Identity {
    qualifier: "com",
    organization: "yi-ting",
    application: "pathkeep",
    bundle_identifier: "com.yi-ting.pathkeep",
    keyring_service: "com.yi-ting.pathkeep",
    schedule_label: "com.yi-ting.pathkeep.backup",
    login_item_label: "com.yi-ting.pathkeep.login",
    autostart_name: "PathKeep",
    windows_upgrade_code: "818daeb2-ee49-5696-a1db-bee51050939c",
    legacy_schedule_labels: &[
        "dev.codex.pathkeep.backup",
        "dev.codex.browser-history-backup.backup",
    ],
};
const DEVELOPMENT: Identity = Identity {
    qualifier: "com",
    organization: "yi-ting",
    application: "pathkeep-dev",
    bundle_identifier: "com.yi-ting.pathkeep.dev",
    keyring_service: "com.yi-ting.pathkeep.dev",
    schedule_label: "com.yi-ting.pathkeep.dev.backup",
    login_item_label: "com.yi-ting.pathkeep.dev.login",
    autostart_name: "PathKeep Dev",
    windows_upgrade_code: "37e94b55-0506-58ec-9137-e48b4c732ae8",
    legacy_schedule_labels: &[],
};
/// Makes the debug-build refusal explicit and independently testable without a release build.
pub const fn classify(designated: bool, debug_assertions: bool) -> BuildClass {
    if designated && !debug_assertions { BuildClass::Distribution } else { BuildClass::Development }
}
const DESIGNATED: bool = match option_env!("PATHKEEP_DISTRIBUTION") {
    Some(value) => value.len() == 1 && value.as_bytes()[0] == b'1',
    None => false,
};
/// The only runtime classification; Cargo tracks the option_env input in dependency fingerprints.
pub const CURRENT: BuildClass = classify(DESIGNATED, cfg!(debug_assertions));
impl BuildClass {
    /// Namespace changes never depend on archive settings or an inherited runtime designation.
    pub const fn identity(self) -> &'static Identity {
        match self {
            Self::Development => &DEVELOPMENT,
            Self::Distribution => &PRODUCTION,
        }
    }
    /// The development process must not offer production update installation.
    pub const fn is_distribution(self) -> bool {
        matches!(self, Self::Distribution)
    }
    /// A stable machine-readable value for the artifact check.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Distribution => "distribution",
        }
    }
}

/// Distinguishes an invalid explicit choice from failure to discover a platform default.
#[derive(Debug, PartialEq, Eq)]
pub enum RootError {
    EmptyOverride,
    DefaultUnavailable,
}
impl fmt::Display for RootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyOverride => "empty-project-root: CHB_PROJECT_ROOT must not be empty",
            Self::DefaultUnavailable => "resolving current project directories",
        })
    }
}
impl std::error::Error for RootError {}
/// Resolve bytes without opening, creating, migrating or copying either directory.
pub fn resolve_root(
    class: BuildClass,
    explicit: Option<OsString>,
    default: impl FnOnce(&Identity) -> Option<PathBuf>,
) -> Result<PathBuf, RootError> {
    if let Some(value) = explicit {
        return if value.is_empty() {
            Err(RootError::EmptyOverride)
        } else {
            Ok(PathBuf::from(value))
        };
    }
    default(class.identity()).ok_or(RootError::DefaultUnavailable)
}

/// Packaging must ask both executables, not accidentally ask the same binary twice.
#[derive(Clone, Copy, Debug)]
pub enum BinaryKind {
    Desktop,
    Worker,
}
/// Returns a report containing compiled defaults only: no root resolution, environment or I/O.
pub fn identity_report(binary: BinaryKind) -> String {
    let id = CURRENT.identity();
    let kind = match binary {
        BinaryKind::Desktop => "desktop",
        BinaryKind::Worker => "worker",
    };
    format!(
        r#"{{"schemaVersion":1,"binary":"{kind}","classification":"{}","debugAssertions":{},"defaultRootQualifier":"{}","defaultRootOrganization":"{}","defaultRootApplication":"{}","bundleIdentifier":"{}","keyringService":"{}","scheduleLabel":"{}","loginItemLabel":"{}","autostartName":"{}","windowsUpgradeCode":"{}"}}"#,
        CURRENT.name(),
        cfg!(debug_assertions),
        id.qualifier,
        id.organization,
        id.application,
        id.bundle_identifier,
        id.keyring_service,
        id.schedule_label,
        id.login_item_label,
        id.autostart_name,
        id.windows_upgrade_code
    )
}
/// Binary main calls this before any GUI, worker, logger or archive startup.
pub fn print_if_requested(binary: BinaryKind) -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().is_none_or(|arg| arg != "--build-identity") {
        return false;
    }
    if args.next().is_some() {
        eprintln!("--build-identity takes no other arguments");
        std::process::exit(2);
    }
    println!("{}", identity_report(binary));
    true
}
