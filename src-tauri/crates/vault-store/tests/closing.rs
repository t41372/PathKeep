//! Responsibilities: prove lock-time closure revokes every shared owner handle.
//! Not responsible for: UI state or password envelopes.
//! Failure modes: clones keep connections/key alive, lock remains held after close,
//! old handles write after reopen; a caller that panics while the store is paused leaves every
//! later request, and the close, panicking on the poisoned lock, and a store dropped while that
//! caller unwinds aborts the process.
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
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

/// The maintenance pause holds the store's workers locked across its caller's code, so a caller
/// that panics there poisons that lock. The workers are left as they were: the store goes on
/// serving, and pausing or closing it again works.
#[test]
fn a_panic_while_paused_leaves_the_store_serving_and_closable() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(Config::new(root.path())).unwrap();
    let paused = catch_unwind(AssertUnwindSafe(|| {
        let _pause = store.pause().unwrap();
        panic!("a caller's bug while the store is paused");
    }));
    assert!(paused.is_err());
    store.write(Db::Archive, Priority::Interactive, |_| Ok(())).unwrap();
    store.read(Db::Archive, |_| Ok(())).unwrap();
    drop(store.pause().unwrap());
    store.close().unwrap();
}

/// A store dropped while that caller unwinds closes in its `Drop`. A second panic there, during
/// the unwinding, would abort the process and hide the first.
#[test]
fn a_store_dropped_while_a_paused_caller_unwinds_reports_the_first_panic() {
    let root = tempfile::tempdir().unwrap();
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let store = Store::open(Config::new(root.path())).unwrap();
        let _pause = store.pause().unwrap();
        panic!("a caller's bug while the store is paused");
    }));
    let message = unwound.expect_err("the caller panicked");
    assert_eq!(message.downcast_ref::<&str>(), Some(&"a caller's bug while the store is paused"));
    Store::open(Config::new(root.path())).unwrap().close().unwrap();
}
