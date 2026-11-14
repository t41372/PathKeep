//! Responsibilities: own an isolated history archive and its persistence boundary.
//! Not responsible for: source discovery, native parsing, UI policy or harness mutation.
//!
//! Raw evidence and normalized queries use separate stores. The archive's own data key
//! keeps it independent from PathKeep's browser vault and assistant runtime.
mod attachment_counts;
mod attachment_outcomes;
mod chunks;
mod comparison;
mod conversation;
mod durability;
mod generations;
#[cfg(test)]
mod initialization_tests;
mod lineage;
mod maintenance;
#[cfg(test)]
mod membership_tests;
mod observations;
mod parse_gaps;
mod projection;
#[cfg(test)]
mod read_concurrency_tests;
mod reader;
mod references;
mod reports;
mod retention;
mod revisions;
mod schema;
mod selection;
mod sources;
mod user_state;

pub use observations::FolderObservation;
pub use projection::ProjectionAttempt;
pub use reader::RawReader;
pub(crate) use reader::expired_read;
pub use reports::BackupRun;
pub use retention::SLICE_BYTES;
pub use revisions::{RevisionInfo, RevisionOutcome, SourceInfo};
pub use user_state::ConversationMark;

use crate::{Error, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use vault_crypto::{DataKey, Envelope, KdfParams, KeyPurpose, TempCryptIo};
use vault_store::{Config, Db, OwnerLock, Store};

const FORMAT_NAME: &str = "agent-history-format";
const FORMAT_BYTES: &[u8] = b"PathKeep Agent History\n1\n";
const PLAIN_FORMAT_BYTES: &[u8] = b"PathKeep Agent History\n1\nunencrypted\n";
const ENVELOPE_NAME: &str = "agent-history-key.envelope";

/// A single owner keeps evidence, projections and the selected encryption mode consistent.
/// Open and query it from blocking workers; the storage facade owns its database threads.
pub struct Archive {
    pub(crate) store: Arc<Store>,
    pub(crate) key: Arc<DataKey>,
    pub(crate) encrypted: bool,
    pub(crate) root: PathBuf,
    pub(crate) search_gate: Mutex<()>,
    pub(crate) state_gate: Mutex<()>,
    pub(crate) capture_gate: Mutex<()>,
    pub(crate) publication_gate: Mutex<()>,
}

impl Archive {
    /// Initializes an empty destination using the product's accepted password KDF.
    /// An existing archive or unrelated directory is never overwritten.
    pub fn create(root: &Path, password: &[u8]) -> Result<Self> {
        Self::create_with_params(root, password, KdfParams::RECOMMENDED)
    }

    /// Allows an embedding application to select explicit KDF parameters. Production
    /// callers should use `create`; integrity tests use a small KDF to test I/O cheaply.
    pub fn create_with_params(root: &Path, password: &[u8], params: KdfParams) -> Result<Self> {
        if password.is_empty() {
            return Err(Error::InvalidInput("an archive password is required".into()));
        }
        check_empty_destination(root)?;
        let (root, owner) = reserve_destination(root)?;
        // Derivation happens after reservation: another initializer cannot finish
        // while this one is spending time on the password KDF.
        let (envelope, key) = Envelope::create(password, params)?;
        install_new_envelope(&root, &envelope)?;
        let archive = Self::open_with_key(root, key, Some(owner), true)?;
        write_marker(&archive.root, FORMAT_BYTES)?;
        Ok(archive)
    }

    /// Creates an archive whose persistent evidence and derived files need no key.
    /// An ephemeral key still protects database spill files for this owner process.
    pub fn create_unencrypted(root: &Path) -> Result<Self> {
        check_empty_destination(root)?;
        let (root, owner) = reserve_destination(root)?;
        let archive = Self::open_with_key(root, temporary_key()?, Some(owner), false)?;
        write_marker(&archive.root, PLAIN_FORMAT_BYTES)?;
        Ok(archive)
    }

    /// Unlocks an existing recognized archive before opening any database. A wrong
    /// password cannot start migrations, create files or recover staging state.
    pub fn open(root: &Path, password: &[u8]) -> Result<Self> {
        reject_symlink(root)?;
        let marker = root.join(FORMAT_NAME);
        reject_symlink(&marker)?;
        let mut bytes = Vec::new();
        File::open(&marker)
            .map_err(|_| Error::InvalidArchive("missing format marker".into()))?
            .take(128)
            .read_to_end(&mut bytes)?;
        if bytes != FORMAT_BYTES && bytes != PLAIN_FORMAT_BYTES {
            return Err(Error::InvalidArchive("unsupported format marker".into()));
        }
        let encrypted = bytes == FORMAT_BYTES;
        if !encrypted && !password.is_empty() {
            return Err(Error::ArchiveNotEncrypted);
        }
        if encrypted && password.is_empty() {
            return Err(Error::PasswordRequired);
        }
        let envelope_path = root.join(ENVELOPE_NAME);
        reject_symlink(&envelope_path)?;
        let key = if encrypted {
            Envelope::parse(&vault_crypto::read_envelope_file(&envelope_path)?)?.unwrap(password)?
        } else {
            temporary_key()?
        };
        for relative in ["archive/catalog.db", "archive/evidence.db"] {
            let path = root.join(relative);
            reject_symlink(&path)?;
            if !path.is_file() {
                return Err(Error::InvalidArchive("a canonical database is missing".into()));
            }
        }
        Self::open_with_key(fs::canonicalize(root)?, key, None, encrypted)
    }

    fn open_with_key(
        root: PathBuf,
        key: DataKey,
        owner: Option<Arc<OwnerLock>>,
        encrypted: bool,
    ) -> Result<Self> {
        reject_symlink(&root.join("archive"))?;
        reject_symlink(&root.join("derived"))?;
        let mut config = Config::new(&root);
        config.databases = vec![Db::Archive, Db::SourceEvidence, Db::Scratch];
        config.paths.archive = root.join("archive/catalog.db");
        config.paths.source_evidence = root.join("archive/evidence.db");
        config.paths.scratch = root.join("derived/scratch.db");
        for (db, purpose) in [
            (Db::Archive, KeyPurpose::ArchiveDatabase),
            (Db::SourceEvidence, KeyPurpose::SourceEvidenceDatabase),
            (Db::Scratch, KeyPurpose::ScratchDatabase),
        ] {
            reject_symlink(config.paths.get(db))?;
            if encrypted {
                config.keys[db as usize] = Some(*key.derive(purpose).expose_bytes());
            }
        }
        config.io = Some(TempCryptIo::new(&key).map_err(std::io::Error::other)?);
        config.owner = owner;
        config.readers_per_database = NonZeroUsize::new(2).unwrap();
        config.migrations = schema::migrations();
        let archive = Self {
            store: Arc::new(Store::open(config)?),
            key: Arc::new(key),
            encrypted,
            root,
            search_gate: Mutex::new(()),
            state_gate: Mutex::new(()),
            capture_gate: Mutex::new(()),
            publication_gate: Mutex::new(()),
        };
        archive.recover_unfinished_revisions()?;
        archive.reclaim_slices()?;
        archive.recover_projection_work()?;
        archive.recover_attachment_counts()?;
        archive.recover_backup_runs()?;
        archive.synchronize_user_state()?;
        Ok(archive)
    }

    /// Identifies the explicitly selected archive without exposing keys or source data.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn check_empty_destination(root: &Path) -> Result<()> {
    reject_symlink(root)?;
    if root.exists() && (!root.is_dir() || fs::read_dir(root)?.next().is_some()) {
        return Err(Error::AlreadyExists("archive destination is not empty".into()));
    }
    Ok(())
}

fn reserve_destination(root: &Path) -> Result<(PathBuf, Arc<OwnerLock>)> {
    fs::create_dir_all(root)?;
    private_directory(root)?;
    let root = fs::canonicalize(root)?;
    reject_symlink(&root.join("archive"))?;
    let owner = Arc::new(OwnerLock::acquire(&root)?);
    // The preflight observation may be stale. Under the ownership lock the only
    // allowed entries are the lock's own scaffolding, never an existing envelope.
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_name() != "archive" || !entry.file_type()?.is_dir() {
            return Err(Error::AlreadyExists(
                "archive destination changed before reservation".into(),
            ));
        }
    }
    for entry in fs::read_dir(root.join("archive"))? {
        if entry?.file_name() != ".pk-owner.lock" {
            return Err(Error::AlreadyExists(
                "archive destination changed before reservation".into(),
            ));
        }
    }
    Ok((root, owner))
}

fn install_new_envelope(root: &Path, envelope: &Envelope) -> Result<()> {
    let path = root.join(ENVELOPE_NAME);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(&path)?;
    output.write_all(&envelope.to_bytes())?;
    output.sync_all()?;
    drop(output);
    durability::sync_file(&path)?;
    durability::sync_directory(root)?;
    Ok(())
}

fn write_marker(root: &Path, bytes: &[u8]) -> Result<()> {
    let path = root.join(FORMAT_NAME);
    let temporary = root.join(".agent-history-format.tmp");
    let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    private_file(&temporary)?;
    durability::sync_file(&temporary)?;
    fs::rename(&temporary, &path)?;
    durability::sync_directory(root)?;
    if let Some(parent) = root.parent() {
        durability::sync_directory(parent)?;
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::InvalidArchive("archive paths must not be symbolic links".into()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
fn private_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn private_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn private_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn private_file(_path: &Path) -> Result<()> {
    Ok(())
}

fn temporary_key() -> Result<DataKey> {
    // Envelope creation obtains a random data key without exposing or persisting it.
    Ok(Envelope::create(
        b"ephemeral spill owner",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )?
    .1)
}
