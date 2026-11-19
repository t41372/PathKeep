//! Place a real, dismissed partial attempt beside the native corpus. Archive
//! operations must preserve its selection and history, including on a device
//! where the copied descriptor grants no access.
use super::*;
use serde_json::{from_value, json};
use vault_worker::data_kinds::{self as kinds, backups, preview};

pub(super) fn seed(f: &Fixture) -> String {
    let folder = f.sandbox.path().join("empty-claude-profile");
    fs::create_dir(&folder).unwrap();
    seed_from(f, "claude-code", &folder);
    fingerprint(&f.vault())
}

fn seed_from(f: &Fixture, tool: &str, folder: &std::path::Path) {
    let folder = folder.canonicalize().unwrap();
    let mut frames = preview::start(
        &f.session,
        from_value(json!({"purpose":"enable","pickedFolder":{"tool":tool,"path":folder}})).unwrap(),
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
    let mut command = json!({"action":"dismiss","taskId":row["taskId"],"expectedToken":row["snapshotToken"],"requestId":uuid::Uuid::new_v4().to_string()});
    if let Some(notice) = row["formatNotices"].get(0) {
        command["action"] = json!("dismiss-notice");
        command["notice"] = notice.clone();
    }
    let dismissed =
        backups::control(&f.session, from_value(command).unwrap(), Arc::new(|_, _| {})).unwrap();
    assert_eq!(dismissed["task"]["attention"], false);
}

pub(super) fn fingerprint(vault: &Vault) -> String {
    let values =
        vault
            .store()
            .read(vault_store::Db::AgentHistoryEvidence, |tx| {
                tx.query_one(
        "SELECT d.task_id,d.request_id,d.request_hash,d.scope_json,s.id,s.descriptor_json,
         r.public_id,r.status,a.request_id,a.request_hash,a.decision,d.acknowledged,s.first_task_id
         FROM ah_desktop_backups d JOIN ah_capture_selections s ON d.selection_id=s.id
         JOIN ah_backup_runs r ON r.id=d.run_id JOIN ah_backup_deliveries a ON a.task_id=d.task_id
         WHERE d.trigger='enable' AND a.decision='dismiss'",(),|r|{
            let mut values=Vec::new();for i in 0..11{values.push(r.get::<String>(i)?);}
            assert_eq!(r.get::<i64>(11)?,1);
            assert_eq!(r.get::<String>(12)?,values[0]);
            Ok(values)
         })
            })
            .unwrap();
    vault_core::utils::sha256_hex(&serde_json::to_vec(&values).unwrap())
}

#[test]
fn retained_notes_and_dismissed_format_version_survive_archive_installation() {
    let f = Fixture::new(false);
    let root = f.sandbox.path().join("future-pi");
    fs::create_dir_all(root.join("sessions")).unwrap();
    fs::write(
        root.join("sessions/one.jsonl"),
        br#"{"type":"session","id":"future","version":999,"cwd":"/project"}
"#,
    )
    .unwrap();
    seed_from(&f, "pi", &root);
    let check = |v: &Vault| {
        let fact = v
            .store()
            .read(vault_store::Db::AgentHistoryEvidence, |s| {
                s.query_one(
                    "SELECT n.subject,n.code,c.seen,c.fresh,f.version,f.dismissed
             FROM ah_format_notices f JOIN ah_run_notes o ON o.run_id=f.run_id
             JOIN ah_capture_notes n ON n.id=o.note_id
             JOIN ah_run_note_counts c ON c.run_id=o.run_id AND c.code=n.code
             WHERE c.version=f.version AND c.tool=f.tool",
                    (),
                    |r| {
                        Ok((
                            r.get::<String>(0)?,
                            r.get::<String>(1)?,
                            r.get::<i64>(2)?,
                            r.get::<i64>(3)?,
                            r.get::<String>(4)?,
                            r.get::<i64>(5)?,
                        ))
                    },
                )
            })
            .unwrap();
        assert_eq!(
            fact,
            ("sessions/one.jsonl".into(), "pi-unsupported-version".into(), 1, 1, "999".into(), 1)
        );
    };
    check(&f.vault());
    f.rekey(ArchiveMode::Encrypted, None, Some(PASSWORD));
    check(&f.vault());
    let current = f.vault();
    let bundle = f.sandbox.path().join("notes.pathkeep");
    vault_worker::export_app_data(&current, bundle.clone(), EXPORT_PASSWORD).unwrap();
    let maintenance = f.session.begin_maintenance().unwrap();
    let (_, installed) = vault_worker::apply_import(
        current.paths(),
        Some(&current),
        f.session.owner(),
        bundle,
        vault_core::ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(EXPORT_PASSWORD.into()),
        },
    )
    .unwrap();
    f.session
        .install_archive(
            installed,
            Some(EXPORT_PASSWORD.into()),
            vault_worker::KeychainAfterInstall::AsSaved,
        )
        .unwrap();
    drop(maintenance);
    check(&f.vault());
}
