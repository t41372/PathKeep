//! A refused command must leave the device grant, evidence and delivery ledger untouched.
use super::*;
use agent_history::{Harness, SourceRoot, capture};
use vault_store::Db;

fn evidence(f: &Fixture) -> [i64; 5] {
    f.vault().store().read(Db::AgentHistoryEvidence, |s| s.query_one(
        "SELECT (SELECT count(*) FROM ah_capture_selections), (SELECT count(*) FROM ah_backup_runs),
                (SELECT count(*) FROM ah_backup_deliveries), (SELECT count(*) FROM ah_sources),
                (SELECT count(*) FROM ah_revisions)", (), |r| Ok([r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?])
    )).unwrap()
}

fn rejected<T: std::fmt::Debug>(
    f: &Fixture,
    expected: &str,
    command: impl FnOnce() -> anyhow::Result<T>,
) {
    let before = evidence(f);
    let policy = kinds::settings().unwrap().policy;
    let error = command().unwrap_err().downcast::<Refusal>().unwrap();
    assert_eq!(error.code(), expected);
    assert_eq!(kinds::settings().unwrap().policy, policy);
    assert_eq!(evidence(f), before, "a refusal must precede staging, capture and dispatch");
}

#[test]
fn overlapping_folders_are_refused_at_review_and_by_the_grant_used_at_enable() {
    let f = Fixture::new(true);
    let archive = f.root.canonicalize().unwrap();
    let inside = archive.join("exports");
    std::fs::create_dir_all(&inside).unwrap();
    let parent = archive.parent().unwrap();
    let sibling = parent.join("pi");
    pi(&sibling.join("sessions/one.jsonl"), "one", 3);
    let ticket = review(&f, &sibling, "pi");
    for folder in [inside.as_path(), parent, &archive] {
        let request =
            from_value(json!({"purpose":"enable","pickedFolder":{"tool":"pi","path":folder}}))
                .unwrap();
        assert!(
            matches!(preview::start(&f.session,request),Err(e) if e.downcast_ref::<Refusal>().is_some_and(|r|r.code()=="scope-denied"))
        );
        let source = SourceRoot { root: folder.into(), harness: Harness::Pi };
        let id = capture::root_identity(&source).unwrap();
        let directory =
            capture::directory_identity(&capture::directory_handle(folder).unwrap()).unwrap();
        // Even a descriptor with the genuine directory identity is unusable when
        // the current archive location overlaps it (e.g. after a vault move).
        assert_eq!(
            capture::directory_issue(&source, &id, &directory, &archive),
            Some("root-changed"),
            "{folder:?}"
        );
        let forged=from_value(json!({"previewId":ticket["previewId"],"selectedRootIds":[ticket["rows"][0]["rootId"],id],
            "expectedPolicyToken":kinds::settings().unwrap().policy_token,"requestId":uuid::Uuid::new_v4().to_string(),"readGit":false})).unwrap();
        rejected(&f, "scope-denied", || backups::enable(&f.session, forged, Arc::new(|_, _| {})));
    }
}

#[test]
fn enable_checks_the_callers_token_and_rejects_a_second_setup() {
    let f = Fixture::new(true);
    let root = f.temp.path().join("pi");
    pi(&root.join("sessions/test.jsonl"), "selected", 3);
    let stale = kinds::settings().unwrap().policy_token;
    kinds::update_sources(
        &f.session,
        from_value(json!({"readGit":false,"expectedToken":stale})).unwrap(),
        |_| {},
    )
    .unwrap();
    let ticket = review(&f, &root, "pi");
    let request = |ticket: &Value, token: String| {
        from_value(json!({"previewId":ticket["previewId"],"selectedRootIds":[ticket["rows"][0]["rootId"]],
            "readGit":false,"expectedPolicyToken":token,"requestId":uuid::Uuid::new_v4().to_string()})).unwrap()
    };
    assert_ne!(stale, kinds::settings().unwrap().policy_token);
    rejected(&f, "policy-changed", || {
        backups::enable(&f.session, request(&ticket, stale), Arc::new(|_, _| {}))
    });
    finished(&f, |e| backups::enable(&f.session, enable_request(&ticket), e));
    for state in [KindState::On, KindState::Paused] {
        if state == KindState::Paused {
            kinds::set_state(
                &f.session,
                kinds::SetState {
                    kind: DataKind::AgentHistory,
                    state,
                    expected_policy_token: kinds::settings().unwrap().policy_token,
                },
                |_| {},
            )
            .unwrap();
        }
        let fresh = review(&f, &root, "pi");
        rejected(&f, "policy-changed", || {
            backups::enable(&f.session, enable_request(&fresh), Arc::new(|_, _| {}))
        });
    }
}

