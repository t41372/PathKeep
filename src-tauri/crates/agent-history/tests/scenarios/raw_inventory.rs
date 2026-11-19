//! Responsibilities: prove raw evidence remains discoverable when no session can be parsed.
//! Not responsible for: private SQL probes or relying on the live source after capture.
//! Failure modes specified before implementation: a bounded preview becomes the only
//! inventory; failed parsing hides raw revisions; keyset pages omit evidence; deleting
//! native files makes a previously captured revision impossible to find or export.
use crate::execute;
use crate::support::{Case, args, identifier, items, source_state, string, write};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

#[test]
fn unparseable_evidence_beyond_the_preview_limit_is_recoverable_after_source_deletion() {
    let mut case = Case::new("raw-inventory-beyond-preview");
    let root = case.root.join("codex");
    let mut originals = BTreeMap::new();
    for index in 0_u16..257 {
        let relative = format!(
            "sessions/2026/11/10/rollout-2026-11-10T12-00-00-{index:08x}-0000-4000-8000-000000000000.jsonl.zst"
        );
        // Zstandard magic with an invalid frame descriptor, plus distinct source bytes.
        let mut bytes = vec![0x28, 0xb5, 0x2f, 0xfd, 0xff];
        bytes.extend_from_slice(&index.to_be_bytes());
        write(&root.join(&relative), &bytes);
        originals.insert(relative, bytes);
    }
    case.fixture(json!({
        "seed":"raw-inventory-beyond-preview-v1","source_count":257,
        "invalid_frame_prefix_hex":"28b52ffdff","suffix":"big-endian u16 file index",
        "harness":"codex","page_size":37,"private_data":false,
    }));
    let before = source_state(&root);
    let sources = args(&["--source", &format!("codex={}", root.display())]);
    let preview = case.call("plan", sources.clone());
    case.equal(
        "preview counts every source",
        preview["total_files"].clone(),
        json!(originals.len()),
    );
    case.equal(
        "preview explicitly reports its display bound",
        preview["truncated"].clone(),
        json!(true),
    );
    case.truth(
        "preview is a sample rather than a complete inventory",
        preview["sources"].as_array().unwrap().len() < originals.len(),
    );

    case.init();
    let backup = case.call("backup", execute(sources));
    case.equal(
        "invalid compression leaves the raw backup successful",
        backup["status"].clone(),
        json!("complete"),
    );
    case.equal(
        "parser failures do not suppress raw capture",
        backup["captured"].clone(),
        json!(originals.len()),
    );
    case.truth("capture leaves all original bytes unchanged", source_state(&root) == before);
    let sessions = case.call("list", args(&["--harness", "codex"]));
    case.equal(
        "this scenario cannot recover through parsed sessions",
        json!(items(&sessions).len()),
        json!(0),
    );
    fs::remove_dir_all(&root).unwrap();

    let mut inventory: Vec<Value> = vec![];
    let mut after: Option<String> = None;
    loop {
        let mut arguments = args(&["--harness", "codex", "--kind", "transcript", "--limit", "37"]);
        if let Some(cursor) = &after {
            arguments.extend(["--after".into(), cursor.clone()]);
        }
        let page = case.call("sources", arguments);
        case.truth("raw inventory respects its page bound", items(&page).len() <= 37);
        inventory.extend_from_slice(items(&page));
        after = page["next_cursor"].as_str().map(str::to_owned);
        if after.is_none() {
            break;
        }
        assert!(inventory.len() <= originals.len(), "source pagination must make progress");
    }
    let ids = inventory.iter().map(|source| string(source, "id")).collect::<BTreeSet<_>>();
    let paths =
        inventory.iter().map(|source| string(source, "relative_path")).collect::<BTreeSet<_>>();
    let expected_paths = originals.keys().map(String::as_str).collect::<BTreeSet<_>>();
    case.equal(
        "all raw source identities survive without live files",
        json!(ids.len()),
        json!(originals.len()),
    );
    case.equal(
        "raw source pagination contains no duplicate rows",
        json!(inventory.len()),
        json!(ids.len()),
    );
    case.equal(
        "raw inventory preserves every original locator",
        json!(paths),
        json!(expected_paths),
    );
    case.truth(
        "every failed-parser source names a retained raw revision",
        inventory.iter().all(|source| !source["latest_revision_id"].is_null()),
    );

    // Recover across the ordered inventory without 257 password derivations; the
    // archive-level integrity tests verify each manifest/chunk independently.
    for index in [0, inventory.len() / 2, inventory.len() - 1] {
        let source = &inventory[index];
        let revisions = case.call("revisions", args(&["--source-id", string(source, "id")]));
        case.equal(
            "raw-only source has its complete retained version",
            json!(items(&revisions).len()),
            json!(1),
        );
        case.equal(
            "inventory head resolves to the retained revision",
            json!(identifier(source, "latest_revision_id")),
            json!(identifier(&items(&revisions)[0], "id")),
        );
        let output = case.root.join(format!("recovered-{index}.raw"));
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
            "raw-only source exports the original bytes after deletion",
            fs::read(output).unwrap() == originals[string(source, "relative_path")],
        );
    }
    let verified = case.call("verify", vec![]);
    case.equal(
        "every retained raw manifest verifies",
        verified["status"].clone(),
        json!("complete"),
    );
}
