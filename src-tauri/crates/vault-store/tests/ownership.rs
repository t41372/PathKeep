//! Responsibilities: prove process ownership and thread lifetime via the public facade.
//! Not responsible for: testing advisory locks with unrelated programs that ignore them.
//! Failure modes: second open touches databases; drop fails to join/release; failed open leaks lock.
mod support;
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use vault_store::{Db, Error, Store};
#[test]
fn child_open() {
    let Some(root) = std::env::var_os("VAULT_STORE_LOCK_TEST_ROOT") else { return };
    assert!(matches!(
        Store::open(support::config(std::path::Path::new(&root))),
        Err(Error::OwnedElsewhere { .. })
    ));
}
#[test]
fn second_process_is_refused_and_drop_joins_before_releasing_lock() {
    let (dir, store) = support::open();
    let before =
        std::fs::metadata(dir.path().join("archive/history-vault.db")).unwrap().modified().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_open", "--nocapture"])
        .env("VAULT_STORE_LOCK_TEST_ROOT", dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        before,
        std::fs::metadata(dir.path().join("archive/history-vault.db")).unwrap().modified().unwrap()
    );
    let dropped = Arc::new(AtomicUsize::new(0));
    struct OnExit(Arc<AtomicUsize>);
    impl Drop for OnExit {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    thread_local! {static OWNER_EXIT: std::cell::RefCell<Option<OnExit>> = const {std::cell::RefCell::new(None)};}
    for db in Db::ALL {
        let dropped = dropped.clone();
        store
            .write(db, vault_store::Priority::Interactive, move |_| {
                OWNER_EXIT.with(|v| *v.borrow_mut() = Some(OnExit(dropped)));
                Ok(())
            })
            .unwrap();
    }
    std::thread::scope(|threads| {
        for db in Db::ALL {
            let barrier = Arc::new(std::sync::Barrier::new(2));
            for _ in 0..2 {
                let barrier = barrier.clone();
                let dropped = dropped.clone();
                let store = &store;
                threads.spawn(move || {
                    store
                        .read(db, move |_| {
                            OWNER_EXIT.with(|v| *v.borrow_mut() = Some(OnExit(dropped)));
                            barrier.wait();
                            Ok(())
                        })
                        .unwrap()
                });
            }
        }
    });
    drop(store);
    assert_eq!(dropped.load(Ordering::SeqCst), 15);
    let store = Store::open(support::config(dir.path())).unwrap();
    assert_eq!(support::count(&store), 0);
}
