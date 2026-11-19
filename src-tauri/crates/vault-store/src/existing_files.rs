//! Enforces the owner's existing-file requirement at the actual Turso file open.
//! Layout and creation records decide which paths are protected. WALs, Scratch and
//! caller-provided spill encryption keep the underlying IO layer's behavior.

use crate::{Config, Error, IoLayer, Result};
use std::{
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::Arc,
};
use turso::core::{
    self, MemoryIO,
    io::{
        Clock, Completion, File, FileId, IO, OpenFlags, PlatformIO,
        clock::{MonotonicInstant, WallClockInstant},
    },
};

struct ExistingFiles {
    inner: IoLayer,
    required: Vec<(PathBuf, Option<FileId>)>,
}

impl Config {
    /// Turso's SDK requests Create even for existing read/write databases. A preflight
    /// alone leaves a window while another role starts or migrates.
    pub(crate) fn protect_existing_files(&mut self) -> Result<()> {
        if self.required_existing.is_empty() {
            return Ok(());
        }
        let inner = match self.io.take() {
            Some(io) => io,
            None => Arc::new(PlatformIO::new().map_err(|e| Error::Operation(Box::new(e)))?),
        };
        let required = self
            .required_existing
            .iter()
            .map(|db| {
                let path = self.paths.get(*db);
                let id = path.to_str().and_then(|p| core::io::get_file_id(p).ok());
                (path.to_owned(), id)
            })
            .collect();
        self.io = Some(Arc::new(ExistingFiles { inner, required }));
        Ok(())
    }
}

impl Clock for ExistingFiles {
    fn current_time_monotonic(&self) -> MonotonicInstant {
        self.inner.current_time_monotonic()
    }

    fn current_time_wall_clock(&self) -> WallClockInstant {
        self.inner.current_time_wall_clock()
    }
}

/// Every method forwards to the wrapped IO, so the platform IO's own choices (shared-WAL
/// coordination, fixed buffers, completion polling) stay in effect. Only `open_file` differs.
impl IO for ExistingFiles {
    fn open_file(&self, path: &str, flags: OpenFlags, direct: bool) -> core::Result<Arc<dyn File>> {
        let Some((_, expected)) =
            self.required.iter().find(|(required, _)| required == Path::new(path))
        else {
            return self.inner.open_file(path, flags, direct);
        };
        let check = || {
            if let Some(expected) = expected
                && self.inner.file_id(path)? != *expected
            {
                return Err(core::LimboError::InternalError(
                    "database identity changed during open".into(),
                ));
            }
            Ok(())
        };
        check()?;
        let file = self.inner.open_file(path, flags - OpenFlags::Create, direct)?;
        check()?;
        if file.size()? < 16 {
            return Err(core::LimboError::Corrupt("incomplete required database header".into()));
        }
        Ok(file)
    }

    fn open_shared_wal_file(&self, path: &str) -> core::Result<Arc<dyn File>> {
        self.inner.open_shared_wal_file(path)
    }

    fn remove_file(&self, path: &str) -> core::Result<()> {
        self.inner.remove_file(path)
    }

    fn supports_shared_wal_coordination(&self) -> bool {
        self.inner.supports_shared_wal_coordination()
    }

    fn step(&self) -> core::Result<()> {
        self.inner.step()
    }

    fn cancel(&self, completions: &[Completion]) -> core::Result<()> {
        self.inner.cancel(completions)
    }

    fn drain_completions(&self, completions: &[Completion]) -> core::Result<()> {
        self.inner.drain_completions(completions)
    }

    fn wait_for_completion(&self, completion: Completion) -> core::Result<()> {
        self.inner.wait_for_completion(completion)
    }

    fn generate_random_number(&self) -> i64 {
        self.inner.generate_random_number()
    }

    fn fill_bytes(&self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest)
    }

    fn get_memory_io(&self) -> Arc<MemoryIO> {
        self.inner.get_memory_io()
    }

    fn register_fixed_buffer(&self, ptr: NonNull<u8>, len: usize) -> core::Result<u32> {
        self.inner.register_fixed_buffer(ptr, len)
    }

    fn yield_now(&self) {
        self.inner.yield_now()
    }

    fn sleep(&self, duration: std::time::Duration) {
        self.inner.sleep(duration)
    }

    fn file_id(&self, path: &str) -> core::Result<FileId> {
        self.inner.file_id(path)
    }
}
