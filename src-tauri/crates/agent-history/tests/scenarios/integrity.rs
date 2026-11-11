//! Responsibilities: exercise authorization and evidence failures through the CLI.
//! Not responsible for: mocking a filesystem or injecting private implementation failures.
//! Failure modes: occupied roots/output files are overwritten; arbitrary transcript paths
//! become read authority; corrupt compression loses raw evidence or looks complete.
use crate::execute;
use crate::support::fixtures::{SHARED_ID, jsonl, pi};
use crate::support::{Case, args, identifier, items, source_state, string, write};
use serde_json::json;
use std::fs;
use std::io::Write;

#[test]
fn occupied_roots_and_existing_exports_are_never_overwritten() {
    let mut case = Case::new("explicit-destinations");
    let occupied = case.archive.join("existing-user-file.txt");
    write(&occupied, b"this file does not belong to an agent-history archive");
    let before = source_state(&case.archive);
    case.fixture(json!({"seed":"explicit-destinations-v1","private_data":false}));
    case.fail("init", vec![]);
    case.truth(
        "initialization refuses an occupied unknown root",
        source_state(&case.archive) == before,
    );
    fs::remove_file(occupied).unwrap();
    case.init();
    let root = case.root.join("pi");
    pi(&root, "/synthetic/project", SHARED_ID, "exportneedle");
    case.call("backup", execute(args(&["--source", &format!("pi={}", root.display())])));
    let session = items(&case.call("list", vec![]))[0].clone();
    let output = case.root.join("readable.md");
    let arguments = args(&[
        "--session",
        string(&session, "id"),
        "--format",
        "markdown",
        "--output",
        output.to_str().unwrap(),
    ]);
    case.fail("export", arguments.clone());
    case.truth("export requires execution consent", !output.exists());
    write(&output, b"existing export must survive");
    case.fail("export", execute(arguments));
    case.truth(
        "explicit export does not imply overwrite consent",
        fs::read(&output).unwrap() == b"existing export must survive",
    );
}

#[test]
fn attachment_references_do_not_grant_access_outside_the_source_root() {
    let mut case = Case::new("attachment-boundaries");
    let root = case.root.join("pi");
    let file = pi(&root, "/synthetic/project", SHARED_ID, "attachmentneedle");
    let forbidden = case.root.join("unrelated/private.txt");
    let external_secret = "unrelatedfilesecretmustneverberead";
    write(&forbidden, external_secret.as_bytes());
    let missing = root.join("missing-tool-output.txt");
    let mut output = fs::OpenOptions::new().append(true).open(&file).unwrap();
    for (id, target) in [("outside-result", &forbidden), ("missing-result", &missing)] {
        output.write_all(&jsonl(&[json!({
            "type":"message","id":id,"parentId":"00000003",
            "timestamp":"2026-11-10T12:00:01.000Z",
            "message":{"role":"toolResult","toolCallId":"synthetic-call","toolName":"bash",
                "content":[{"type":"text","text":"truncated output remains in the transcript"}],
                "details":{"fullOutputPath":target},"isError":false,"timestamp":1794312001000_i64}
        })])).unwrap();
    }
    drop(output);
    let before = source_state(&root);
    case.fixture(json!({"seed":"attachment-boundaries-v1","private_data":false}));
    case.init();
    let sources = args(&["--source", &format!("pi={}", root.display())]);
    let plan = case.call("plan", sources.clone());
    case.truth(
        "plan cannot discover files outside approved roots",
        !plan["sources"].to_string().contains(forbidden.to_str().unwrap()),
    );
    let result = case.call("backup", execute(sources));
    case.equal(
        "missing attachment evidence is explicit",
        result["status"].clone(),
        json!("partial"),
    );
    case.truth(
        "boundary or missing-file diagnostics are present",
        !result["issues"].as_array().unwrap().is_empty(),
    );
    let secret_search = case.call("search", args(&["--query", external_secret]));
    case.equal(
        "unrelated file contents are not imported",
        secret_search["total"].clone(),
        json!(0),
    );
    let session = items(&case.call("list", vec![]))[0].clone();
    let revisions = case.call("revisions", args(&["--source-id", string(&session, "source_id")]));
    let exported = case.root.join("attachment-transcript.raw");
    case.call(
        "raw-export",
        execute(args(&[
            "--revision",
            &identifier(&items(&revisions)[0], "id"),
            "--output",
            exported.to_str().unwrap(),
        ])),
    );
    case.truth(
        "attachment failures do not erase native transcript bytes",
        fs::read(exported).unwrap() == fs::read(&file).unwrap(),
    );
    case.truth("capture does not edit the source tree", before == source_state(&root));
}

