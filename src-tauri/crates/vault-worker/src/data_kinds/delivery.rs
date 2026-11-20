//! Revoke in-flight interactive results and queued notifications across a lock,
//! off or owner replacement. The capture worker has separate background authority.
use super::{Refusal, admitted_config};
use anyhow::Result;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

static READERS: LazyLock<Mutex<HashMap<PathBuf, Weak<AtomicBool>>>> =
    LazyLock::new(Default::default);

/// Buffered metadata can only be delivered during the session which admitted it.
#[derive(Clone)]
pub struct Permit(Arc<AtomicBool>);
impl Permit {
    pub fn current() -> Self {
        vault_core::project_paths()
            .ok()
            .map(|p| Self::at(&p.app_root))
            .unwrap_or_else(|| Self(Arc::new(AtomicBool::new(false))))
    }
    pub(super) fn at(root: &Path) -> Self {
        let mut readers = READERS.lock().unwrap();
        let key = std::path::absolute(root).unwrap_or_else(|_| root.to_owned());
        if let Some(epoch) = readers.get(&key).and_then(Weak::upgrade) {
            return Self(epoch);
        }
        readers.retain(|_, reader| reader.strong_count() > 0);
        let epoch = Arc::new(AtomicBool::new(true));
        readers.insert(key, Arc::downgrade(&epoch));
        Self(epoch)
    }
    pub fn valid(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Unlock creates no right to deliver something buffered before the lock.
pub(crate) fn revoke(root: &Path) {
    super::semantic::revoke(root);
    let key = std::path::absolute(root).unwrap_or_else(|_| root.to_owned());
    if let Some(epoch) = READERS.lock().unwrap().remove(&key).and_then(|reader| reader.upgrade()) {
        epoch.store(false, Ordering::Release);
    }
}

/// Check at the transport boundary even when a lock and unlock both occurred
/// during a long inventory query.
pub fn read<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let paths = vault_core::project_paths()?;
    let permit = Permit::at(&paths.app_root);
    let value = operation()?;
    let config = admitted_config(&paths)?;
    if !permit.valid() {
        return Err(if config.capture_policy.agent_state_if_present(true)
            == vault_core::KindState::Off
        {
            Refusal::Off
        } else {
            Refusal::Replaced
        }
        .into());
    }
    Ok(value)
}
