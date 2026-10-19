//! Responsibilities: prove deletion preserves the live app's ownership inode.
//! Not responsible for: database wipe contents (vault-core's existing real-wipe suite).
//! Failure modes: wipe unlinks the guard so another owner can acquire a new inode.
use vault_store::{Error, OwnerLock};
#[test]
fn wipe_keeps_owner_lock_file_and_exclusive_ownership() {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let guard = OwnerLock::acquire(root.path()).unwrap();
    let file = root.path().join("archive/.pk-owner.lock");
    vault_core::wipe_all_data(&paths, &Default::default(), || Ok(()), |_| Ok(()), |_| Ok(()))
        .unwrap();
    assert!(file.is_file(), "wipe must retain the ownership inode");
    assert!(matches!(OwnerLock::acquire(root.path()), Err(Error::OwnedElsewhere { .. })));
    drop(guard);
    OwnerLock::acquire(root.path()).unwrap();
}
