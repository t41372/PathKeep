//! Responsibilities: prove the app can reserve ownership before touching databases.
//! Not responsible for: storage migrations or IPC.
//! Failure modes: public ownership acquisition opens databases; a second owner wins;
//! dropping the app reservation fails to release the lock or unlinks the lockfile.
use vault_store::{Error, OwnerLock};
#[test]
fn app_reserves_owner_lock_without_opening_databases() {
    let root = tempfile::tempdir().unwrap();
    let lock = OwnerLock::acquire(root.path()).unwrap();
    assert!(matches!(OwnerLock::acquire(root.path()), Err(Error::OwnedElsewhere { .. })));
    let entries: Vec<_> = std::fs::read_dir(root.path().join("archive")).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert!(root.path().join("archive/.pk-owner.lock").is_file());
    drop(lock);
    assert!(root.path().join("archive/.pk-owner.lock").is_file());
    OwnerLock::acquire(root.path()).unwrap();
}
