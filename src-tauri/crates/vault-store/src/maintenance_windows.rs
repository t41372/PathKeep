//! Releases delete-pending file handles before probing the public name again.
//! A Windows sharing error alone never establishes that the named file is absent.
use crate::{
    Db, Result, Store,
    worker::{HeldFiles, Workers},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Seek, SeekFrom},
    os::windows::fs::OpenOptionsExt,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) fn pending_name(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32 | 33 | 303))
}

impl Store {
    /// Copy the held generation while browsing can still use its read guard.
    /// Delete-on-close keeps this private anchor independent of the trial or
    /// listed copy (and of anyone replacing or freeing that copy).
    pub(super) fn anchor_pending(&self, workers: &[(Db, Workers)]) -> Result<Vec<(Db, HeldFiles)>> {
        let mut close = Vec::new();
        for role in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
            let path = self.paths.get(role);
            if !turso::core::io::get_file_id(path.to_str().expect("validated role path"))
                .is_err_and(|e| pending_name(&e))
            {
                continue;
            }
            let dir = vault_layout::path(self.owner()?.root(), vault_layout::ArtifactId::Staging);
            fs::create_dir_all(&dir)?;
            // Retry may already have closed the SQL workers while retaining their
            // original handles. Release those originals too, after making the anchor.
            for (_, _, files) in
                self.retired_files.lock().unwrap().iter_mut().filter(|(_, db, _)| *db == role)
            {
                *files = anchor(files, &dir, path)?;
            }
            if let Some((_, worker)) = workers.iter().find(|(db, _)| *db == role) {
                worker.quarantine.store(true, Ordering::Release);
                worker.maintenance(|_| Ok(()))?;
                close.push((role, anchor(&worker.held_files, &dir, path)?));
            }
        }
        Ok(close)
    }

    pub(super) fn release_pending(
        &self,
        workers: &mut Vec<(Db, Workers)>,
        close: Vec<(Db, HeldFiles)>,
    ) {
        for (role, files) in close {
            workers.retain(|(db, _)| *db != role);
            self.retired_files.lock().unwrap().push((self.generation(), role, files));
        }
    }
}

fn anchor(files: &HeldFiles, dir: &Path, main: &Path) -> Result<HeldFiles> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x04000000;
    let mut anchors = Vec::new();
    for (suffix, input) in files {
        let mut name = main.as_os_str().to_owned();
        name.push(suffix);
        let mut source = input.try_clone()?;
        source.seek(SeekFrom::Start(0))?;
        let mut spool: File = loop {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = dir
                .join(format!("maintenance-agent-preview-handle-{}-{serial}", std::process::id()));
            match OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
                .open(path)
            {
                Ok(file) => break file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        };
        super::maintenance::copy_bytes(Path::new(&name), &mut source, &mut spool)?;
        spool.sync_all()?;
        anchors.push((suffix.clone(), spool));
    }
    Ok(anchors)
}
