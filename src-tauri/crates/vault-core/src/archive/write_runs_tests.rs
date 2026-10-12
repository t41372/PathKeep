//! Responsibilities: pin how a run ends and what may change it afterwards.
//! Not responsible for: the operations that start runs.
//! Failure modes: a failure after a run ended rewrites its status; a step after a committed run
//! fails without a trace, or with the run's status changed.
use super::*;

fn status(vault: &Vault, run: i64) -> (String, Option<String>, String) {
    vault
        .store()
        .read(Db::Archive, move |s| {
            s.query_one(
                "SELECT status,error_message,warnings_json FROM runs WHERE id=?1",
                [run],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
        })
        .unwrap()
}

#[test]
fn a_run_that_ended_keeps_how_it_ended() {
    let vault = Vault::open_temp().unwrap();
    let run = start(&vault, "retention_prune", "manual", &[]).unwrap();
    complete(&vault, run, json!({}), json!([]), now_rfc3339()).unwrap();
    end_failed(&vault, run, &anyhow::anyhow!("manifest write failed"), json!({}), None);
    assert_eq!(status(&vault, run), ("success".into(), None, "[]".into()));
}

#[test]
fn a_failed_step_after_a_run_ended_is_a_warning_on_it() {
    let vault = Vault::open_temp().unwrap();
    let run = start(&vault, "import", "manual", &[]).unwrap();
    complete(&vault, run, json!({}), json!([]), now_rfc3339()).unwrap();
    assert_eq!(after_success(&vault, run, "the next step", Ok(7)), Some(7));
    let missing: Option<()> =
        after_success(&vault, run, "the next step", Err(anyhow::anyhow!("disk full")));
    assert_eq!(missing, None);
    let (status, error, warnings) = status(&vault, run);
    assert_eq!((status.as_str(), error), ("success", None));
    let warnings: Vec<Value> = serde_json::from_str(&warnings).unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["code"], REVIEW_RECORD_FAILED);
    assert_eq!(warnings[0]["diagnostic"], "disk full");
}
