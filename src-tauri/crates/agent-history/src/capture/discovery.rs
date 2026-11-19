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
pub struct Discovery<'a> {
    pub(super) progress: Option<&'a dyn Fn() -> crate::Result<()>>,
    folders: std::cell::Cell<u64>,
    unknown: std::cell::Cell<u64>,
    cancel: CancelToken,
    root: Option<(PathBuf, Arc<Handle>)>,
}
impl Discovery<'_> {
    pub(crate) fn new(cancel: &CancelToken, root: Option<(PathBuf, Arc<Handle>)>) -> Self {
        Self { cancel: cancel.clone(), root, ..Default::default() }
    }
    pub(crate) fn visited_entry(&self) -> crate::Result<()> {
        if let Some(progress) = self.progress {
            progress()?;
        }
        self.check()
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
            let dir = Self::descendant(pinned, rel.parent().unwrap_or(Path::new("")))?;
            cap::stat(
                &dir,
                Path::new(rel.file_name().ok_or(io::ErrorKind::InvalidInput)?),
                cap::FollowSymlinks::No,
            )
        } else {
            fs::symlink_metadata(path).map(cap::Metadata::from_just_metadata)
        }
    }
    fn descendant(pinned: &Handle, relative: &Path) -> io::Result<fs::File> {
        let mut dir = pinned.as_file().try_clone()?;
        for part in relative.components() {
            let Component::Normal(name) = part else {
                return Err(io::ErrorKind::PermissionDenied.into());
            };
            dir = cap::open_dir_nofollow(&dir, Path::new(name))?;
        }
        Ok(dir)
    }
    pub(crate) fn unknown_file(&self) {
        self.unknown.set(self.unknown.get() + 1)
    }
    pub(super) fn inventory_counts(&self) -> (u64, u64) {
        (self.folders.get(), self.unknown.get())
    }
    pub(crate) fn read_dir(&self, path: &Path) -> io::Result<Entries> {
        let entries = self.reread_dir(path)?;
        if self.root.as_ref().is_some_and(|(root, _)| path != root) {
            self.folders.set(self.folders.get() + 1);
        }
        Ok(entries)
    }
    pub(crate) fn reread_dir(&self, path: &Path) -> io::Result<Entries> {
        let opened = if let Some((root, pinned)) = &self.root {
            Self::descendant(pinned, path.strip_prefix(root).map_err(io::Error::other)?)?
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
}
pub(crate) struct Entries {
    parent: PathBuf,
    reader: cap::ReadDir,
}
impl Iterator for Entries {
    type Item = io::Result<Entry>;
    fn next(&mut self) -> Option<Self::Item> {
        // Omit Git administration before a classifier can stat, count or descend into it.
        // Direct reads of authorized project Git metadata use a separate observer.
        self.reader
            .by_ref()
            .find(|entry| !entry.as_ref().is_ok_and(|e| e.file_name().eq_ignore_ascii_case(".git")))
            .map(|entry| {
                entry.map(|inner| Entry { path: self.parent.join(inner.file_name()), inner })
            })
    }
}
