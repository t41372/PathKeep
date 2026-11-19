//! Exercise foreground displacement detection and Retry with a real NTFS access denial.
//! The handle-level delete-pending case lives beside the Windows disposition query.
#![cfg(windows)]

use std::{
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use vault_store::{Config, Db, Migration, Priority, Store};

fn acl(path: &Path, action: &str, principal: &str) {
    let result = Command::new("icacls.exe").arg(path).args([action, principal]).output().unwrap();
    assert!(result.status.success(), "{result:?}");
}

#[test]
fn focus_during_acl_denial_then_retry_keeps_the_dirty_conversation_workers() {
    let temp = tempfile::tempdir().unwrap();
    let mut config = Config::new(temp.path());
    config.pair_seals = true;
    for role in [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel] {
        config.migrations.register(
            role,
            vec![
                Migration::sql(1, vault_store::PAIR_SCHEMA),
                Migration::sql(2, "CREATE TABLE saved(value TEXT)"),
            ],
        );
    }
    let evidence = config.paths.agent_history_evidence.clone();
    let store = Arc::new(Store::open(config).unwrap());
    let copied = Arc::new(AtomicBool::new(false));
    let observed = copied.clone();
    store.on_displacement(move |_| {
        observed.store(true, Ordering::Release);
        Ok(())
    });
    store.pause().unwrap().seal_and_checkpoint().unwrap();
    store
        .write(Db::AgentHistoryEvidence, Priority::Interactive, |s| {
            s.execute("INSERT INTO saved VALUES('captured before denial')", ())?;
            Ok(())
        })
        .unwrap();

    // A numeric SID works on localized installations. Only the disposable file's
    // read-data right changes. Remove the denial before checking the observations.
    let user = Command::new("whoami.exe").args(["/user", "/fo", "csv", "/nh"]).output().unwrap();
    assert!(user.status.success());
    let csv = String::from_utf8_lossy(&user.stdout);
    let sid = csv.trim().rsplit(',').next().unwrap().trim_matches('"');
    assert!(sid.starts_with("S-1-"));
    acl(&evidence, "/deny", &format!("*{sid}:(RD)"));
    let unavailable = store.preserve_observed_displacements();
    let reason = store.conversation_availability();
    acl(&evidence, "/remove:d", &format!("*{sid}"));

    assert!(unavailable.is_err());
    let reason = reason.unwrap_err();
    assert_eq!(reason.role, Db::AgentHistoryEvidence);
    assert!(reason.cause.access_refused(), "{reason}");
    assert!(!copied.load(Ordering::Acquire));
    assert!(store.retry_access().unwrap());
    store.conversation_availability().unwrap();
    let saved = store
        .read(Db::AgentHistoryEvidence, |s| {
            s.query_one("SELECT value FROM saved", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(saved, "captured before denial");
}
