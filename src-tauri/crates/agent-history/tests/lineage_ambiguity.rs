//! Responsibilities: leave ambiguous physical rollout prefixes unresolved across roots.
//! Not responsible for: splitting the conversation key or preferring a capture order.
//! Failure mode: a matching thread ID and equal boundary lengths select a stranger's prefix.
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::native::*;
use support::{Case, args, string};

#[test]
fn divergent_copies_of_a_physical_rollout_do_not_supply_an_arbitrary_prefix() {
    let mut case = Case::unencrypted("ambiguous-native-prefix");
    let base = support::harness::canonical_root(&case);
    let roots = [base.join("a"), base.join("b"), base.join("child")];
    let thread = "33333333-3333-4333-8333-333333333333";
    let spec = |turns| RolloutSpec {
        thread,
        rollout: None,
        name_hour: 12,
        clock_hour: 12,
        cwd: "/synthetic/project",
        base: None,
        meta: json!({}),
        turns,
    };
    let first = codex_thread_rollout(&roots[0], spec(&["originone"]));
    let other = codex_thread_rollout(&roots[1], spec(&["origintwo"]));
    assert_eq!(
        support::fixtures::jsonl(&first.rows).len(),
        support::fixtures::jsonl(&other.rows).len()
    );
    let child = codex_thread_rollout(
        &roots[2],
        RolloutSpec {
            thread,
            rollout: Some("44444444-4444-4444-8444-444444444444"),
            name_hour: 13,
            clock_hour: 13,
            cwd: "/synthetic/project",
            base: Some((&first, 3)),
            meta: json!({}),
            turns: &["child body"],
        },
    );
    case.fixture(json!({"seed":"ambiguous-native-prefix-v1","private_data":false}));
    case.init();
    let mut arguments = Vec::new();
    for root in &roots {
        arguments.extend(args(&["--source", &format!("codex={}", root.display())]));
    }
    arguments.push("--execute".into());
    case.call("backup", arguments);
    let conversation = case.conversations().remove(0);
    let current = support::harness::current_source(&conversation);
    case.equal(
        "ambiguous evidence remains unresolved",
        current["relation"].clone(),
        json!("whole"),
    );
    let events = case.all("events", args(&["--session", string(&conversation, "id")]));
    case.equal("only its own records are read", json!(events.len()), json!(child.rows.len()));
}
