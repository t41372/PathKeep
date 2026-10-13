//! Responsibilities: prove lock-time closure revokes every shared owner handle.
//! Not responsible for: UI state or password envelopes.
//! Failure modes: clones keep connections/key alive, lock remains held after close,
//! old handles write after reopen.
use std::sync::Arc;
use vault_store::{Config, Db, Error, Priority, Store};

#[test]
fn close_revokes_clones_and_releases_ownership() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(Config::new(root.path())).unwrap());
    let retained = store.clone();
    store.close().unwrap();
    assert!(matches!(retained.read(Db::Archive, |_| Ok(())), Err(Error::Closed)));
    assert!(matches!(
        retained.write(Db::Archive, Priority::Interactive, |_| Ok(())),
        Err(Error::Closed)
    ));
    let reopened = Store::open(Config::new(root.path())).unwrap();
    reopened.read(Db::Archive, |_| Ok(())).unwrap();
    store.close().unwrap();
}
