//! Exercise both screen streams through a captured Claude transcript and its subagent.
#[allow(dead_code)]
mod support;
use agent_history::{Archive, Harness, SourceRoot, capture::BackupOptions, search::ScreenQuery};
use serde_json::{Value, json};
use support::native::*;
use vault_store::CancelToken;

#[test]
fn search_and_find_use_the_same_native_parts_and_addresses() {
    let tmp = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&tmp.path().join("archive")).unwrap();
    let root = tmp.path().join("claude");
    let cwd = "/project";
    claude_main(
        &root,
        "parent",
        &[
            claude_user("parent", "u", None, cwd, "go away go"),
            claude_reply("parent", "a", "u", cwd, "甲🙂 **NEEDLE** go go"),
            claude_tool_call("parent", "t", "a", cwd, json!({"command":"needle argument"})),
            json!({"type":"assistant","uuid":"reason","parentUuid":"t","sessionId":"parent","timestamp":TIME,
                "message":{"role":"assistant","content":[{"type":"thinking","thinking":"bluecipher"},{"type":"text","text":"redcipher"}]}}),
            claude_title("parent", "Custom umbrella"),
        ],
    );
    claude_subagent(&root, "parent", "agent-child.jsonl", "child", cwd, "descendant needle");
    a.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &BackupOptions::default())
        .unwrap();
    let search = |q: Value, scope: Option<&str>| {
        let request: ScreenQuery = serde_json::from_value(q).unwrap();
        let mut frames = Vec::new();
        a.stream_conversations(scope, &request, &CancelToken::new(), |ty, p| {
            frames.push((ty.to_owned(), p));
            Ok(())
        })
        .unwrap();
        frames
    };
    let frames = search(json!({"query":"needle"}), None);
    assert_eq!(frames.last().unwrap().1["hits"], 4);
    assert_eq!(frames.last().unwrap().1["conversations"], 1);
    let group = &frames.iter().rev().find(|(ty, _)| ty == "groups").unwrap().1["items"][0];
    let id = group["id"].as_str().unwrap();
    let hits = search(json!({"query":"/nEe[d]+le/"}), Some(id));
    let matches: Vec<_> = hits
        .iter()
        .filter(|(ty, _)| ty == "matches")
        .flat_map(|(_, p)| p["items"].as_array().unwrap())
        .collect();
    assert_eq!(matches.len(), 4);
    let child_title = matches.iter().find(|hit| hit["role"] == "title").unwrap();
    assert!(child_title["subagentId"].is_string());
    assert_eq!(child_title["range"], json!([11, 17]));
    assert!(matches.iter().any(|m| m["subagentId"].is_string() && m["role"] == "user"));
    let assistant = matches.iter().find(|m| m["role"] == "agent").unwrap();
    assert_eq!(assistant["range"], json!([8, 14]));
    assert_eq!(assistant["address"]["nativeId"], "a");
    for (q, count) in [
        (json!({"query":"\"go go\""}), 1),
        (json!({"query":"needle","roles":["tool"]}), 2),
        (json!({"query":"umbrella","roles":["thinking"]}), 1),
        (json!({"query":"go absent"}), 0),
        (json!({"query":"bluecipher redcipher"}), 2),
        (json!({"query":"redcipher bluecipher","roles":["agent"]}), 0),
        (json!({"query":"bluecipher","roles":["thinking"]}), 1),
        (
            json!({"query":"needle","filters":{"projectId":"/project","day":"2026-11-10","timeZone":"UTC"}}),
            4,
        ),
        (json!({"query":"needle","filters":{"day":"2026-11-09","timeZone":"UTC"}}), 0),
    ] {
        assert_eq!(search(q, None).last().unwrap().1["hits"], count);
    }
    use agent_history::archive::ConversationMark;
    a.mark_conversation(
        id,
        &ConversationMark {
            title: Some(Some("Renamed cloud".into())),
            put_away: Some(true),
            ..Default::default()
        },
        true,
    )
    .unwrap();
    let renamed = search(json!({"query":"cloud","roles":["thinking"]}), None);
    let card = &renamed.iter().find(|(ty, _)| ty == "groups").unwrap().1["items"][0];
    assert_eq!(card["id"], id);
    assert_eq!(card["archived"], true);
    assert_eq!(card["titleHit"], true);
    assert_eq!(card["snippets"], json!([]));
    assert_eq!(
        search(json!({"query":"cloud","filters":{"harnesses":["pi"]}}), None).last().unwrap().1["searchedCount"],
        0
    );
    let token = CancelToken::new();
    let re = serde_json::from_value(json!({"query":"/.*/"})).unwrap();
    let mut terminal = false;
    let err = a
        .stream_conversations(None, &re, &token, |ty, _| {
            assert_ne!(ty, "finished");
            terminal = true;
            token.cancel();
            Ok(())
        })
        .unwrap_err();
    assert!(terminal);
    assert_eq!(err.code(), "cancelled");
    let bad = serde_json::from_value(json!({"query":"/[/"})).unwrap();
    assert_eq!(
        a.stream_conversations(None, &bad, &CancelToken::new(), |_, _| Ok(())).unwrap_err().code(),
        "invalid_regex"
    );
}

