//! Responsibilities: scope parse completeness to the inherited prefix across every read API.
//! Not responsible for: dropping malformed raw tails or changing supplied contract assertions.
//! Failure mode: an abandoned malformed tail makes a valid inherited line look incomplete.
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::native::*;
use support::{Case, args, string, write};

#[test]
fn inherited_parse_gaps_only_affect_the_records_on_the_current_line() {
    for inside in [false, true] {
        let mut case = Case::unencrypted(&format!("bounded-parser-coverage-{inside}"));
        let root = support::harness::canonical_root(&case).join("codex");
        let thread = "33333333-3333-4333-8333-333333333333";
        let meta = codex_meta(thread, "/synthetic/project");
        let reply = codex_reply(thread, "answer", "reply");
        let prefix = if inside {
            let mut bytes = support::fixtures::jsonl(&[meta]);
            bytes.extend(b"{\"invalid\":\n");
            bytes.extend(support::fixtures::jsonl(&[reply]));
            bytes
        } else {
            support::fixtures::jsonl(&[meta, codex_user("first"), reply])
        };
        let path =
            root.join(format!("sessions/2026/11/10/rollout-2026-11-10T12-00-00-{thread}.jsonl"));
        let mut bytes = prefix.clone();
        if !inside {
            bytes.extend(b"{\"unfinished\":");
        }
        write(&path, &bytes);
        let mut header = codex_meta(thread, "/synthetic/project");
        header["payload"]["timestamp"] = json!("2026-11-10T13:00:00Z");
        header["payload"]["history_base"] = json!({"thread_id":thread,
            "end_ordinal_exclusive":3,"end_byte_offset":prefix.len()});
        codex_rollout(
            &root,
            "2026-11-10T13-00-00",
            &format!("{thread}_44444444-4444-4444-8444-444444444444"),
            &[header, codex_user_at("2026-11-10T13:01:00Z", "continuedcoverage")],
        );
        case.fixture(json!({"seed":"bounded-parser-coverage-v1","inside":inside,
            "private_data":false}));
        case.init();
        case.call("backup", args(&["--source", &format!("codex={}", root.display()), "--execute"]));
        let conversation = case.conversations().remove(0);
        case.equal(
            "only included parse gaps affect list",
            conversation["complete"].clone(),
            json!(!inside),
        );
        let search = case.call("search", args(&["--query", "continuedcoverage"]));
        case.equal("search uses the same coverage", search["complete"].clone(), json!(!inside));
        let verify = case.call("verify", vec![]);
        case.equal(
            "verify uses the same coverage",
            verify["normalization_complete"].clone(),
            json!(!inside),
        );
        let output = case.unused_path("coverage-export");
        let report = case.call(
            "export",
            args(&[
                "--session",
                string(&conversation, "id"),
                "--output",
                output.to_str().unwrap(),
                "--execute",
            ]),
        );
        case.equal("export uses the same coverage", report["complete"].clone(), json!(!inside));
    }
}
