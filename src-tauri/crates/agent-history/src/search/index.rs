//! Responsibilities: open encrypted derived state and authenticate its commit watermark.
//! Not responsible for: canonical publication, SQL indexing walks or query semantics.
//! Corruption is rebuildable only through the caller's explicit synchronize operation.

use super::schema::{self, Fields};
use crate::{Archive, Error, Result};
use serde::{Deserialize, Serialize};
use tantivy::{Index, IndexSettings};
use vault_crypto::EncryptedDirectory;

const CACHE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub(super) struct Commit {
    pub version: u32,
    pub watermark: i64,
}

pub(super) struct Opened {
    pub index: Index,
    pub directory: Option<EncryptedDirectory>,
    pub fields: Fields,
    pub watermark: i64,
}

fn directory(
    archive: &Archive,
) -> Result<(Box<dyn tantivy::Directory>, Option<EncryptedDirectory>)> {
    validate_parent(archive)?;
    let path = &archive.search_path;
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::IndexBehind
        } else {
            Error::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::InvalidInput("search directory must be an owned directory".into()));
    }
    let key = archive.search_key.lock().unwrap().clone();
    if let Some(key) = key {
        let directory = EncryptedDirectory::open_with_subkey(path, key, CACHE_BYTES)?;
        Ok((Box::new(directory.clone()), Some(directory)))
    } else {
        Ok((
            Box::new(tantivy::directory::MmapDirectory::open(path).map_err(std::io::Error::other)?),
            None,
        ))
    }
}

pub(super) fn open(archive: &Archive) -> Result<Opened> {
    let (storage, directory) = directory(archive)?;
    if !checked(Index::exists(&*storage).map_err(Into::into), &directory)? {
        return Err(Error::IndexBehind);
    }
    let index = checked(Index::open(storage), &directory)?;
    let (schema, fields) = schema::schema();
    if index.schema() != schema {
        return Err(Error::IndexBehind);
    }
    schema::register(&index);
    let metadata = checked(index.load_metas(), &directory)?;
    let commit: Commit = serde_json::from_str(&metadata.payload.unwrap_or_default())
        .map_err(|_| Error::IndexBehind)?;
    if commit.version != schema::VERSION || commit.watermark < 0 {
        return Err(Error::IndexBehind);
    }
    Ok(Opened { index, directory, fields, watermark: commit.watermark })
}

pub(super) fn recreate(archive: &Archive) -> Result<Opened> {
    validate_parent(archive)?;
    let path = &archive.search_path;
    if path.try_exists()? {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::InvalidInput("search directory must be an owned directory".into()));
        }
        // Both search modes own generations here. Rebuilding the lexical index keeps
        // the independently consented semantic publication available.
        for item in std::fs::read_dir(path)? {
            let item = item?;
            if item.file_name() == "semantic" {
                continue;
            }
            if item.file_type()?.is_dir() {
                vault_fs::remove_dir_all(item.path())?;
            } else {
                std::fs::remove_file(item.path())?;
            }
        }
    }
    std::fs::create_dir_all(path)?;
    #[cfg(test)]
    if std::env::var_os("PATHKEEP_INITIAL_INDEX_CRASH_FIXTURE").is_some() {
        std::process::exit(76);
    }
    let (storage, directory) = directory(archive)?;
    let (schema, fields) = schema::schema();
    let index = checked(Index::create(storage, schema, IndexSettings::default()), &directory)?;
    schema::register(&index);
    Ok(Opened { index, directory, fields, watermark: 0 })
}

pub(super) fn validate_parent(archive: &Archive) -> Result<()> {
    match std::fs::symlink_metadata(
        archive
            .search_path
            .parent()
            .ok_or_else(|| Error::InvalidInput("search directory requires a parent".into()))?,
    ) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(Error::InvalidInput("search parent must be an owned directory".into()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn checked<T>(
    result: tantivy::Result<T>,
    directory: &Option<EncryptedDirectory>,
) -> Result<T> {
    // Tantivy can swallow a positions-file read error. Its apparent success must
    // not override the authenticated directory's recorded failure.
    if directory.as_ref().is_some_and(|directory| directory.integrity_error().is_some()) {
        return Err(Error::IndexBehind);
    }
    result.map_err(|error| match &error {
        tantivy::TantivyError::DataCorruption(_) | tantivy::TantivyError::SchemaError(_) => {
            Error::IndexBehind
        }
        tantivy::TantivyError::IoError(cause)
            if matches!(
                cause.kind(),
                std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::InvalidData
                    | std::io::ErrorKind::UnexpectedEof
            ) =>
        {
            Error::IndexBehind
        }
        tantivy::TantivyError::OpenReadError(
            tantivy::directory::error::OpenReadError::FileDoesNotExist(_),
        ) => Error::IndexBehind,
        _ if vault_crypto::IndexFileError::find_in(&error).is_some() => Error::IndexBehind,
        _ => Error::Io(std::io::Error::other(error)),
    })
}

/// Only a recognizable previous search schema schedules a format upgrade at visibility.
pub(super) fn previous_schema(archive: &Archive) -> Result<bool> {
    let (storage, directory) = directory(archive)?;
    let index = checked(Index::open(storage), &directory)?;
    let meta = checked(index.load_metas(), &directory)?;
    Ok(meta
        .payload
        .and_then(|payload| serde_json::from_str::<Commit>(&payload).ok())
        .is_some_and(|commit| commit.version == 2 && commit.watermark >= 0)
        && index.schema().get_field("text").is_ok()
        && index.schema().get_field("card_id").is_err())
}
