//! The screens consume the same four native transcripts as the public CLI.
#[allow(dead_code)]
#[path = "support/fixtures.rs"]
mod fixtures;
use agent_history::{Archive, SourceRoot};
use serde_json::{Value, json};

#[test]
fn native_conversations_open_page_and_edit() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixtures::Fixture::four_harnesses(dir.path());
    let a = Archive::create_unencrypted(&dir.path().join("archive")).unwrap();
    let roots = fixture
        .roots
        .iter()
        .map(|(tool, root)| SourceRoot { harness: tool.parse().unwrap(), root: root.clone() })
        .collect::<Vec<_>>();
    a.backup(&roots, &Default::default()).unwrap();
    let page = a.chat_list(&serde_json::from_value(json!({"limit":2})).unwrap()).unwrap();
    let next = a
        .chat_list(&serde_json::from_value(json!({"limit":2,"cursor":page["nextCursor"]})).unwrap())
        .unwrap();
    let rows = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(next["items"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let h = a.chat_header(id).unwrap();
        assert_eq!(h["summary"]["id"], id);
        assert!(row["messages"].as_i64().unwrap() >= 1);
        let q = serde_json::from_value(json!({"id":id,"anchor":{"kind":"end"},"limit":2})).unwrap();
        let w = a.chat_window(&q, &Default::default()).unwrap();
        assert_eq!(w["next"], Value::Null);
        assert!(!w["rows"].as_array().unwrap().is_empty());
        let info = a.chat_info(id).unwrap();
        match row["harness"].as_str().unwrap() {
            "claude-code" | "pi" => assert_eq!(info["own"]["tokens"]["output"], 2),
            "codex" | "deepseek" => assert!(info["own"]["tokens"].is_null()),
            _ => panic!("unexpected tool"),
        }
    }
}

fn write(path: &std::path::Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[path = "support/native.rs"]
mod native;

fn capture(a: &Archive, h: agent_history::Harness, root: &std::path::Path) {
    a.backup(
        &[SourceRoot { harness: h, root: root.into() }],
        &agent_history::capture::BackupOptions { read_git: false, ..Default::default() },
    )
    .unwrap();
}
fn all(a: &Archive) -> Vec<Value> {
    a.chat_list(&Default::default()).unwrap()["items"].as_array().unwrap().clone()
}

#[test]
fn streamed_responses_are_charged_once_children_roll_up_and_forks_own_only_their_suffix() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("claude");
    let cwd = "/project";
    let name = "origin";
    let prompt = native::claude_user(name, "u", None, cwd, "original question");
    let mut prefix = vec![prompt];
    for (i, tool) in ["Read", "Read", "Agent"].iter().enumerate() {
        let mut call =
            native::claude_tool_call(name, &format!("a{i}"), "u", cwd, json!({"file_path":"f"}));
        call["message"]["id"] = json!("response");
        call["message"]["content"][0]["name"] = json!(tool);
        call["message"]["usage"] = json!({"input_tokens":20,"output_tokens":5});
        prefix.push(call);
    }
    prefix.push(json!({"type":"system","subtype":"turn_duration","durationMs":60000,"sessionId":name,"timestamp":native::EPOCH+60000}));
    native::claude_main(&root, name, &prefix);
    let child = native::claude_subagent(&root, name, "agent-kid.jsonl", "kid", cwd, "child work");
    let mut sub: Vec<Value> = std::fs::read_to_string(&child)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    sub[1]["message"]["usage"] = json!({"input_tokens":5,"output_tokens":1});
    write(&child, &fixtures::jsonl(&sub));
    let mut fork = prefix.clone();
    fork.push(native::claude_user("fork", "fork-u", Some("a2"), cwd, "fork question"));
    fork.push(native::claude_reply("fork", "fork-a", "fork-u", cwd, "fork answer"));
    native::claude_main(&root, "fork", &fork);
    capture(&a, agent_history::Harness::ClaudeCode, &root);
    let rows = all(&a);
    assert_eq!(rows.len(), 2);
    let parent = rows.iter().find(|r| r["childCount"] == 1).unwrap();
    assert_eq!(
        (parent["messages"].as_i64(), parent["turns"].as_i64(), parent["tokensTotal"].as_i64()),
        (Some(1), Some(1), Some(31))
    );
    let info = a.chat_info(parent["id"].as_str().unwrap()).unwrap();
    assert_eq!(info["own"]["calls"], 3);
    assert_eq!(info["subagentTokens"], 6);
    assert_eq!(info["own"]["agentMs"], 60000);
    assert_eq!(rows.iter().find(|r| r["id"] != parent["id"]).unwrap()["tokensTotal"], 12);
}

