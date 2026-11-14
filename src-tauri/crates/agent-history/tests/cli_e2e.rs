//! Responsibilities: prove archive behavior through the production CLI and real files.
//! Not responsible for: mocking storage, pinning private helpers or claiming platform coverage.
//! Failure modes were specified before implementation in tests/README.md. Commands are not
//! executed until the final verification phase requested by the user.
#[path = "scenarios/integrity.rs"]
mod integrity;
#[path = "scenarios/raw_inventory.rs"]
mod raw_inventory;
mod support;

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use support::fixtures::{FUTURE_SENTINEL, Fixture, PRIVATE_TEXT, SHARED_ID, jsonl, pi, pi_message};
use support::{Case, archive_contains, args, identifier, items, source_state, string, write};

fn execute(mut arguments: Vec<String>) -> Vec<String> {
    arguments.push("--execute".into());
    arguments
}

fn all_events(case: &mut Case, session: &str) -> Vec<Value> {
    let mut events = vec![];
    let mut after: Option<String> = None;
    loop {
        let mut arguments = args(&["--session", session, "--limit", "2"]);
        if let Some(cursor) = &after {
            arguments.extend(["--after".into(), cursor.to_owned()]);
        }
        let page = case.call("events", arguments);
        events.extend_from_slice(items(&page));
        after = page["next_cursor"].as_str().map(str::to_owned);
        if after.is_none() {
            break;
        }
        assert!(events.len() < 10_000, "pagination must make progress");
    }
    let ordinals =
        events.iter().map(|event| event["ordinal"].as_u64().unwrap()).collect::<BTreeSet<_>>();
    case.equal("event pages do not repeat ordinals", json!(ordinals.len()), json!(events.len()));
    events
}

#[test]
fn four_native_harnesses_remain_separate_and_export_their_exact_evidence() {
    let mut case = Case::new("four-native-harnesses");
    let fixture = Fixture::four_harnesses(&case.root);
    case.fixture(fixture.description());
    let before = source_state(&fixture.source_tree);
    let plan = case.call("plan", fixture.source_args());
    case.truth(
        "plan discovers old and current native artifacts",
        plan["sources"].as_array().unwrap().len() >= 5,
    );
    case.truth("planning does not initialize an archive", !case.archive.exists());
    case.init();
    case.fail("backup", fixture.source_args());
    let empty = case.call("list", vec![]);
    case.equal("backup requires explicit execution", json!(items(&empty).len()), json!(0));
    case.call("backup", execute(fixture.source_args()));

    let page = case.call("list", args(&["--project", &fixture.project, "--limit", "100"]));
    let sessions = items(&page).to_vec();
    case.equal("one current session per harness", json!(sessions.len()), json!(4));
    let ids = sessions.iter().map(|session| string(session, "id")).collect::<BTreeSet<_>>();
    case.equal("reused native IDs have independent archive identities", json!(ids.len()), json!(4));
    for session in &sessions {
        case.equal("native ID is preserved", session["native_id"].clone(), json!(SHARED_ID));
        case.equal(
            "project is read from native evidence",
            session["project_path"].clone(),
            json!(fixture.project),
        );
    }

    let search = case.call(
        "search",
        args(&["--query", "sharedprojectneedle", "--project", &fixture.project, "--limit", "100"]),
    );
    let found = items(&search).iter().map(|hit| string(hit, "session_id")).collect::<BTreeSet<_>>();
    case.equal("project search spans all harnesses", json!(found), json!(ids));
    case.equal(
        "successful search declares complete index coverage",
        search["complete"].clone(),
        json!(true),
    );

    for (harness, expected_path) in
        ["claude-code", "codex", "pi", "deepseek"].iter().zip(&fixture.files)
    {
        let session = sessions.iter().find(|item| string(item, "harness") == *harness).unwrap();
        let events = all_events(&mut case, string(session, "id"));
        case.truth("native transcript is accessible after reopening", !events.is_empty());
        if *harness == "claude-code" {
            let repeated = events
                .iter()
                .filter(|event| event["payload"]["message"]["id"] == "same-response")
                .count();
            case.equal(
                "same provider response ID does not erase branches",
                json!(repeated),
                json!(2),
            );
            case.truth(
                "unknown nested payload survives",
                events.iter().any(|event| event["payload"].to_string().contains(FUTURE_SENTINEL)),
            );
        }
        let revisions = case.call(
            "revisions",
            args(&["--source-id", string(support_current_source(session), "source_id")]),
        );
        let latest = &items(&revisions)[0];
        let output = case.root.join(format!("{harness}.raw"));
        case.call(
            "raw-export",
            execute(args(&[
                "--revision",
                &identifier(latest, "id"),
                "--output",
                output.to_str().unwrap(),
            ])),
        );
        case.truth(
            "raw export is byte exact, including compression",
            fs::read(&output).unwrap() == fs::read(expected_path).unwrap(),
        );
    }

    let claude = sessions.iter().find(|session| session["harness"] == "claude-code").unwrap();
    let normalized = case.root.join("claude-normalized.jsonl");
    case.call(
        "export",
        execute(args(&[
            "--session",
            string(claude, "id"),
            "--format",
            "jsonl",
            "--output",
            normalized.to_str().unwrap(),
        ])),
    );
    let normalized = fs::read_to_string(normalized).unwrap();
    case.truth("normalized JSONL retains opaque evidence", normalized.contains(FUTURE_SENTINEL));
    for line in normalized.lines() {
        serde_json::from_str::<Value>(line).expect("each export line is independently valid JSON");
    }
    let markdown = case.root.join("claude.md");
    case.call(
        "export",
        execute(args(&[
            "--session",
            string(claude, "id"),
            "--format",
            "markdown",
            "--output",
            markdown.to_str().unwrap(),
        ])),
    );
    let markdown = fs::read_to_string(markdown).unwrap();
    case.truth(
        "Markdown includes both branches",
        markdown.contains("branchalphaneedle")
            && markdown.contains("branchbetaneedle")
            && markdown.contains("unselectedbranchneedle"),
    );

    let replay = case.call("backup", execute(fixture.source_args()));
    case.equal("unchanged input creates no raw revision", replay["captured"].clone(), json!(0));
    case.truth(
        "read-only sources retain bytes and modification times",
        before == source_state(&fixture.source_tree),
    );
    case.truth(
        "raw and indexed conversation content is encrypted at rest",
        !archive_contains(&case.archive, PRIVATE_TEXT.as_bytes()),
    );
    case.wrong_password();
    let verify = case.call("verify", vec![]);
    case.equal(
        "archive verifies after all reopen operations",
        verify["status"].clone(),
        json!("complete"),
    );
}

