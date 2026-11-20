//! Responsibilities: observe actual residue after the worker closes and wipes a populated owner.
//! Not responsible for: future unknown-file refusal, scheduler-failure injection or power loss.

use super::support::*;

#[test]
fn wipe_removes_markers_checkpoints_snapshots_and_owned_exports_without_changing_the_browser() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(!encrypted);
        fixture.seed();
        fixture.rekey(
            if encrypted {
                vault_core::ArchiveMode::Encrypted
            } else {
                vault_core::ArchiveMode::Plaintext
            },
            (!encrypted).then_some(PASSWORD),
            encrypted.then_some(NEW_PASSWORD),
        );
        let snapshot = safety_snapshot();
        vault_worker::run_full_archive_restore(
            &fixture.session,
            encrypted.then_some(NEW_PASSWORD),
            &vault_core::SnapshotRestoreRequest { snapshot_path: snapshot.display().to_string() },
        )
        .unwrap();
        let bundle = fixture.root.join("exports/pathkeep/format-3/lifecycle.pathkeep");
        vault_worker::export_app_data(&fixture.vault(), bundle, EXPORT_PASSWORD).unwrap();
        let browser = std::fs::read(fixture.browser()).unwrap();
        let before =
            observe(&fixture.root, Some(&fixture.vault()), &format!("wipe-before-{encrypted}"));
        assert_eq!(before["unknown"], serde_json::json!([]));
        assert_eq!(before, expected(&format!("wipe-before-{encrypted}")));
        let report = vault_worker::wipe_all_data(vault_worker::WIPE_CONFIRMATION_WORD, || {
            fixture.session.close_vault().map_err(anyhow::Error::msg)
        })
        .unwrap();
        assert_eq!(report, vault_core::WipeReport::default());
        let after = observe(&fixture.root, None, &format!("wipe-after-{encrypted}"));
        assert_eq!(after["unknown"], serde_json::json!([]));
        assert_eq!(after, expected("wiped"));
        assert_eq!(std::fs::read(fixture.browser()).unwrap(), browser);
        assert!(
            !vault_core::load_config(&vault_core::project_paths_with_root(&fixture.root))
                .unwrap()
                .initialized
        );
    }
}