#[test]
fn global_search_keeps_the_last_commit_during_backup_and_invalidates_a_trashed_card() {
    use agent_history::archive::ConversationMark;
    use std::sync::Arc;
    let tmp = tempfile::tempdir().unwrap();
    let archive = Arc::new(Archive::create_unencrypted(&tmp.path().join("archive")).unwrap());
    let root = tmp.path().join("claude");
    let source = SourceRoot { harness: Harness::ClaudeCode, root: root.clone() };
    let add = |n| {
        let id = format!("00000000-0000-4000-8000-{n:012}");
        let title = if n == 130 { "amber needle" } else { "Experiment" };
        let text = if n < 128 || n == 131 { "amber needle" } else { "question" };
        let mut rows = vec![claude_user(&id, "u", None, "/bench", text), claude_title(&id, title)];
        if n == 128 {
            rows.push(claude_tool_call(&id, "t", "u", "/bench", json!({"cmd":"amber needle"})))
        }
        if n == 129 {
            claude_subagent(&root, &id, "agent-child.jsonl", "child", "/bench", "amber needle");
        }
        claude_main(&root, &id, &rows);
    };
    for n in 0..131 {
        add(n);
    }
    archive.backup(std::slice::from_ref(&source), &BackupOptions::default()).unwrap();
    fn count(a: &Archive, q: ScreenQuery) -> Value {
        let mut result = None;
        a.stream_conversations(None, &q, &CancelToken::new(), |ty, p| {
            if ty == "finished" {
                result = Some(p)
            }
            Ok(())
        })
        .unwrap();
        result.unwrap()
    }
    let query = || ScreenQuery::all("\"amber needle\"".into());
    let scoped = count(
        &archive,
        serde_json::from_value(
            json!({"query":"\"amber needle\"","roles":["agent","user","thinking"]}),
        )
        .unwrap(),
    );
    // The child contributes both its assignment and the title derived from it.
    assert_eq!(scoped["hits"], 131);
    add(131);
    let previous = archive.clone();
    let observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = observed.clone();
    archive
        .backup(
            &[source],
            &BackupOptions {
                progress: Some(Arc::new(move |p| {
                    if p.phase == "index" {
                        let end = count(&previous, query());
                        assert_eq!(end["hits"], 132);
                        assert_eq!(end["conversations"], 131);
                        assert_eq!(end["indexLag"], 1);
                        flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                })),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(observed.load(std::sync::atomic::Ordering::SeqCst));
    let end = count(&archive, query());
    assert_eq!(end["hits"], 133);
    assert_eq!(end["indexLag"], 0);
    let mut marked = false;
    let error = archive
        .stream_conversations(None, &query(), &CancelToken::new(), |ty, p| {
            if ty == "groups" && !marked {
                archive.mark_conversation(
                    p["items"][0]["id"].as_str().unwrap(),
                    &ConversationMark { trash: Some(true), ..Default::default() },
                    true,
                )?;
                marked = true;
            }
            assert_ne!(ty, "finished");
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code(), "stale_cursor");
    assert_eq!(count(&archive, query())["conversations"], 131);
    assert_eq!(
        count(
            &archive,
            serde_json::from_value(
                json!({"query":"\"amber needle\"","filters":{"flags":["trash"]}})
            )
            .unwrap()
        )["hits"],
        1
    );
}

#[test]
fn search_uses_current_deepseek_generation_and_codex_tool_output() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let deep = t.path().join("deep");
    deepseek_generations(
        &deep,
        "11111111-1111-4111-8111-111111111111",
        "/bench",
        "generationneedle",
    );
    let codex = t.path().join("codex");
    let thread = "22222222-2222-4222-8222-222222222222";
    codex_rollout(
        &codex,
        "2026-11-10T12-00-00",
        thread,
        &[
            codex_meta(thread, "/bench"),
            codex_user("test command"),
            json!({"timestamp":TIME,"type":"response_item","payload":{"type":"function_call_output","call_id":"c","output":"a tool's ToolNeedle"}}),
        ],
    );
    a.backup(
        &[
            SourceRoot { harness: Harness::Deepseek, root: deep },
            SourceRoot { harness: Harness::Codex, root: codex },
        ],
        &Default::default(),
    )
    .unwrap();
    for (word, role, harness) in
        [("generationneedle", "user", "deepseek"), ("toolneedle", "tool", "codex")]
    {
        let q = serde_json::from_value(
            json!({"query":word,"roles":[role],"filters":{"harnesses":[harness]}}),
        )
        .unwrap();
        let mut body = None;
        a.stream_conversations(None, &q, &CancelToken::new(), |ty, p| {
            if ty == "groups" {
                let card = &p["items"][0];
                assert_eq!(card["harness"], harness);
                body = Some(card["snippets"].as_array().unwrap().len());
                assert_eq!(card["snippets"][0]["role"], role);
                assert_eq!(card["hits"].as_u64().unwrap(), 1 + u64::from(card["titleHit"] == true));
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(body, Some(1), "exactly one producing generation for {word}");
    }
}

#[test]
fn an_open_stream_keeps_its_snippets_while_find_reads_the_new_revision() {
    use std::sync::Arc;
    let tmp = tempfile::tempdir().unwrap();
    let a = Arc::new(Archive::create_unencrypted(&tmp.path().join("archive")).unwrap());
    let source = SourceRoot { harness: Harness::ClaudeCode, root: tmp.path().join("claude") };
    let write = |body: &str| {
        claude_main(
            &source.root,
            "switch",
            &[
                claude_user("switch", "u", None, "/bench", "question"),
                claude_reply("switch", "a", "u", "/bench", body),
                claude_title("switch", "Pinned card"),
            ],
        )
    };
    write("beforecommit");
    a.backup(std::slice::from_ref(&source), &Default::default()).unwrap();
    let id =
        a.chat_list(&Default::default()).unwrap()["items"][0]["id"].as_str().unwrap().to_owned();
    let mut checked = false;
    a.stream_conversations(
        None,
        &ScreenQuery::all("beforecommit".into()),
        &CancelToken::new(),
        |ty, p| {
            if ty == "started" {
                write("aftercommit");
                let owner = a.clone();
                let id = id.clone();
                a.backup(
                    std::slice::from_ref(&source),
                    &BackupOptions {
                        progress: Some(Arc::new(move |p| {
                            if p.phase == "index" {
                                let mut matched = false;
                                owner
                                    .stream_conversations(
                                        Some(&id),
                                        &ScreenQuery::all("aftercommit".into()),
                                        &Default::default(),
                                        |ty, p| {
                                            if ty == "finished" {
                                                assert_eq!(p["hits"], 1);
                                                matched = true
                                            }
                                            Ok(())
                                        },
                                    )
                                    .unwrap();
                                assert!(matched);
                            }
                        })),
                        ..Default::default()
                    },
                )?;
            } else if ty == "groups" {
                assert_eq!(p["items"][0]["snippets"][0]["text"], "beforecommit");
                checked = true;
            } else if ty == "finished" {
                assert_eq!(p["hits"], 1);
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(checked);
}

#[test]
fn a_pi_hit_outside_the_default_leaf_opens_at_its_record() {
    use support::fixtures::{SHARED_ID, jsonl, pi_message};
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    let sessions = root.join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(
        sessions.join("tree.jsonl"),
        jsonl(&[
            json!({"type":"session","version":3,"id":SHARED_ID,"timestamp":TIME,"cwd":"/fixture"}),
            pi_message("prompt", None, "user", "question"),
            pi_message("old", Some("prompt"), "assistant", "earlierneedle"),
            pi_message("new", Some("prompt"), "assistant", "current answer"),
        ]),
    )
    .unwrap();
    a.backup(&[SourceRoot { harness: Harness::Pi, root }], &Default::default()).unwrap();
    let mut address = None;
    a.stream_conversations(
        None,
        &ScreenQuery::all("earlierneedle".into()),
        &Default::default(),
        |ty, p| {
            if ty == "groups" {
                address = Some(p["items"][0]["snippets"][0]["address"].clone());
            }
            Ok(())
        },
    )
    .unwrap();
    let address = address.unwrap();
    assert_eq!(address["nativeId"], "old");
    let window=serde_json::from_value(json!({"id":address["conversationId"],"publication":address["publication"],"branch":address["sourceId"],"leaf":address["ordinal"],
        "anchor":{"kind":"record","sourceId":address["sourceId"],"ordinal":address["ordinal"]},"before":0,"limit":1})).unwrap();
    let page = a.chat_window(&window, &Default::default()).unwrap();
    assert_eq!(page["rows"][0]["parts"][0]["text"], "earlierneedle");
}
