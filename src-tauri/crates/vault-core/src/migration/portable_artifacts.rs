//! Responsibilities: stream immutable provenance through an encrypted portable artifact file.
//! Not responsible for: database copying, derived search data or provider/model caches.
//! Failure modes: plaintext temporary files, missing provenance, unsafe archive paths,
//! uncancellable copies, archive-sized file lists or source-machine artifact locators.
//! Tests: migration::native_tests and vault-crypto portable streams.
use crate::{ProjectPaths, Vault};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use vault_crypto::DataKey;
use vault_store::{CancelToken, Db, Priority};

pub(super) const NAME: &str = "archive/portable-artifacts.pkf";
const ROOTS: [&str; 3] = ["audit", "raw-snapshots", "sidecars"];
const ORIGIN: &str = "pathkeep-origin.json";
const ALIASES: &str = "audit/pathkeep-import-roots.json";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Origin {
    pub source_root: String,
}

/// A streaming TAR keeps the outer ZIP manifest bounded to six entries even when
/// the audit history contains many files. The existing authenticated codec seals it.
pub(super) fn pack(
    paths: &ProjectPaths,
    sealed: &Path,
    key: &DataKey,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    fs::create_dir_all(sealed.join("archive"))?;
    vault_crypto::write_artifact(sealed, NAME, key, |output| {
        let result = (|| -> Result<()> {
            let mut tar = tar::Builder::new(CheckedWriter { output, cancel });
            tar.sparse(false);
            tar.follow_symlinks(false);
            let origin =
                serde_json::to_vec(&Origin { source_root: paths.app_root.display().to_string() })?;
            let mut header = tar::Header::new_gnu();
            header.set_size(origin.len() as u64);
            header.set_mode(0o600);
            header.set_cksum();
            tar.append_data(&mut header, ORIGIN, origin.as_slice())?;
            for root in ROOTS {
                let source = paths.app_root.join(root);
                if !source.exists() {
                    continue;
                }
                let mut directories = Vec::new();
                // Everything under these roots moves, saved page text included: it is
                // the user's data, and a page fetched again may have changed or gone.
                for entry in walkdir::WalkDir::new(&source) {
                    cancel.check()?;
                    let entry = entry?;
                    let name = entry
                        .path()
                        .strip_prefix(&paths.app_root)?
                        .to_string_lossy()
                        .replace('\\', "/");
                    if entry.file_type().is_dir() {
                        directories.push((entry.into_path(), name));
                    } else if entry.file_type().is_file() {
                        tar.append_path_with_name(entry.path(), name)?;
                    }
                }
                // Directory timestamps are installed after all children, preserving
                // the keyless recovery inventory's ordering on the destination.
                for (path, name) in directories.into_iter().rev() {
                    tar.append_dir(name, path)?;
                }
            }
            tar.finish()?;
            Ok(())
        })();
        result.map_err(|error| io::Error::other(error.into_boxed_dyn_error()))
    })?;
    Ok(sealed.join(NAME))
}

pub(super) fn unpack(stage: &Path, key: &DataKey, cancel: &CancelToken) -> Result<Origin> {
    Ok(vault_crypto::read_artifact(stage, NAME, key, |input| {
        let result = (|| -> Result<Origin> {
            let mut tar = tar::Archive::new(CheckedReader { input, cancel });
            let mut origin = None;
            for entry in tar.entries()? {
                cancel.check()?;
                let mut entry = entry?;
                let name = entry.path()?.to_string_lossy().replace('\\', "/");
                if name == ORIGIN {
                    origin = Some(serde_json::from_reader(&mut entry)?);
                    continue;
                }
                let relative = super::validate_bundle_relative_path(&name)?;
                anyhow::ensure!(
                    ROOTS.iter().any(|root| name == *root || name.starts_with(&format!("{root}/"))),
                    "unsupported artifact root: {name}"
                );
                let path = stage.join(relative);
                let kind = entry.header().entry_type();
                let modified =
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(entry.header().mtime()?);
                if kind.is_dir() {
                    fs::create_dir_all(&path)?;
                    set_directory_time(&path, modified)?;
                } else if kind.is_file() {
                    fs::create_dir_all(path.parent().unwrap())?;
                    let mut file = File::create(&path)?;
                    io::copy(&mut entry, &mut file)?;
                    file.set_times(fs::FileTimes::new().set_modified(modified))?;
                    file.sync_all()?;
                } else {
                    anyhow::bail!("unsupported artifact entry type: {name}");
                }
            }
            origin.ok_or_else(|| anyhow::anyhow!("bundle provenance metadata is missing"))
        })();
        result.map_err(|error| io::Error::other(error.into_boxed_dyn_error()))
    })?)
}

pub(super) fn roots() -> Vec<PathBuf> {
    ROOTS.into_iter().map(PathBuf::from).collect()
}