fn insights_tokens(a: &Archive) -> Value {
    let mut observed = Value::Null;
    a.agent_insights(
        &Default::default(),
        "UTC",
        native::EPOCH,
        &Default::default(),
        &Default::default(),
        |ty, p| {
            if ty == "overview" {
                observed = p["tokens"].clone();
            }
            Ok(())
        },
    )
    .unwrap();
    observed
}

#[test]
fn codex_cumulative_usage_follows_the_retained_cut_and_counts_abandoned_work_once() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("codex");
    let thread = fixtures::SHARED_ID;
    let usage = |input, read, output| json!({"timestamp":native::TIME,"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":input,"cached_input_tokens":read,"output_tokens":output}}}});
    let base = vec![
        native::codex_meta(thread, "/p"),
        native::codex_user("question"),
        usage(100, 40, 10),
        native::codex_user("abandoned"),
        usage(160, 60, 18),
    ];
    native::codex_rollout(&root, "2026-11-10T12-00-00", thread, &base);
    let mut meta = native::codex_meta_at(thread, "/p", "2026-11-10T13:00:00Z");
    meta["payload"]["history_base"] = json!({"thread_id":thread,"end_ordinal_exclusive":3,"end_byte_offset":fixtures::jsonl(&base[..3]).len()});
    native::codex_rollout(
        &root,
        "2026-11-10T13-00-00",
        &format!("{thread}_22222222-2222-4222-8222-222222222222"),
        &[
            meta,
            native::codex_user("replacement"),
            usage(100, 40, 10),
            usage(130, 50, 14),
            usage(130, 50, 14),
        ],
    );
    capture(&a, agent_history::Harness::Codex, &root);
    let rows = all(&a);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["tokensTotal"], 212);
    assert_eq!(insights_tokens(&a), json!({"input":120,"cacheRead":70,"cacheWrite":0,"output":22}));
    let i = a.chat_info(rows[0]["id"].as_str().unwrap()).unwrap();
    assert_eq!(i["own"]["tokens"], json!({"input":120,"cacheRead":70,"cacheWrite":0,"output":22}));
    assert_eq!(
        i["currentLineTokens"],
        json!({"input":80,"cacheRead":50,"cacheWrite":0,"output":14})
    );
}

#[test]
fn pi_leaf_footers_and_toc_follow_the_selected_reply() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    let mut earlier = fixtures::pi_message("left", Some("p"), "assistant", "earlier");
    earlier["message"]["usage"]["output"] = json!(0);
    let latest = fixtures::pi_message("right", Some("p"), "assistant", "replacement");
    write(
        &root.join("sessions/p.jsonl"),
        &fixtures::jsonl(&[
            json!({"type":"session","version":3,"id":"pi","cwd":"/one"}),
            fixtures::pi_message("p", None, "user", "Question"),
            earlier,
            latest,
        ]),
    );
    capture(&a, agent_history::Harness::Pi, &root);
    let row = &all(&a)[0];
    let id = &row["id"];
    assert_eq!(row["tokensTotal"], 22);
    for (leaf, text, tokens) in [(None, "replacement", 2), (Some(2), "earlier", 0)] {
        let response = a
            .chat_window(
                &serde_json::from_value(
                    json!({"id":id,"leaf":leaf,"anchor":{"kind":"end"},"limit":1}),
                )
                .unwrap(),
                &Default::default(),
            )
            .unwrap();
        let reply = &response["rows"][0];
        assert_eq!(reply["parts"][0]["text"], text);
        assert_eq!(reply["footer"]["facts"]["tokens"]["output"], tokens);
        let prompts = a
            .chat_window(
                &serde_json::from_value(json!({"id":id,"leaf":leaf,"view":"prompts"})).unwrap(),
                &Default::default(),
            )
            .unwrap();
        assert_eq!(prompts["total"], 1);
        assert_eq!(prompts["rows"][0]["visibleRank"], 0);
    }
}

