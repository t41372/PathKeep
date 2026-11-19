//! Walk reviewed directories by handle. The displayed path identifies an
//! artifact; it does not authorize reopening a replacement tree by that name.
use super::directory_handle;
use cap_primitives::fs as cap;
use same_file::Handle;
use std::{
    ffi::OsString,
    fs, io,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use vault_store::CancelToken;

/// Desktop discovery pins the reviewed object. Standalone commands can make
/// a fresh selection at invocation and retain their file-root semantics.
#[derive(Default)]
pub struct Discovery {
    cancel: CancelToken,
    root: Option<(PathBuf, Arc<Handle>)>,
}
impl Discovery {
    pub(crate) fn new(cancel: &CancelToken, root: Option<(PathBuf, Arc<Handle>)>) -> Self {
        Self { cancel: cancel.clone(), root }
    }
    pub(crate) fn check(&self) -> crate::Result<()> {
        self.cancel.check().map_err(|_| crate::Error::Cancelled)
    }
    pub(crate) fn still_selected(&self) -> bool {
        self.root.as_ref().is_none_or(|(path, pinned)| {
            directory_handle(path).is_ok_and(|now| &now == pinned.as_ref())
        })
    }
    pub(crate) fn metadata(&self, path: &Path) -> io::Result<cap::Metadata> {
        if let Some((root, pinned)) = &self.root {
            let rel = path.strip_prefix(root).map_err(io::Error::other)?;
            if rel.as_os_str().is_empty() {
                return cap::Metadata::from_file(pinned.as_file());
            }
            cap::stat(pinned.as_file(), rel, cap::FollowSymlinks::No)
        } else {
            fs::symlink_metadata(path).map(cap::Metadata::from_just_metadata)
        }
    }
    pub(crate) fn read_dir(&self, path: &Path) -> io::Result<Entries> {
        let opened = if let Some((root, pinned)) = &self.root {
            let mut dir = pinned.as_file().try_clone()?;
            for component in path.strip_prefix(root).map_err(io::Error::other)?.components() {
                let Component::Normal(name) = component else {
                    return Err(io::ErrorKind::PermissionDenied.into());
                };
                if name.eq_ignore_ascii_case(".git") {
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
                dir = cap::open_dir_nofollow(&dir, Path::new(name))?;
            }
            dir
        } else {
            directory_handle(path)?.as_file().try_clone()?
        };
        Ok(Entries { parent: path.to_owned(), reader: cap::read_base_dir(&opened)? })
    }
}

/// The path label is used by native format classifiers. File type and metadata
/// come from the handle-relative entry, independent of subsequent renames.
pub(crate) struct Entry {
    path: PathBuf,
    inner: cap::DirEntry,
}
impl Entry {
    pub(crate) fn path(&self) -> PathBuf {
        self.path.clone()
    }
    pub(crate) fn file_name(&self) -> OsString {
        self.inner.file_name()
    }
    pub(crate) fn file_type(&self) -> io::Result<cap::FileType> {
        self.inner.file_type()
    }
    pub(crate) fn metadata(&self) -> io::Result<cap::Metadata> {
        self.inner.metadata()
    }
}
pub(crate) struct Entries {
    parent: PathBuf,
    reader: cap::ReadDir,
}
impl Iterator for Entries {
    type Item = io::Result<Entry>;
    fn next(&mut self) -> Option<Self::Item> {
        self.reader.next().map(|entry| {
            entry.map(|inner| Entry { path: self.parent.join(inner.file_name()), inner })
        })
    }
}