pub(super) fn remember_origin(stage: &Path, source: &str) -> Result<()> {
    let path = stage.join(ALIASES);
    let mut roots: Vec<String> =
        fs::read(&path).ok().and_then(|v| serde_json::from_slice(&v).ok()).unwrap_or_default();
    if !roots.iter().any(|root| root == source) {
        roots.push(source.into());
    }
    fs::create_dir_all(path.parent().unwrap())?;
    crate::durable_io::atomic_durable_write(&path, &serde_json::to_vec(&roots)?)
}

/// Immutable audit JSON retains its original text and hash. Locked review resolves
/// its old artifact paths through the imported provenance without rewriting it.
pub fn resolve_imported_artifact(paths: &ProjectPaths, value: &str) -> String {
    let roots: Vec<String> = fs::read(paths.app_root.join(ALIASES))
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
        .unwrap_or_default();
    roots
        .into_iter()
        .find_map(|root| relocated(value, &root, &paths.app_root))
        .unwrap_or_else(|| value.into())
}

struct CheckedWriter<'a> {
    output: &'a mut dyn Write,
    cancel: &'a CancelToken,
}
impl Write for CheckedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.cancel.check().map_err(io::Error::other)?;
        self.output.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
struct CheckedReader<'a> {
    input: &'a mut dyn Read,
    cancel: &'a CancelToken,
}
impl Read for CheckedReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.cancel.check().map_err(io::Error::other)?;
        self.input.read(bytes)
    }
}

/// Only artifact locators change. History URLs, browser paths and immutable manifest
/// contents retain their recorded values, and every update is in a bounded batch.
pub(super) fn relocate(vault: &Vault, from: &str, to: &Path, cancel: &CancelToken) -> Result<()> {
    for (db, table, column, json) in [
        (Db::Archive, "manifests", "file_path", false),
        (Db::Archive, "snapshots", "file_path", false),
        (Db::Archive, "import_batches", "audit_path", false),
        (Db::Archive, "runs", "stats_json", true),
        (Db::SourceEvidence, "source_batches", "artifact_refs_json", true),
    ] {
        let mut cursor = 0;
        loop {
            cancel.check()?;
            let rows = vault.store().read(db, move |s| {
                s.query(
                    &format!("SELECT rowid,{column} FROM {table} WHERE rowid>?1 AND {column} IS NOT NULL ORDER BY rowid LIMIT 1024"),
                    [cursor],
                    |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
                )
            })?;
            if rows.is_empty() {
                break;
            }
            cursor = rows.last().unwrap().0;
            let changed = rows
                .into_iter()
                .filter_map(|(id, text)| {
                    let next = if json {
                        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else {
                            return None;
                        };
                        if !relocate_json(&mut value, from, to) {
                            return None;
                        }
                        value.to_string()
                    } else {
                        relocated(&text, from, to)?
                    };
                    Some(vec![vault_store::Value::Integer(id), vault_store::Value::Text(next)])
                })
                .collect::<Vec<_>>();
            vault.store().write(db, Priority::Batch, move |s| {
                s.execute_rows(&format!("UPDATE {table} SET {column}=?2 WHERE rowid=?1"), changed)?;
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn relocated(value: &str, from: &str, to: &Path) -> Option<String> {
    let normalized = value.replace('\\', "/");
    let from = from.replace('\\', "/");
    let relative = normalized.strip_prefix(&format!("{}/", from.trim_end_matches('/')))?;
    Some(to.join(relative).display().to_string())
}
fn relocate_json(value: &mut serde_json::Value, from: &str, to: &Path) -> bool {
    let mut changed = false;
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                changed |= relocate_json(value, from, to);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if [
                    "path",
                    "snapshotPath",
                    "manifestPath",
                    "auditPath",
                    "historyPath",
                    "faviconsPath",
                ]
                .contains(&key.as_str())
                {
                    if let Some(next) = value.as_str().and_then(|v| relocated(v, from, to)) {
                        *value = serde_json::Value::String(next);
                        changed = true;
                    }
                } else {
                    changed |= relocate_json(value, from, to);
                }
            }
        }
        _ => {}
    }
    changed
}

#[cfg(not(windows))]
fn set_directory_time(path: &Path, time: std::time::SystemTime) -> io::Result<()> {
    File::open(path)?.set_times(fs::FileTimes::new().set_modified(time))
}
#[cfg(windows)]
fn set_directory_time(path: &Path, time: std::time::SystemTime) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    // A directory-capable handle with FILE_WRITE_ATTRIBUTES preserves its timestamp.
    fs::OpenOptions::new()
        .access_mode(0x100)
        .custom_flags(0x0200_0000)
        .open(path)?
        .set_times(fs::FileTimes::new().set_modified(time))
}
