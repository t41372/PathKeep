//! Responsibilities: keep native lineage IDs from authorizing filesystem reads.
//! Not responsible for: denying genuine adapter-declared attachment or Pi lineage paths.
//! Failure mode: an ID containing a slash is reinterpreted as an approved-root file path.
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::native::*;
use support::{Case, args, string, write};

#[test]
fn native_parent_and_rollout_ids_never_become_paths_to_read() {
    for harness in ["codex", "deepseek"] {
        let mut case = Case::unencrypted(&format!("native-link-authority-{harness}"));
        let root = support::harness::canonical_root(&case).join(harness);
        let unrelated = root.join("unrelated.txt");
        write(&unrelated, b"synthetic private bytes outside the adapter's artifact layout");
        if harness == "codex" {
            let id = "33333333-3333-4333-8333-333333333333";
            let mut header = codex_meta(id, "/synthetic/project");
            header["payload"]["history_base"] = json!({"thread_id":unrelated,
                "end_ordinal_exclusive":1,"end_byte_offset":10});
            codex_rollout(&root, "2026-11-10T12-00-00", id, &[header, codex_user("work")]);
        } else {
            let mut header = deepseek_header("native-child", 4, "/synthetic/project");
            header["parentSession"] = json!(unrelated);
            write(
                &deepseek_folder(&root, "native-child").join("session.v4.jsonl"),
                &support::fixtures::jsonl(&[header, deepseek_user(0, "work")]),
            );
        }
        case.fixture(json!({"seed":"native-link-authority-v1","harness":harness,
            "private_data":false}));
        case.init();
        case.call(
            "backup",
            args(&["--source", &format!("{harness}={}", root.display()), "--execute"]),
        );
        let sources = case.source_index();
        case.truth(
            "native IDs never authorize a file read",
            sources.values().all(|source| string(source, "relative_path") != "unrelated.txt"),
        );
    }
}
