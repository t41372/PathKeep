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