#[test]
fn deepseek_seeded_child_owns_only_work_after_the_last_inherited_marker() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("d");
    let response = |n, id: &str, i, r, o| {
        json!({"type":"assistant/message","seq":n,"time":native::EPOCH+n,"surfaceOp":"append","data":{
        "turn":1,"step":1,"stream":[],"message":{"id":id,"role":"assistant","source":{"kind":"model","model":"d","provider":"d"},"content":[{"type":"text","text":"answer"}]},
        "usage":{"inputTokens":i,"cacheReadTokens":r,"outputTokens":o}}})
    };
    let parent = native::deepseek_header("main", 4, "/p");
    let first = response(1, "first", 7, 3, 2);
    write(
        &native::deepseek_folder(&root, "main").join("session.jsonl"),
        &fixtures::jsonl(&[parent, native::deepseek_user(0, "question"), first.clone()]),
    );
    let mut child = native::deepseek_header("child", 4, "/p");
    child["origin"] = json!("subagent");
    child["parentSession"] = json!("main");
    child["isSeeded"] = json!(true);
    write(
        &native::deepseek_folder(&root, "child").join("session.jsonl"),
        &fixtures::jsonl(&[
            child,
            first,
            json!({"type":"session/end-seed","seq":2,"time":native::EPOCH+2,"data":{"inherited":true}}),
            response(3, "new", 4, 2, 1),
        ]),
    );
    capture(&a, agent_history::Harness::Deepseek, &root);
    let rows = all(&a);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["tokensTotal"], 19);
    assert_eq!(insights_tokens(&a), json!({"input":11,"cacheRead":5,"cacheWrite":0,"output":3}));
    let info = a.chat_info(rows[0]["id"].as_str().unwrap()).unwrap();
    assert_eq!(info["subagentTokens"], 7);
    assert_eq!(info["own"]["messages"], 1);
}

#[test]
fn paging_survives_a_new_head_and_local_day_requires_activity() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    let input = |id: &str, project: &str, start: &str, last: &str| {
        let mut u = fixtures::pi_message("u", None, "user", id);
        u["timestamp"] = json!(start);
        let mut reply = fixtures::pi_message("a", Some("u"), "assistant", "answer");
        reply["timestamp"] = json!(start);
        let mut later = fixtures::pi_message("u2", Some("a"), "user", "later");
        later["timestamp"] = json!(last);
        write(
            &root.join(format!("sessions/{id}.jsonl")),
            &fixtures::jsonl(&[
                json!({"type":"session","version":3,"id":id,"cwd":project,"timestamp":start}),
                u,
                reply,
                later,
            ]),
        );
    };
    input("a", "/A", "2026-11-01T01:00:00Z", "2026-11-12T05:00:00Z");
    input("b", "/B", "2026-11-05T00:00:00Z", "2026-11-11T05:00:00Z");
    input("c", "/A", "2026-11-06T00:00:00Z", "2026-11-10T05:00:00Z");
    capture(&a, agent_history::Harness::Pi, &root);
    let list = |q| a.chat_list(&serde_json::from_value(q).unwrap()).unwrap();
    let p = list(json!({"limit":2}));
    assert_eq!(p["items"][0]["title"], "a");
    input("new", "/B", "2026-11-13T00:00:00Z", "2026-11-13T05:00:00Z");
    capture(&a, agent_history::Harness::Pi, &root);
    let next = list(json!({"cursor":p["nextCursor"],"limit":2}));
    assert_eq!(next["items"][0]["title"], "c");
    let day = list(json!({"day":"2026-11-11","timeZone":"America/Los_Angeles","group":"day"}));
    assert_eq!(day["items"].as_array().unwrap().len(), 1);
    assert_eq!(day["items"][0]["title"], "a");
    assert!(
        list(json!({"day":"2026-11-08","timeZone":"America/Los_Angeles"}))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_codex_prompt_and_its_ui_completion_share_one_display_rank() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("codex");
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let id = fixtures::SHARED_ID;
    let completion = json!({"timestamp":native::TIME,"type":"event_msg","payload":{"type":"item_completed","thread_id":id,"turn_id":"turn-a","item":{"type":"UserMessage","id":"ui-prompt","content":[{"type":"text","text":"question"}]}}});
    native::codex_rollout(
        &root,
        "2026-11-10T12-00-00",
        id,
        &[
            native::codex_meta(id, "/p"),
            native::codex_user("question"),
            completion,
            native::codex_reply(id, "answer", "answer"),
        ],
    );
    capture(&a, agent_history::Harness::Codex, &root);
    let rows = all(&a);
    let id = &rows[0]["id"];
    let h = a.chat_header(id.as_str().unwrap()).unwrap();
    assert_eq!(h["visibleRecords"], 2);
    let window =
        |v| a.chat_window(&serde_json::from_value(v).unwrap(), &Default::default()).unwrap();
    assert_eq!(window(json!({"id":id}))["rows"].as_array().unwrap().len(), 2);
    let raw = window(json!({"id":id,"view":"raw"}));
    assert_eq!(raw["rows"][2]["mirrorOfOrdinal"], 1);
    assert_eq!(raw["rows"][2]["visibleRank"], 0);
    assert_eq!(raw["rows"][2]["visible"], false);
    assert_eq!(
        window(
            json!({"id":id,"anchor":{"kind":"native","sourceId":h["currentSource"],"nativeId":"ui-prompt"},"before":0})
        )["rows"][0]["ordinal"],
        1
    );
}