#[test]
fn rewrite_truncation_partial_tail_and_deletion_keep_every_raw_revision() {
    let mut case = Case::new("source-revisions");
    let root = case.root.join("pi");
    let file = pi(&root, "/synthetic/project", SHARED_ID, "originalneedle");
    let source_args = args(&["--source", &format!("pi={}", root.display())]);
    case.fixture(json!({"seed":"source-revisions-v1","harness":"pi","private_data":false}));
    case.init();
    case.call("backup", execute(source_args.clone()));
    let session = items(&case.call("list", vec![]))[0].clone();
    let original = fs::read(&file).unwrap();
    let modified = fs::metadata(&file).unwrap().modified().unwrap();
    let rewritten = String::from_utf8(original.clone())
        .unwrap()
        .replace("originalneedle", "replacedneedle")
        .into_bytes();
    assert_eq!(original.len(), rewritten.len(), "exercise same-size source replacement");
    write(&file, &rewritten);
    File::open(&file).unwrap().set_modified(modified).unwrap();
    case.call("backup", execute(source_args.clone()));
    let new_hit = case.call("search", args(&["--query", "replacedneedle"]));
    case.truth("same size and mtime do not hide replacement", !items(&new_hit).is_empty());
    let old_hit = case.call("search", args(&["--query", "originalneedle"]));
    case.equal("current search does not retain replaced text", old_hit["total"].clone(), json!(0));

    let header_end = rewritten.iter().position(|byte| *byte == b'\n').unwrap() + 1;
    let truncated = rewritten[..header_end].to_vec();
    write(&file, &truncated);
    case.call("backup", execute(source_args.clone()));
    let mut incomplete = rewritten.clone();
    incomplete.extend_from_slice(b"{\"type\":\"message\",\"id\":\"incomplete");
    write(&file, &incomplete);
    let partial = case.call("backup", execute(source_args.clone()));
    case.equal(
        "incomplete tail has explicit partial coverage",
        partial["status"].clone(),
        json!("partial"),
    );
    case.truth(
        "partial coverage gives a machine-readable reason",
        partial["issues"].as_array().unwrap().iter().any(|issue| issue["code"].is_string()),
    );
    case.truth("backup does not repair source newline", fs::read(&file).unwrap() == incomplete);
    fs::remove_file(&file).unwrap();
    case.call("backup", execute(source_args));

    let revisions = case.call(
        "revisions",
        args(&["--source-id", string(support_current_source(&session), "source_id")]),
    );
    case.equal(
        "all four distinct versions survive deletion",
        json!(items(&revisions).len()),
        json!(4),
    );
    let mut exported = vec![];
    for (index, revision) in items(&revisions).iter().enumerate() {
        let output = case.root.join(format!("retained-{index}.raw"));
        case.call(
            "raw-export",
            execute(args(&[
                "--revision",
                &identifier(revision, "id"),
                "--output",
                output.to_str().unwrap(),
            ])),
        );
        exported.push(fs::read(output).unwrap());
    }
    for expected in [original, rewritten, truncated, incomplete] {
        case.truth(
            "an observed raw version remains exactly recoverable",
            exported.contains(&expected),
        );
    }
}

