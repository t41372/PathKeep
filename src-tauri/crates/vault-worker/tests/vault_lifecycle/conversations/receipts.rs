//! Place a real, dismissed partial attempt beside the native corpus. Archive
//! operations must preserve its selection and history, including on a device
//! where the copied descriptor grants no access.
use super::*;
use serde_json::{from_value, json};
use vault_worker::data_kinds::{self as kinds, backups, preview};

pub(super) fn seed(f: &Fixture) -> String {
    let folder = f.sandbox.path().join("empty-claude-profile");
    fs::create_dir(&folder).unwrap();
    let folder = folder.canonicalize().unwrap();
    let mut frames = preview::start(
        &f.session,
        from_value(json!({"purpose":"enable","pickedFolder":{"tool":"claude-code","path":folder}}))
            .unwrap(),
    )
    .unwrap();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let p = rt.block_on(async {
        loop {
            let frame = frames.next_frame().await.unwrap();
            if frame["type"] == "finished" {
                break frame["payload"].clone();
            }
        }
    });
    let (tx, rx) = std::sync::mpsc::channel();
    let receipt=backups::enable(&f.session,from_value(json!({"previewId":p["previewId"],"selectedRootIds":[p["rows"][0]["rootId"]],
        "expectedPolicyToken":kinds::settings().unwrap().policy_token,"readGit":false,"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),
        Arc::new(move |name,_|{if name=="pathkeep://kind-backup-finished"{let _=tx.send(());}})).unwrap();
    rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    let tasks =
        backups::tasks(&f.session, from_value(json!({"group":"attention","limit":1})).unwrap())
            .unwrap();
    let row = &tasks["items"][0];
    assert_eq!(row["taskId"], receipt["firstSave"]["taskId"]);
    let dismissed = backups::control(
        &f.session,
        from_value(
            json!({"action":"dismiss","taskId":row["taskId"],"expectedToken":row["snapshotToken"],
        "requestId":uuid::Uuid::new_v4().to_string()}),
        )
        .unwrap(),
        Arc::new(|_, _| {}),
    )
    .unwrap();
    assert_eq!(dismissed["task"]["attention"], false);
    fingerprint(&f.vault())
}

pub(super) fn fingerprint(vault: &Vault) -> String {
    let values =
        vault
            .store()
            .read(vault_store::Db::AgentHistoryEvidence, |tx| {
                tx.query_one(
        "SELECT d.task_id,d.request_id,d.request_hash,d.scope_json,s.id,s.descriptor_json,
         r.public_id,r.status,a.request_id,a.request_hash,a.decision,d.acknowledged
         FROM ah_desktop_backups d JOIN ah_capture_selections s ON d.selection_id=s.id
         JOIN ah_backup_runs r ON r.id=d.run_id JOIN ah_backup_deliveries a ON a.task_id=d.task_id
         WHERE d.trigger='enable' AND a.decision='dismiss'",(),|r|{
            let mut values=Vec::new();for i in 0..11{values.push(r.get::<String>(i)?);}
            assert_eq!(r.get::<i64>(11)?,1);
            Ok(values)
         })
            })
            .unwrap();
    vault_core::utils::sha256_hex(&serde_json::to_vec(&values).unwrap())
}