#[test]
fn attachments_edit_results_and_pr_filters_use_structured_native_evidence() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("claude");
    let name = "main";
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let file = native::claude_upload(&root, name, "clip.txt", b"retained bytes");
    let mut call = native::claude_tool_call(
        name,
        "editor",
        "u",
        "/p",
        json!({"file_path":"main.rs","old_string":"first\nold\nlast\n","new_string":"first\nnew\nlast\n"}),
    );
    call["message"]["content"][0]["name"] = json!("Edit");
    call["message"]["model"] = json!("claude-sonnet-4-5");
    native::claude_main(
        &root,
        name,
        &[
            native::claude_user(name, "u", None, "/p", "Edit the file"),
            native::claude_attachment(name, "attachment", &file),
            call,
            json!({"type":"user","uuid":"result","parentUuid":"editor","sessionId":name,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call-editor","content":"done","is_error":false}]}, "toolUseResult":{"durationMs":75}}),
            json!({"type":"pr-link","sessionId":name,"prNumber":42,"prRepository":"example/repo","prUrl":"https://github.com/example/repo/pull/42"}),
        ],
    );
    capture(&a, agent_history::Harness::ClaudeCode, &root);
    let filtered = a
        .chat_list(&serde_json::from_value(json!({"flags":["hasPr","hasEdits"]})).unwrap())
        .unwrap();
    let id = &filtered["items"][0]["id"];
    let h = a.chat_header(id.as_str().unwrap()).unwrap();
    let w = a
        .chat_window(&serde_json::from_value(json!({"id":id})).unwrap(), &Default::default())
        .unwrap();
    let chip = &w["rows"][0]["attachments"][0];
    assert_eq!(chip["name"], "clip.txt");
    std::fs::remove_file(&file).unwrap();
    let bytes=a.chat_attachment(&serde_json::from_value(json!({"id":id,"publication":h["publication"],"sourceId":chip["sourceId"],"ordinal":chip["ordinal"],"key":chip["key"],"offset":2,"limit":4})).unwrap(),&Default::default()).unwrap();
    assert_eq!(serde_json::from_value::<Vec<u8>>(bytes["bytes"].clone()).unwrap(), b"tain");
    let op = &w["rows"][1]["operations"][0];
    assert_eq!(op["status"], "ok");
    assert_eq!(op["durationMs"], 75);
    assert_eq!(op["edit"]["diff"], "-old\n+new\n");
    let info = a.chat_info(id.as_str().unwrap()).unwrap();
    assert_eq!(info["own"]["messages"], 1);
    assert_eq!(info["total"]["addedLines"], 1);
    assert_eq!(info["total"]["removedLines"], 1);
    assert_eq!(info["estimatedUsd"], 0.000060);
}

#[test]
fn pi_prompt_versions_select_different_suffixes() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    let mut rows = vec![json!({"type":"session","version":3,"id":"demo","cwd":"/p"})];
    for (id, p, role, text) in [
        ("q", None, "user", "start"),
        ("a", Some("q"), "assistant", "base"),
        ("left", Some("a"), "user", "option one"),
        ("x", Some("left"), "assistant", "alpha"),
        ("right", Some("a"), "user", "option two"),
        ("y", Some("right"), "assistant", "beta"),
    ] {
        rows.push(fixtures::pi_message(id, p, role, text));
    }
    write(&root.join("sessions/tree.jsonl"), &fixtures::jsonl(&rows));
    capture(&a, agent_history::Harness::Pi, &root);
    let id = all(&a)[0]["id"].clone();
    let branches = a
        .chat_branches(
            &serde_json::from_value(json!({"id":id,"promptOrdinal":5})).unwrap(),
            &Default::default(),
        )
        .unwrap();
    assert_eq!(branches["versions"]["count"], 2);
    for (v, text) in branches["versions"]["items"].as_array().unwrap().iter().zip(["alpha", "beta"])
    {
        let w = a
            .chat_window(
                &serde_json::from_value(
                    json!({"id":id,"leaf":v["leaf"],"limit":1,"anchor":{"kind":"end"}}),
                )
                .unwrap(),
                &Default::default(),
            )
            .unwrap();
        assert_eq!(w["rows"][0]["parts"][0]["text"], text);
    }
}