#[test]
fn isolated_roots_and_keyset_pages_do_not_merge_or_omit_sessions() {
    let mut case = Case::new("roots-and-pagination");
    let first = case.root.join("pi-first");
    let second = case.root.join("pi-second");
    pi(&first, "/synthetic/one", SHARED_ID, "firstrootneedle");
    pi(&second, "/synthetic/two", SHARED_ID, "secondrootneedle");
    for index in 0..5 {
        pi(&first, "/synthetic/one", &format!("pagination-{index}"), "another conversation");
    }
    case.fixture(json!({"seed":"roots-pagination-v1","session_count":7,"private_data":false}));
    case.init();
    case.call(
        "backup",
        execute(args(&[
            "--source",
            &format!("pi={}", first.display()),
            "--source",
            &format!("pi={}", second.display()),
        ])),
    );
    let mut ids = BTreeSet::new();
    let mut seen = 0;
    let mut after: Option<String> = None;
    loop {
        let mut arguments = args(&["--limit", "2", "--sort", "created"]);
        if let Some(cursor) = &after {
            arguments.extend(["--after".into(), cursor.to_owned()]);
        }
        let page = case.call("list", arguments);
        for session in items(&page) {
            ids.insert(string(session, "id").to_owned());
            seen += 1;
        }
        after = page["next_cursor"].as_str().map(str::to_owned);
        if after.is_none() {
            break;
        }
        assert!(seen <= 7, "pagination must not repeat a page");
    }
    case.equal("every physical session appears once", json!([seen, ids.len()]), json!([7, 7]));
    let one = case.call("search", args(&["--query", "firstrootneedle"]));
    let two = case.call("search", args(&["--query", "secondrootneedle"]));
    case.truth(
        "same native ID in two roots stays independent",
        string(&items(&one)[0], "session_id") != string(&items(&two)[0], "session_id"),
    );
    let project = case.call("list", args(&["--project", "/synthetic/two"]));
    case.equal(
        "project filtering does not merge root identities",
        json!(items(&project).len()),
        json!(1),
    );
}

#[test]
fn search_reaches_past_eight_mebibytes_and_newly_appended_content() {
    let mut case = Case::new("large-conversation");
    let root = case.root.join("pi");
    let file = pi(&root, "/synthetic/large", SHARED_ID, "startneedle");
    let mut output = fs::OpenOptions::new().append(true).open(&file).unwrap();
    let block = "ordinary transcript text ".repeat(1_400);
    for index in 0..320 {
        let row = pi_message(&format!("large-{index}"), Some("00000003"), "user", &block);
        output.write_all(&jsonl(&[row])).unwrap();
    }
    output
        .write_all(&jsonl(&[pi_message(
            "beyond-cap",
            Some("00000003"),
            "user",
            "beyondeightmebibytesneedle",
        )]))
        .unwrap();
    drop(output);
    assert!(fs::metadata(&file).unwrap().len() > 8 * 1024 * 1024);
    case.fixture(json!({"seed":"large-conversation-v1","padding_records":320,"padding_repeats":1400,"private_data":false}));
    case.init();
    let source = args(&["--source", &format!("pi={}", root.display())]);
    case.call("backup", execute(source.clone()));
    let found = case.call("search", args(&["--query", "beyondeightmebibytesneedle"]));
    case.truth("no conversation-level indexing cutoff hides its end", !items(&found).is_empty());
    let mut output = fs::OpenOptions::new().append(true).open(&file).unwrap();
    output
        .write_all(&jsonl(&[pi_message(
            "append-1",
            Some("beyond-cap"),
            "user",
            "appendedafterbackupneedle",
        )]))
        .unwrap();
    drop(output);
    case.call("backup", execute(source));
    let appended = case.call("search", args(&["--query", "appendedafterbackupneedle"]));
    case.truth(
        "incremental backup publishes newly searchable content",
        !items(&appended).is_empty(),
    );
    let verify = case.call("verify", vec![]);
    case.equal(
        "large archive survives reopen and verifies",
        verify["status"].clone(),
        json!("complete"),
    );
}

fn support_current_source(item: &serde_json::Value) -> &serde_json::Value {
    item["sources"].as_array().unwrap().iter().find(|source| source["current"] == true).unwrap()
}
