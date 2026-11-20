//! Local, rebuildable semantic pages live inside the Agent search role. This gives them the
//! same encryption, disk accounting and lifecycle as the lexical index. The caller owns ranking.
use crate::{Archive, Error, Result, archive::lifetime::Job};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tantivy::directory::{Directory, MmapDirectory};
use vault_crypto::{EncryptedDirectory, SubKey};
use vault_store::CancelToken;

pub struct SemanticFiles {
    root: PathBuf,
    key: Option<Arc<SubKey>>,
    _lease: Job,
}
impl Archive {
    pub fn semantic_files(&self, cancel: &CancelToken) -> Result<SemanticFiles> {
        let lease = self.jobs.track(cancel)?;
        super::index::validate_parent(self)?;
        for dir in [&self.search_path, &self.search_path.join("semantic")] {
            match fs::symlink_metadata(dir) {
                Ok(m) if !m.is_dir() => {
                    return Err(Error::InvalidInput("expected owned semantic directory".into()));
                }
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
        Ok(SemanticFiles {
            root: self.search_path.join("semantic"),
            key: self.search_key.lock().unwrap().clone(),
            _lease: lease,
        })
    }
}
impl SemanticFiles {
    pub fn path(&self) -> &Path {
        &self.root
    }
    fn directory(&self, generation: Option<&str>) -> Result<Box<dyn Directory>> {
        let dir = match generation {
            None => self.root.clone(),
            Some(name)
                if name.starts_with("generation-")
                    && name.bytes().all(|b| b == b'-' || b.is_ascii_alphanumeric()) =>
            {
                self.root.join(name)
            }
            _ => return Err(Error::InvalidInput("invalid semantic generation".into())),
        };
        if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
            return Err(Error::InvalidInput("invalid semantic directory".into()));
        }
        if let Some(key) = &self.key {
            Ok(Box::new(EncryptedDirectory::open_with_subkey(&dir, key.clone(), 1024 * 1024)?))
        } else {
            Ok(Box::new(MmapDirectory::open(dir).map_err(std::io::Error::other)?))
        }
    }
    pub fn read(&self, generation: Option<&str>, name: &str) -> Result<Vec<u8>> {
        let data = self
            .directory(generation)?
            .atomic_read(Path::new(name))
            .map_err(std::io::Error::other)?;
        Ok(zstd::decode_all(data.as_slice())?)
    }
    pub fn write(&self, generation: Option<&str>, name: &str, data: &[u8]) -> Result<()> {
        self.directory(generation)?.atomic_write(Path::new(name), &zstd::encode_all(data, 1)?)?;
        Ok(())
    }
}