#[test]
fn image_only_prompt_has_a_seekable_display_row() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    let mut image = fixtures::pi_message("u", None, "user", "");
    image["message"]["content"] =
        json!([{"type":"image","mimeType":"image/png","data":"iVBORw0KGgo="}]);
    write(
        &root.join("sessions/chat.jsonl"),
        &fixtures::jsonl(&[
            json!({"type":"session","version":3,"id":"pic"}),
            image,
            fixtures::pi_message("a", Some("u"), "assistant", "picture"),
        ]),
    );
    capture(&a, agent_history::Harness::Pi, &root);
    let id = all(&a)[0]["id"].clone();
    let h = a.chat_header(id.as_str().unwrap()).unwrap();
    assert_eq!(h["visibleRecords"], 2);
    for leaf in [None, Some(2)] {
        let read = |view: &str| {
            a.chat_window(
                &serde_json::from_value(json!({"id":id,"leaf":leaf,"view":view})).unwrap(),
                &Default::default(),
            )
            .unwrap()
        };
        assert_eq!(read("visible")["rows"][0]["attachments"][0]["bytes"], 8);
        assert_eq!(read("prompts")["rows"][0]["visibleRank"], 0);
        assert_eq!(
            read("raw")["rows"].as_array().unwrap().iter().find(|v| v["ordinal"] == 1).unwrap()["visibleRank"],
            0
        );
    }
}

#[test]
fn interrupted_work_has_a_notice_and_footer_without_an_assistant_reply() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("codex");
    let native_id = fixtures::SHARED_ID;
    native::codex_rollout(
        &root,
        "2026-11-10T12-00-00",
        native_id,
        &[
            native::codex_meta(native_id, "/p"),
            native::codex_user_at("2026-11-10T12:00:00.000Z", "stop soon"),
            json!({"timestamp":"2026-11-10T12:00:10.000Z","type":"event_msg","payload":{"type":"turn_aborted","turn_id":"turn-a"}}),
        ],
    );
    capture(&a, agent_history::Harness::Codex, &root);
    let response = a
        .chat_window(
            &serde_json::from_value(
                json!({"id":all(&a)[0]["id"],"anchor":{"kind":"end"},"limit":1}),
            )
            .unwrap(),
            &Default::default(),
        )
        .unwrap();
    let row = &response["rows"][0];
    assert_eq!(row["ordinal"], 2);
    assert_eq!(row["notices"][0]["code"], "interrupted");
    assert_eq!(row["footer"]["endReason"], "interrupted");
    assert_eq!(row["footer"]["facts"]["agentMs"], 10000);
    assert!(row["footer"]["facts"]["tokens"].is_null());
}

