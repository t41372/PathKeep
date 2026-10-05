//! Responsibilities: identify an already-open native owner at legacy SQL boundaries.
//! Not responsible for: opening storage, retaining keys or authorising UI operations.
//! Failure modes: a closed Store appears unlocked; a missing legacy password breaks
//! non-SQL commands; status opens a second owner. Tests: session/dispatch regressions.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};
use vault_store::Store;
fn owners() -> &'static Mutex<HashMap<PathBuf, Weak<Store>>> {
    static OWNERS: OnceLock<Mutex<HashMap<PathBuf, Weak<Store>>>> = OnceLock::new();
    OWNERS.get_or_init(Default::default)
}
pub(super) fn register(root: &Path, store: &Arc<Store>) {
    let mut owners = owners().lock().expect("native owners");
    owners.retain(|_, store| store.strong_count() > 0);
    owners.insert(root.to_owned(), Arc::downgrade(store));
}
pub(crate) fn store(root: &Path) -> Option<Arc<Store>> {
    owners()
        .lock()
        .expect("native owners")
        .get(root)
        .and_then(Weak::upgrade)
        .filter(|s| s.is_open())
}
