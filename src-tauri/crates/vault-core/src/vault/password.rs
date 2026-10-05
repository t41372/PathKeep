//! Responsibilities: durably stage an authenticated envelope before any database is created,
//! then promote it after a successful Store open, including after an interrupted first open.
//! Not responsible for: keychain I/O, session state or database migrations.
//! Failure modes: a crash strands encrypted files, recovery replaces their key, an
//! incorrect password mutates files, or an envelope is published before its Store opens.
//! Tests: vault::tests recovery cases.
use super::{ENVELOPE_ACCESS, EnvelopeMissing, Vault};
use crate::{AppConfig, ProjectPaths};
use std::{path::Path, sync::Arc};
use vault_crypto::{
    DataKey, Envelope, EnvelopeError, KdfParams, read_envelope_file, write_envelope_file,
};

#[derive(Debug)]
pub struct ArchivePasswordWrong;
impl std::fmt::Display for ArchivePasswordWrong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("archive-password-wrong: the archive password did not match")
    }
}
impl std::error::Error for ArchivePasswordWrong {}
fn unwrap(bytes: &[u8], password: &str) -> anyhow::Result<DataKey> {
    Envelope::parse(bytes)?.unwrap(password.as_bytes()).map_err(|error| match error {
        EnvelopeError::WrongPassword => ArchivePasswordWrong.into(),
        other => other.into(),
    })
}
pub(super) fn unlock(paths: &ProjectPaths, password: &str) -> anyhow::Result<DataKey> {
    let bytes =
        read_envelope_file(&paths.app_root.join("archive/keys.envelope")).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                anyhow::Error::new(EnvelopeMissing)
            } else {
                error.into()
            }
        })?;
    unwrap(&bytes, password)
}
pub(super) fn open(
    paths: ProjectPaths,
    config: AppConfig,
    password: &str,
    owner: Option<Arc<vault_store::OwnerLock>>,
) -> anyhow::Result<(Vault, DataKey)> {
    open_using(paths, config, password, owner, |paths, config, key, owner| {
        Ok(Vault::open_owned(paths, config, Some(key), owner)?)
    })
}
pub(super) fn open_using(
    paths: ProjectPaths,
    config: AppConfig,
    password: &str,
    owner: Option<Arc<vault_store::OwnerLock>>,
    open: impl FnOnce(
        ProjectPaths,
        AppConfig,
        &DataKey,
        Option<Arc<vault_store::OwnerLock>>,
    ) -> anyhow::Result<Vault>,
) -> anyhow::Result<(Vault, DataKey)> {
    let _guard = ENVELOPE_ACCESS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let final_path = paths.app_root.join("archive/keys.envelope");
    let pending = final_path.with_file_name("keys.envelope.pending");
    let (key, promote) = match read_envelope_file(&final_path) {
        Ok(bytes) => (unwrap(&bytes, password)?, false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let key = match read_envelope_file(&pending) {
                Ok(bytes) => unwrap(&bytes, password)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let files = vault_store::Paths::under(&paths.app_root);
                    if vault_store::Db::ALL.iter().any(|db| files.get(*db).exists()) {
                        return Err(EnvelopeMissing.into());
                    }
                    if paths.archive_database_path.exists()
                        && !crate::archive_key_opens(&paths, password)?
                    {
                        return Err(ArchivePasswordWrong.into());
                    }
                    let (envelope, key) =
                        Envelope::create(password.as_bytes(), KdfParams::RECOMMENDED)?;
                    std::fs::create_dir_all(pending.parent().unwrap())?;
                    // vault-crypto fsyncs both the file and its containing directory.
                    write_envelope_file(&pending, &envelope)?;
                    key
                }
                Err(error) => return Err(error.into()),
            };
            (key, true)
        }
        Err(error) => return Err(error.into()),
    };
    let vault = open(paths, config, &key, owner)?;
    if promote {
        std::fs::rename(&pending, &final_path)?;
        sync_parent(&final_path)?;
    }
    Ok((vault, key))
}
#[cfg(unix)]
fn sync_parent(path: &Path) -> std::io::Result<()> {
    std::fs::File::open(path.parent().unwrap())?.sync_all()
}
#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> std::io::Result<()> {
    Ok(())
}