#[test]
fn reader_exports_complete_visible_text_and_byte_exact_originals_including_children() {
    use std::io::Read;
    let t = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("claude");
    let id = "9c2a2265-90e6-42ba-829c-538e664ff232";
    let cwd = "/project with a 'quote'";
    let answer = format!("<script>unsafe</script>{}END-MARKER", "A".repeat(90_000));
    let file = native::claude_main(
        &root,
        id,
        &[
            native::claude_user(id, "u", None, cwd, "Résumé input"),
            native::claude_reply(id, "a", "u", cwd, &answer),
        ],
    );
    let child = native::claude_subagent(&root, id, "agent-kid.jsonl", "kid", cwd, "child message");
    capture(&archive, agent_history::Harness::ClaudeCode, &root);
    let row = all(&archive).pop().unwrap();
    let conv = row["id"].as_str().unwrap();
    let header = archive.chat_header(conv).unwrap();
    let command = header["resumeCommand"].as_str().unwrap();
    assert!(command.contains("claude --resume") && command.ends_with(id));
    assert_eq!(header["resumeCommand"], archive.chat_info(conv).unwrap()["resumeCommand"]);
    assert_eq!(
        archive.palette_conversations("RÉSUMÉ", &Default::default()).unwrap()[0]["id"],
        conv
    );
    for format in ["html", "markdown", "jsonl", "originals"] {
        let q = serde_json::from_value(
            json!({"id":conv,"format":format,"publication":header["publication"]}),
        )
        .unwrap();
        let mut out = Vec::new();
        archive.export_conversation(&q, &mut out, true, true).unwrap();
        if format == "originals" {
            let mut files = tar::Archive::new(out.as_slice())
                .entries()
                .unwrap()
                .map(|e| {
                    let mut bytes = Vec::new();
                    e.unwrap().read_to_end(&mut bytes).unwrap();
                    bytes
                })
                .collect::<Vec<_>>();
            files.sort();
            let mut expected = vec![std::fs::read(&file).unwrap(), std::fs::read(&child).unwrap()];
            expected.sort();
            assert_eq!(files, expected);
            continue;
        }
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("END-MARKER"), "{format} clipped the reply");
        assert!(text.contains("Résumé input"));
        if format == "html" {
            assert!(text.contains("&lt;script&gt;"));
            assert!(!text.contains("<script>"));
        }
        if format == "jsonl" {
            assert!(text.lines().all(|l| serde_json::from_str::<Value>(l).is_ok()));
        }
    }
}

#[test]
fn codex_resume_is_bound_to_a_verified_cli_release_and_a_present_source() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("codex home");
    let id = "11111111-1111-4111-a111-111111111111";
    let unknown = "22222222-2222-4222-a222-222222222222";
    for (session, version) in [(id, "0.167.1"), (unknown, "unknown")] {
        let mut meta = native::codex_meta(session, "/work directory");
        meta["payload"]["cli_version"] = json!(version);
        let file =
            root.join(format!("sessions/2026/11/10/rollout-2026-11-10T12-00-00-{session}.jsonl"));
        write(&file, &fixtures::jsonl(&[meta, native::codex_user(session)]));
    }
    capture(&a, agent_history::Harness::Codex, &root);
    let mut known = None;
    for row in all(&a) {
        let conv = row["id"].as_str().unwrap();
        let header = a.chat_header(conv).unwrap();
        if header["nativeSessionId"] == unknown {
            assert!(header["resumeCommand"].is_null());
            continue;
        }
        assert_eq!(header["nativeSessionId"], id);
        let command = header["resumeCommand"].as_str().unwrap();
        assert!(command.contains("CODEX_HOME") && command.ends_with(&format!("codex resume {id}")));
        known = Some(conv.to_owned());
    }
    let saved = known.unwrap();
    std::fs::remove_file(
        root.join(format!("sessions/2026/11/10/rollout-2026-11-10T12-00-00-{id}.jsonl")),
    )
    .unwrap();
    capture(&a, agent_history::Harness::Codex, &root);
    let header = a.chat_header(&saved).unwrap();
    assert_eq!(header["summary"]["onlyInPathKeep"], true);
    assert!(header["resumeCommand"].is_null());
}

#[test]
fn embedding_inputs_and_semantic_hits_use_the_same_native_speaker_and_part() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let dir = t.path().join("claude");
    let id = "reasoning-example";
    let mut reply = native::claude_reply(id, "reply", "prompt", "/project", "visible answer");
    reply["message"]["content"] = json!([{"type":"thinking","thinking":"hidden reasoning"},{"type":"text","text":"visible answer"}]);
    native::claude_main(
        &dir,
        id,
        &[native::claude_user(id, "prompt", None, "/project", "question"), reply],
    );
    capture(&a, agent_history::Harness::ClaudeCode, &dir);
    let records = a.embedding_rows(0).unwrap();
    let record =
        records.iter().find(|r| r.chunks.iter().any(|c| c.text == "visible answer")).unwrap();
    let addresses =
        record.chunks.iter().enumerate().map(|(i, _)| (record.id, i as u32)).collect::<Vec<_>>();
    let q =
        serde_json::from_value(json!({"query":"reasoning","mode":"semantic","roles":["thinking"]}))
            .unwrap();
    let hits = a.semantic_page(&addresses, &q).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].1["role"], "thinking");
    assert_eq!(hits[0].1["text"], "hidden reasoning");
}

#[path = "chat_screens/capture_fixtures.rs"]
mod capture_fixtures;
