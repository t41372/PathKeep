//! Drive the command DTOs through a real vault and the native four-harness fixture.
use super::*;
use serde_json::Value;
use vault_worker::data_kinds::chats;
#[test]
fn chat_screen_commands_deliver_native_rows_counts_and_edits() {
    let f = Fixture::new(true);
    let corpus = fixtures::Fixture::four_harnesses(f.temp.path());
    let roots = corpus
        .roots
        .iter()
        .map(|(h, r)| agent_history::SourceRoot { harness: h.parse().unwrap(), root: r.clone() })
        .collect::<Vec<_>>();
    f.vault().agent_history().unwrap().backup(&roots, &Default::default()).unwrap();
    let s = &f.session;
    let list = |v| chats::list(s, from_value(v).unwrap()).unwrap();
    let counts = |v| chats::counts(s, from_value(v).unwrap()).unwrap();
    let edit = |v| {
        chats::edit(
            s,
            from_value(v).unwrap(),
            std::sync::Arc::new(|name, _| {
                assert_eq!(name, "pathkeep://agent-conversations-changed")
            }),
        )
        .unwrap()
    };
    assert_eq!(counts(json!({}))["spaceRoots"], 4);
    let groups = list(json!({"group":"project"}));
    assert_eq!(groups["groups"][0]["count"], 4);
    assert_eq!(groups["groups"][0]["remaining"], 1);
    assert_eq!(groups["groups"][0]["items"].as_array().unwrap().len(), 3);
    let id = list(json!({"harnesses":["pi"]}))["items"][0]["id"].clone();
    let h = chats::header(s, from_value(json!({"id":id})).unwrap()).unwrap();
    let src = &h["currentSource"];
    let info = chats::info(s, from_value(json!({"id":id})).unwrap()).unwrap();
    assert_eq!(info["own"]["tokens"]["input"], 10);
    assert_eq!(info["own"]["recordedUsd"], 0.0);
    let win = |v| chats::window(s, from_value(v).unwrap()).unwrap();
    let tail =
        win(json!({"id":id,"publication":h["publication"],"limit":1,"anchor":{"kind":"end"}}));
    assert!(tail["next"].is_null());
    assert!(tail["previous"].is_number());
    let address = win(
        json!({"id":id,"anchor":{"kind":"native","sourceId":src,"nativeId":"00000002"},"before":0}),
    );
    assert_eq!(address["rows"][0]["ordinal"], 2);
    assert_eq!(address["rows"][0]["footer"]["facts"]["tokens"]["output"], 2);
    let raw = win(json!({"id":id,"view":"raw","limit":1}));
    let bytes=chats::raw(s,from_value(json!({"id":id,"publication":raw["publication"],"sourceId":src,"ordinal":0,"offset":0,"limit":4096})).unwrap()).unwrap();
    let bytes: Vec<u8> = from_value(bytes["bytes"].clone()).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap()["type"], "session");
    let branches = chats::branches(s, from_value(json!({"id":id})).unwrap()).unwrap();
    assert_eq!(branches["sources"].as_array().unwrap().len(), 1);
    assert_eq!(branches["leaves"]["total"], 1);
    let prompts = win(json!({"id":id,"view":"prompts"}));
    assert_eq!(prompts["total"], 1);
    assert!(prompts["rows"][0]["preview"].as_str().unwrap().contains("uniquepineedle"));
    for (action, key) in [("star", "starred"), ("archive", "archived")] {
        assert_eq!(edit(json!({"action":action,"id":id,"on":true}))["summary"][key], true);
    }
    assert_eq!(counts(json!({"flags":["archived","starred"]}))["matchingRoots"], 1);
    assert_eq!(
        edit(json!({"action":"rename","id":id,"title":"New name"}))["summary"]["title"],
        "New name"
    );
    assert_eq!(edit(json!({"action":"trash","id":id}))["summary"]["trashed"], true);
    assert_eq!(counts(json!({}))["spaceRoots"], 3);
    assert_eq!(list(json!({"flags":["trash"]}))["items"][0]["id"], id);
    let restored = edit(json!({"action":"restore","id":id}));
    assert_eq!(restored["summary"]["archived"], true);
    assert_eq!(restored["summary"]["title"], "New name");
    assert_ne!(
        edit(json!({"action":"rename","id":id,"title":""}))["summary"]["titleSource"],
        "user"
    );
}

#[test]
fn move_import_counts_one_root_and_undo_trashes_all_three_subagents_with_it() {
    use agent_history::{Harness, SourceRoot};
    use vault_core::migration::conversations::{imports, undo};

    let transfer = tempfile::tempdir().unwrap();
    let bundle = transfer.path().join("family.pathkeep");
    {
        let donor = Fixture::new(true);
        let root = donor.temp.path().join("claude");
        let user = native::claude_user("parent", "message", None, "/project", "delegate work");
        native::claude_main(&root, "parent", &[user]);
        for child in ["a", "b", "c"] {
            native::claude_subagent(
                &root,
                "parent",
                &format!("agent-{child}.jsonl"),
                child,
                "/project",
                "delegated work",
            );
        }
        let source = SourceRoot { harness: Harness::ClaudeCode, root };
        donor.vault().agent_history().unwrap().backup(&[source], &Default::default()).unwrap();
        vault_worker::export_app_data(donor.vault(), bundle.clone(), support::PASSWORD).unwrap();
    }

    let destination = Fixture::new(true);
    let (_, installed) = vault_worker::apply_import(
        &destination.paths(),
        Some(destination.vault()),
        destination.session.owner(),
        bundle,
        vault_core::ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(support::PASSWORD.into()),
        },
    )
    .unwrap();
    let vault = installed.vault;
    let archive = vault.agent_history().unwrap();
    let family = archive.list_sessions(&Default::default()).unwrap().items;
    assert_eq!(family.len(), 4);
    let root = family.iter().find(|row| row.parent_id.is_none()).unwrap();
    assert_eq!(family.iter().filter(|row| row.parent_id.as_ref() == Some(&root.id)).count(), 3);

    let receipts = imports(&vault).unwrap();
    assert_eq!(receipts.len(), 1);
    let receipt = &receipts[0];
    assert_eq!(receipt["added"], 1);
    assert_eq!(receipt["eligible"], 1);
    let result = undo(&vault, receipt["id"].as_str().unwrap().into()).unwrap();
    assert_eq!(result["moved"], 1);
    let copy = std::path::Path::new(result["copyPath"].as_str().unwrap());
    let saved = agent_history::Archive::open(copy, support::PASSWORD.as_bytes()).unwrap();
    for row in family {
        assert!(archive.session(&row.id).unwrap().trashed);
        assert!(!saved.session(&row.id).unwrap().trashed);
    }
    let after = imports(&vault).unwrap();
    assert_eq!(after[0]["added"], 1);
    assert_eq!(after[0]["eligible"], 0);
}