#[test]
fn disabling_a_granted_tool_refuses_manual_backup_before_dispatch() {
    let f = Fixture::new(true);
    let root = f.temp.path().join("pi");
    let file = root.join("sessions/selected.jsonl");
    pi(&file, "one", 3);
    finished(&f, |e| backups::enable(&f.session, enable_request(&review(&f, &root, "pi")), e));
    kinds::update_sources(&f.session,from_value(json!({"toolChanges":{"pi":false},"expectedToken":kinds::settings().unwrap().policy_token})).unwrap(),|_|{}).unwrap();
    pi(&file, "new-content", 3);
    let old = std::time::UNIX_EPOCH + Duration::from_secs(123);
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_accessed(old))
        .unwrap();
    rejected(&f, "scope-denied", || {
        backups::start(&f.session,from_value(json!({"scope":{"type":"tool","kind":"agent-history","tool":"pi"},"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(|_,_|{}))
    });
    assert_eq!(std::fs::metadata(file).unwrap().accessed().unwrap(), old);
}

#[test]
fn enable_refuses_a_lost_selection_before_staging_it() {
    let f = Fixture::new(true);
    let lost = f.temp.path().join("offline");
    pi(&lost.join("sessions/missing.jsonl"), "lost", 3);
    let first = review(&f, &lost, "pi");
    let offline = lost.with_extension("removed");
    std::fs::rename(&lost, &offline).unwrap();
    rejected(&f, "root-changed", || {
        backups::enable(&f.session, enable_request(&first), Arc::new(|_, _| {}))
    });
}

#[test]
fn editing_sources_cannot_add_a_folder_lost_after_review() {
    let f = Fixture::new(true);
    let root = f.temp.path().join("pi");
    pi(&root.join("sessions/stable.jsonl"), "stable", 3);
    let kept = review(&f, &root, "pi");
    finished(&f, |e| backups::enable(&f.session, enable_request(&kept), e));
    let lost = f.temp.path().join("offline");
    pi(&lost.join("sessions/missing.jsonl"), "lost", 3);
    let add = review(&f, &lost, "pi");
    std::fs::rename(&lost, lost.with_extension("removed")).unwrap();
    let patch = from_value(json!({
        "selectedRootIds":[kept["rows"][0]["rootId"],add["rows"][0]["rootId"]],
        "previewId":add["previewId"],
        "expectedToken":kinds::settings().unwrap().policy_token
    }))
    .unwrap();
    rejected(&f, "root-changed", || kinds::update_sources(&f.session, patch, |_| {}));
}

/// Keep the ticket, policy, open store and selected inode valid. Only the
/// current archive location changes between the picker and the Enable command.
#[cfg(unix)]
#[test]
fn genuine_review_cannot_enable_after_archive_moves_across_grant_boundary() {
    use std::{fs, os::unix::fs::symlink};
    for archive_contains_source in [false, true] {
        let f = Fixture::new(true);
        let moved = f.temp.path().join("moved");
        let selected = if archive_contains_source {
            moved.join("exports/picked")
        } else {
            f.temp.path().join("picked")
        };
        pi(&selected.join("sessions/test.jsonl"), "one", 3);
        let ticket = review(&f, &selected, "pi");
        let destination = if archive_contains_source {
            // Preserve the selected inode at exactly the same canonical path.
            let held = f.temp.path().join("held");
            fs::rename(&moved, &held).unwrap();
            fs::rename(&f.root, &moved).unwrap();
            fs::create_dir_all(moved.join("exports")).unwrap();
            fs::rename(held.join("exports/picked"), &selected).unwrap();
            moved
        } else {
            let target = selected.join("archive");
            fs::rename(&f.root, &target).unwrap();
            target
        };
        symlink(&destination, &f.root).unwrap();
        let id = ticket["rows"][0]["rootId"].as_str().unwrap().to_owned();
        assert!(
            preview::reviewed_sources(ticket["previewId"].as_str().unwrap(), &[id]).is_ok(),
            "the reviewed directory itself must still match"
        );
        rejected(&f, "root-changed", || {
            backups::enable(&f.session, enable_request(&ticket), Arc::new(|_, _| {}))
        });
    }
}
