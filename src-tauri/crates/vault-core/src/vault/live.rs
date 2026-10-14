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

#[derive(Clone)]
struct SearchOwner {
    indexer: Weak<crate::search::Indexer>,
    sessions: Weak<crate::search::SearchSessions>,
}
fn searches() -> &'static Mutex<HashMap<PathBuf, SearchOwner>> {
    static SEARCH: OnceLock<Mutex<HashMap<PathBuf, SearchOwner>>> = OnceLock::new();
    SEARCH.get_or_init(Default::default)
}
pub(super) fn register_search(
    root: &Path,
    indexer: &Arc<crate::search::Indexer>,
    sessions: &Arc<crate::search::SearchSessions>,
) {
    let mut entries = searches().lock().unwrap();
    entries.retain(|_, owner| owner.indexer.strong_count() > 0);
    entries.insert(
        root.to_owned(),
        SearchOwner { indexer: Arc::downgrade(indexer), sessions: Arc::downgrade(sessions) },
    );
}
pub(crate) fn search(
    root: &Path,
) -> anyhow::Result<(Arc<Store>, Arc<crate::search::Indexer>, Arc<crate::search::SearchSessions>)> {
    let store =
        store(root).ok_or_else(|| anyhow::anyhow!("search requires an open Vault owner"))?;
    let entries = searches().lock().unwrap();
    let owner = entries.get(root).ok_or_else(|| anyhow::anyhow!("search owner is closed"))?;
    Ok((
        store,
        owner.indexer.upgrade().ok_or_else(|| anyhow::anyhow!("search owner closed"))?,
        owner.sessions.upgrade().ok_or_else(|| anyhow::anyhow!("search sessions closed"))?,
    ))
}