#[test]
fn corrupt_compressed_sources_remain_recoverable_with_partial_coverage() {
    let mut case = Case::new("corrupt-compression");
    let root = case.root.join("codex");
    let path = root.join("sessions/2026/11/10/rollout-2026-11-10T12-00-00-corrupt.jsonl.zst");
    // The filename and compression header are recognizable; the final frame is incomplete.
    let bytes = vec![0x28, 0xb5, 0x2f, 0xfd, 0x20, 0x50, 0x01];
    write(&path, &bytes);
    case.fixture(
        json!({"seed":"corrupt-compression-v1","source_hex":"28b52ffd205001","private_data":false}),
    );
    case.init();
    let source = args(&["--source", &format!("codex={}", root.display())]);
    let plan = case.call("plan", source.clone());
    let source_id = plan["sources"][0]["source_id"]
        .as_str()
        .expect("plan exposes stable source identity")
        .to_owned();
    let backup = case.call("backup", execute(source));
    case.equal(
        "decompression failure is not complete coverage",
        backup["status"].clone(),
        json!("partial"),
    );
    case.truth("decompression has a diagnostic", !backup["issues"].as_array().unwrap().is_empty());
    let revisions = case.call("revisions", args(&["--source-id", &source_id]));
    case.equal("raw capture survives parser failure", json!(items(&revisions).len()), json!(1));
    let output = case.root.join("corrupt-but-preserved.raw");
    case.call(
        "raw-export",
        execute(args(&[
            "--revision",
            &identifier(&items(&revisions)[0], "id"),
            "--output",
            output.to_str().unwrap(),
        ])),
    );
    case.truth(
        "raw corrupt input is recoverable byte for byte",
        fs::read(output).unwrap() == bytes,
    );
}

#[test]
fn missing_inherited_rollout_is_reported_without_discarding_the_child() {
    let mut case = Case::new("missing-lineage");
    let root = case.root.join("codex");
    let path =
        root.join(format!("sessions/2026/11/10/rollout-2026-11-10T12-00-00-{SHARED_ID}.jsonl"));
    let bytes = jsonl(&[
        json!({"type":"session_meta","timestamp":"2026-11-10T12:00:00.000Z",
            "ordinal":0,"payload":{"id":SHARED_ID,"session_id":SHARED_ID,
                "cwd":"/synthetic/child","timestamp":"2026-11-10T12:00:00.000Z",
                "originator":"codex_cli_rs","source":"cli","history_mode":"paginated",
                "history_base":{"thread_id":"99999999-9999-4999-8999-999999999999",
                    "end_byte_offset":100,"end_ordinal_exclusive":2}}}),
        json!({"type":"response_item","timestamp":"2026-11-10T12:00:01.000Z",
            "ordinal":1,"payload":{"type":"message","role":"user",
                "content":[{"type":"input_text","text":"retainedchildneedle"}]}}),
    ]);
    write(&path, &bytes);
    case.fixture(json!({"seed":"missing-lineage-v1","private_data":false}));
    case.init();
    let backup =
        case.call("backup", execute(args(&["--source", &format!("codex={}", root.display())])));
    case.equal(
        "missing inherited history cannot claim complete coverage",
        backup["status"].clone(),
        json!("partial"),
    );
    case.truth(
        "lineage diagnosis distinguishes absent inherited history",
        backup["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"].as_str().is_some_and(|code| code.contains("lineage"))),
    );
    let sessions = case.call("list", vec![]);
    case.equal("available child remains accessible", json!(items(&sessions).len()), json!(1));
    let revision =
        case.call("revisions", args(&["--source-id", string(&items(&sessions)[0], "source_id")]));
    let output = case.root.join("child.raw");
    case.call(
        "raw-export",
        execute(args(&[
            "--revision",
            &identifier(&items(&revision)[0], "id"),
            "--output",
            output.to_str().unwrap(),
        ])),
    );
    case.truth("native byte offsets survive export unchanged", fs::read(output).unwrap() == bytes);
}
