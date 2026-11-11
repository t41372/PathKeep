//! Responsibilities: open encrypted derived state and authenticate its commit watermark.
//! Not responsible for: canonical publication, SQL indexing walks or query semantics.
//! Corruption is rebuildable only through the caller's explicit synchronize operation.

use super::schema::{self, Fields};
use crate::{Archive, Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
    pub directory: EncryptedDirectory,
    pub fields: Fields,
    pub watermark: i64,
}

pub(super) fn path(archive: &Archive) -> PathBuf {
    archive.root.join("derived/history-search")
}

fn directory(archive: &Archive) -> Result<EncryptedDirectory> {
    validate_parent(archive)?;
    let path = path(archive);
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::IndexBehind
        } else {
            Error::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::InvalidInput("search directory must be an owned directory".into()));
    }
    Ok(EncryptedDirectory::open(&path, &archive.key, CACHE_BYTES)?)
}

pub(super) fn open(archive: &Archive) -> Result<Opened> {
    let directory = directory(archive)?;
    if !checked(Index::exists(&directory).map_err(Into::into), &directory)? {
        return Err(Error::IndexBehind);
    }
    let index = checked(Index::open(directory.clone()), &directory)?;
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
    let path = path(archive);
    if path.try_exists()? {
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::InvalidInput("search directory must be an owned directory".into()));
        }
        // Only derived state under this archive's fixed search directory is reset.
        // The source and canonical records are outside this operation's ownership.
        std::fs::remove_dir_all(&path)?;
    }
    std::fs::create_dir_all(&path)?;
    let directory = directory(archive)?;
    let (schema, fields) = schema::schema();
    let index =
        checked(Index::create(directory.clone(), schema, IndexSettings::default()), &directory)?;
    schema::register(&index);
    Ok(Opened { index, directory, fields, watermark: 0 })
}

fn validate_parent(archive: &Archive) -> Result<()> {
    match std::fs::symlink_metadata(archive.root.join("derived")) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(Error::InvalidInput("search parent must be an owned directory".into()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn checked<T>(result: tantivy::Result<T>, directory: &EncryptedDirectory) -> Result<T> {
    // Tantivy can swallow a positions-file read error. Its apparent success must
    // not override the authenticated directory's recorded failure.
    if directory.integrity_error().is_some() {
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
