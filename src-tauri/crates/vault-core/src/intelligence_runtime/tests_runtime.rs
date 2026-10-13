//! Responsibilities: recovery of interrupted Insights jobs when an archive opens.
//! Not responsible for: queue execution or frontend rendering.
//! Failure modes: reopen strands claims, or a read reclaims live work.
use super::*;
use vault_store::{Db, Priority, params_owned as params};

fn running(vault: &crate::Vault, kind: &str) {
    let kind = kind.to_owned();
    vault.write(Db::Intelligence,Priority::Interactive,move |s| {
        s.execute("INSERT INTO intelligence_jobs(job_type,plugin_id,state,priority,attempt,dedupe_key,payload_json,created_at,scheduled_at,updated_at) VALUES(?1,?2,'running',10,1,'recovery','{}','','','')",params![kind,TITLE_NORMALIZATION_PLUGIN_ID])?;Ok(())
    }).unwrap();
}
fn state(vault: &crate::Vault) -> String {
    vault
        .store()
        .read(Db::Intelligence, |s| {
            s.query_one(
                "SELECT state FROM intelligence_jobs WHERE dedupe_key='recovery'",
                (),
                |r| Ok(r.get::<String>(0)?),
            )
        })
        .unwrap()
}
fn recovery(kind: &str) {
    for key in [None, Some([77; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        running(&vault, kind);
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let reopened = crate::Vault::open(paths, config, key).unwrap();
        assert_eq!(state(&reopened), "queued");
        assert_eq!(load_intelligence_runtime(&reopened).unwrap().queue.queued, 1);
    }
}
#[test]
fn load_intelligence_runtime_recovers_interrupted_deterministic_jobs() {
    recovery(FULL_REBUILD_JOB_TYPE);
}
#[test]
fn load_intelligence_runtime_only_recovers_running_jobs_once_per_archive() {
    let vault = crate::Vault::open_temp().unwrap();
    running(&vault, FULL_REBUILD_JOB_TYPE);
    for _ in 0..2 {
        assert_eq!(load_intelligence_runtime(&vault).unwrap().queue.running, 1);
        assert_eq!(state(&vault), "running");
    }
}
