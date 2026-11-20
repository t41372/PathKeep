//! Real owner, captured Pi transcripts, stdio-owner tool calls and loopback provider transport.
//! Private markers distinguish an authorized passage from unrelated transcript or native metadata.
#[allow(dead_code)]
#[path = "../../agent-history/tests/support/fixtures.rs"]
mod fixtures;
#[path = "agent_ai/paging.rs"]
mod paging;
#[path = "agent_ai/provider.rs"]
mod provider;
#[allow(dead_code)]
#[path = "root_admission/support.rs"]
mod support;
use agent_history::{Harness, SourceRoot};
use serde_json::{Value, from_value, json};
use std::{
    path::Path,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use vault_core::{AiProviderConfig, AiProviderPurpose, ControlGate, load_config, save_config};
use vault_worker::data_kinds::{self as kinds, semantic};
fn write(path: &Path, data: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, data).unwrap();
}
fn events() -> kinds::backups::Events {
    Arc::new(|_, _| {})
}
fn prefs(f: &support::Fixture, v: Value) {
    kinds::viewer::set(&f.session, from_value(v).unwrap(), events()).unwrap();
}
fn configure(f: &support::Fixture, p: &provider::Provider) {
    let paths = f.paths();
    let mut c = load_config(&paths).unwrap();
    c.ai.enabled = true;
    c.ai.assistant_enabled = true;
    c.ai.semantic_index_enabled = true;
    c.ai.mcp_enabled = true;
    let provider = |id: &str, purpose| AiProviderConfig {
        id: id.into(),
        purpose,
        enabled: true,
        default_model: "fixture".into(),
        base_url: Some(p.url.clone()),
        ..Default::default()
    };
    c.ai.llm_providers = vec![provider("llm", AiProviderPurpose::Llm)];
    c.ai.llm_provider_id = Some("llm".into());
    c.ai.embedding_providers = vec![provider("embed", AiProviderPurpose::Embedding)];
    c.ai.embedding_provider_id = Some("embed".into());
    save_config(&ControlGate::acquire(&paths), &c).unwrap();
    let root = f.temp.path().join("pi");
    for (name, text) in
        [("one", "A kitten purred. CITED_MARKER"), ("two", "UNRELATED_MARKER network firewall")]
    {
        let text = if name == "one" {
            format!("{text} {} TAIL_PRIVATE_DETAILS", "long result ".repeat(1400))
        } else {
            text.to_owned()
        };
        let rows = [
            json!({"type":"session","version":3,"id":name,"timestamp":"2026-11-10T12:00:00Z","cwd":"/PRIVATE_PATH_MARKER"}),
            fixtures::pi_message("u", None, "user", &text),
        ];
        write(&root.join(format!("sessions/{name}.jsonl")), &fixtures::jsonl(&rows));
    }
    f.vault()
        .agent_history()
        .unwrap()
        .backup(&[SourceRoot { harness: Harness::Pi, root }], &Default::default())
        .unwrap();
}
fn save_reviewed_pi(f: &support::Fixture, native: &Path) {
    let mut preview = kinds::preview::start(
        &f.session,
        from_value(json!({"purpose":"enable","pickedFolder":{"tool":"pi","path":native}})).unwrap(),
    )
    .unwrap();
    let approved =
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(
            async {
                loop {
                    let frame = preview.next_frame().await.unwrap();
                    assert_ne!(frame["type"], "error", "{frame}");
                    if frame["type"] == "finished" {
                        break frame["payload"].clone();
                    }
                }
            },
        );
    let (tx, rx) = mpsc::channel();
    kinds::backups::enable(&f.session,from_value(json!({"previewId":approved["previewId"],"selectedRootIds":[approved["rows"][0]["rootId"]],
        "readGit":false,"expectedPolicyToken":kinds::settings().unwrap().policy_token,"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(move|name,event|{if name=="pathkeep://kind-backup-finished"{tx.send(event).unwrap();}})).unwrap();
    rx.recv_timeout(Duration::from_secs(30)).unwrap();
}
fn ask(f: &support::Fixture) -> Vec<Value> {
    send(
        f,
        json!({"messages":[{"role":"user","content":"What did I save about pets?"}],"toolsEnabled":true}),
    )
}
fn send(f: &support::Fixture, request: Value) -> Vec<Value> {
    let (tx, rx) = mpsc::channel();
    let q = from_value(request).unwrap();
    vault_worker::ai_chat_send(f.vault(), None, &q, move |e| {
        tx.send(serde_json::to_value(e).unwrap()).unwrap();
    })
    .unwrap();
    let mut frames = Vec::new();
    loop {
        let e = rx.recv_timeout(Duration::from_secs(30)).unwrap();
        let kind = e["chunk"]["kind"].clone();
        frames.push(e);
        assert_ne!(kind, "error", "{frames:?}");
        if kind == "done" {
            break;
        }
    }
    frames
}
#[test]
fn provider_receives_only_the_independently_authorized_content() {
    let f = support::Fixture::new(true);
    let p = provider::Provider::new();
    configure(&f, &p);
    assert!(
        semantic::control(&f.session, from_value(json!({"action":"build"})).unwrap(), events())
            .is_err()
    );
    assert!(
        kinds::chats::search(
            &f.session,
            from_value(json!({"query":"kitten","mode":"semantic"})).unwrap()
        )
        .is_err()
    );
    assert!(p.requests.lock().unwrap().is_empty());
    ask(&f);
    let off = std::mem::take(&mut *p.requests.lock().unwrap());
    assert!(off.iter().any(|(path, _)| path.ends_with("/chat/completions")));
    assert!(!json!(off).to_string().contains("CITED_MARKER"));
    assert!(!json!(off).to_string().contains("search_agent_conversations"));
    prefs(&f, json!({"assistantMayRead":true}));
    let frames = ask(&f);
    let sent = std::mem::take(&mut *p.requests.lock().unwrap());
    let wire = json!(sent).to_string();
    assert!(
        wire.contains("CITED_MARKER")
            && !wire.contains("UNRELATED_MARKER")
            && !wire.contains("PRIVATE_PATH_MARKER")
            && !wire.contains("TAIL_PRIVATE_DETAILS"),
        "{wire}"
    );
    assert!(json!(frames).to_string().contains("pathkeep://conversation/open"));
    assert!(!sent.iter().any(|(path, _)| path.ends_with("/embeddings")));
    prefs(&f, json!({"assistantMayRead":false,"buildSemanticIndex":true}));
    let start = Instant::now();
    loop {
        let state = semantic::status(&f.session, from_value(json!({})).unwrap()).unwrap();
        if state["job"]["state"] == "ready" {
            break;
        }
        assert_ne!(state["job"]["state"], "failed", "{state}");
        assert!(start.elapsed() < Duration::from_secs(45), "{state}");
        std::thread::sleep(Duration::from_millis(30));
    }
    let mut stream = kinds::chats::search(
        &f.session,
        from_value(json!({"mode":"semantic","query":"kitten"})).unwrap(),
    )
    .unwrap();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let mut hits = vec![];
        while let Some(e) = stream.next_frame().await {
            assert_ne!(e["type"], "error", "{e}");
            if e["type"] == "passages" {
                hits.extend(e["payload"]["items"].as_array().unwrap().clone());
            }
        }
        assert!(hits.iter().any(|h| h["text"].as_str().unwrap().contains("UNRELATED_MARKER")));
        assert!(hits[0]["text"].as_str().unwrap().contains("CITED_MARKER"));
    });
    let sent = std::mem::take(&mut *p.requests.lock().unwrap());
    assert!(sent.iter().all(|(path, _)| path.ends_with("/embeddings")));
    assert!(json!(sent).to_string().contains("UNRELATED_MARKER"));
    assert!(!json!(sent).to_string().contains("PRIVATE_PATH_MARKER"));
    // A real enable-and-save updates the index. Only the newly added document goes to the service.
    let native = f.temp.path().join("pi");
    write(
        &native.join("sessions/new.jsonl"),
        &fixtures::jsonl(&[
            json!({"type":"session","version":3,"id":"new","timestamp":"2026-11-10T12:00:00Z"}),
            fixtures::pi_message("u", None, "user", "ADDED_AFTER_PUBLICATION"),
        ]),
    );
    let before =
        semantic::status(&f.session, from_value(json!({})).unwrap()).unwrap()["serving"].clone();
    save_reviewed_pi(&f, &native);
    let since = Instant::now();
    loop {
        let current = semantic::status(&f.session, from_value(json!({})).unwrap()).unwrap();
        if current["job"]["state"] == "ready"
            && current["serving"]["generation"] != before["generation"]
        {
            assert_eq!(current["serving"]["reusedVectors"], before["vectors"]);
            break;
        }
        assert!(since.elapsed() < Duration::from_secs(45), "{current}");
        std::thread::sleep(Duration::from_millis(30));
    }
    let update = std::mem::take(&mut *p.requests.lock().unwrap());
    assert_eq!(update.len(), 1, "{update:?}");
    assert!(update[0].1["input"].to_string().contains("ADDED_AFTER_PUBLICATION"));
    assert!(!update[0].1["input"].to_string().contains("CITED_MARKER"));
    prefs(&f, json!({"buildSemanticIndex":false,"mcpMayRead":true}));
    assert!(
        kinds::chats::search(
            &f.session,
            from_value(json!({"mode":"semantic","query":"kitten"})).unwrap()
        )
        .is_err()
    );
    assert!(p.requests.lock().unwrap().is_empty());
}

#[test]
fn mcp_checks_each_switch_and_lock_on_the_same_stdio_connection() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
    };
    let f = support::Fixture::new(true);
    let provider = provider::Provider::new();
    configure(&f, &provider);
    let session = f.session.clone();
    let _owner = vault_owner_ipc::Server::start(&f.root, move |request, progress| {
        vault_worker::owner_requests::dispatch_with_session(request, progress, &session)
    })
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_pathkeep-worker"))
        .arg("mcp-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut input = input;
    let mut exchange = |q: Value| {
        writeln!(input, "{q}").unwrap();
        input.flush().unwrap();
        if q.get("id").is_none() {
            return Value::Null;
        }
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    assert!(exchange(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}}))["result"].is_object());
    exchange(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let list = exchange(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}));
    for name in ["search-conversations", "read-conversation"] {
        assert!(list["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == name));
    }
    let mut call = |name: &str, args: Value| {
        exchange(
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":name,"arguments":args}}),
        )
    };
    let refused = |v: &Value| v.get("error").is_some() || v["result"]["isError"] == true;
    assert!(refused(&call("search-conversations", json!({"query":"kitten"}))));
    prefs(&f, json!({"assistantMayRead":true}));
    assert!(refused(&call("search-conversations", json!({"query":"kitten"}))));
    prefs(&f, json!({"assistantMayRead":false,"mcpMayRead":true}));
    let searched = call("search-conversations", json!({"query":"kitten","limit":1}));
    assert!(!refused(&searched), "{searched}");
    let body: Value =
        serde_json::from_str(searched["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let id = body["items"][0]["conversationId"].as_str().unwrap();
    let read = call("read-conversation", json!({"id":id,"limit":1}));
    assert!(!refused(&read) && read.to_string().contains("CITED_MARKER"), "{read}");
    assert!(!read.to_string().contains("UNRELATED_MARKER"));
    let page: Value =
        serde_json::from_str(read["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let record = &page["rows"][0];
    let tail = call(
        "read-conversation",
        json!({"id":id,"publication":page["publication"],"record":{"sourceId":record["sourceId"],"ordinal":record["ordinal"],"chunk":1},"limit":50}),
    );
    assert!(tail.to_string().contains("TAIL_PRIVATE_DETAILS"), "{tail}");
    let audits = f
        .vault()
        .store()
        .read(vault_store::Db::Archive, |s| {
            s.query("SELECT stats_json FROM runs WHERE run_type='mcp_query' ORDER BY id", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })
        .unwrap();
    assert!(
        audits.iter().any(|s| s.contains("search-conversations"))
            && audits.iter().any(|s| s.contains("read-conversation"))
    );
    assert!(!audits.join("").contains("CITED_MARKER"));
    prefs(&f, json!({"mcpMayRead":false}));
    assert!(refused(&call("read-conversation", json!({"id":id}))));
    prefs(&f, json!({"mcpMayRead":true}));
    let paths = f.paths();
    let mut cfg = load_config(&paths).unwrap();
    cfg.app_lock.enabled = true;
    save_config(&ControlGate::acquire(&paths), &cfg).unwrap();
    vault_core::lock_app_session(&paths, &cfg, Some("manual")).unwrap();
    assert!(refused(&call("search-conversations", json!({"query":"kitten"}))));
    assert!(refused(&call("read-conversation", json!({"id":id}))));
    assert!(provider.requests.lock().unwrap().is_empty());
    drop(input);
    let _ = child.wait().unwrap();
}

#[test]
fn encrypted_build_can_pause_and_cancel_while_the_previous_generation_serves() {
    let f = support::Fixture::encrypted();
    let p = provider::Provider::new();
    configure(&f, &p);
    prefs(&f, json!({"buildSemanticIndex":true}));
    let status = || semantic::status(&f.session, from_value(json!({})).unwrap()).unwrap();
    let wait = |phase: &str| {
        let now = Instant::now();
        loop {
            let s = status();
            if s["job"]["state"] == phase {
                return s;
            }
            assert!(now.elapsed() < Duration::from_secs(45), "{s}");
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    let first = wait("ready");
    f.vault().agent_history().unwrap().rebuild_search().unwrap();
    assert_eq!(status()["serving"], first["serving"]);
    let root =
        f.paths().artifact_path(vault_layout::ArtifactId::AgentHistorySearch).join("semantic");
    assert_eq!(&std::fs::read(root.join("current")).unwrap()[..4], b"PKSI");
    for entry in
        std::fs::read_dir(root.join(first["serving"]["generation"].as_str().unwrap())).unwrap()
    {
        let path = entry.unwrap().path();
        assert_eq!(&std::fs::read(path).unwrap()[..4], b"PKSI");
    }
    let initial = p.requests.lock().unwrap().len();
    let session = f.session.clone();
    let held = std::sync::atomic::AtomicBool::new(false);
    let row = semantic::control(
        &f.session,
        from_value(json!({"action":"build"})).unwrap(),
        Arc::new(move |_, r| {
            if r["state"] == "running" && !held.swap(true, std::sync::atomic::Ordering::AcqRel) {
                semantic::control(
                    &session,
                    from_value(json!({"action":"pause","jobId":r["jobId"]})).unwrap(),
                    events(),
                )
                .unwrap();
            }
        }),
    )
    .unwrap();
    assert!(
        vault_worker::foreground_work().contains(&vault_worker::ForegroundWork::AgentSemanticIndex)
    );
    assert_eq!(wait("paused")["serving"], first["serving"]);
    let mut search = kinds::chats::search(
        &f.session,
        from_value(json!({"mode":"semantic","query":"kitten"})).unwrap(),
    )
    .unwrap();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let mut matches = 0;
        while let Some(frame) = search.next_frame().await {
            assert_ne!(frame["type"], "error", "{frame}");
            if frame["type"] == "passages" {
                matches += frame["payload"]["items"].as_array().unwrap().len();
            }
        }
        assert_eq!(matches as u64, first["serving"]["passages"].as_u64().unwrap());
    });
    semantic::control(
        &f.session,
        from_value(json!({"action":"cancel","jobId":row["jobId"]})).unwrap(),
        events(),
    )
    .unwrap();
    assert_eq!(wait("cancelled")["serving"], first["serving"]);
    assert!(
        !vault_worker::foreground_work()
            .contains(&vault_worker::ForegroundWork::AgentSemanticIndex)
    );
    assert_eq!(p.requests.lock().unwrap().len(), initial + 1);
    let empty = || from_value(json!({})).unwrap();
    prefs(&f, json!({"buildSemanticIndex":false}));
    let off = status();
    assert!(off["serving"].is_null());
    let expected = std::fs::metadata(root.join("current")).unwrap().len()
        + std::fs::read_dir(root.join(first["serving"]["generation"].as_str().unwrap()))
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len())
            .sum::<u64>();
    assert_eq!(off["keptBytes"], expected.to_string());
    let preview = semantic::preview_free(&f.session, empty()).unwrap();
    let request =
        |yes: bool| from_value(json!({"previewId":preview["previewId"],"confirm":yes})).unwrap();
    assert!(semantic::free(&f.session, request(false)).is_err());
    assert_eq!(preview["keptBytes"], status()["keptBytes"]);
    let freed = semantic::free(&f.session, request(true)).unwrap();
    assert_eq!(freed["freedBytes"], preview["keptBytes"]);
    assert!(semantic::free(&f.session, request(true)).is_err());
    assert_eq!(status()["keptBytes"], "0");
    assert!(!root.join("current").exists());
    assert!(
        f.vault()
            .agent_history()
            .unwrap()
            .search_page(
                &agent_history::SearchQuery {
                    text: "kitten".into(),
                    harness: None,
                    project_path: None,
                    limit: 1
                },
                0
            )
            .unwrap()
            .total
            > 0
    );
    assert_eq!(p.requests.lock().unwrap().len(), initial + 1);
}

#[test]
fn a_saved_citation_reopens_at_the_record_and_revocation_excludes_its_reply_from_replay() {
    let f = support::Fixture::new(true);
    let p = provider::Provider::new();
    configure(&f, &p);
    f.vault().store().write(vault_store::Db::Agent,vault_store::Priority::Interactive,|s|{
        s.execute("INSERT INTO conversations(id,title,created_at,updated_at,message_count) VALUES('chat','pets','2026-11-10','2026-11-10',1)",())?;
        s.execute("INSERT INTO messages(id,conversation_id,seq,role,content,created_at) VALUES('answer','chat',1,'assistant','CITED_MARKER reply','2026-11-10')",())?;
        Ok(())
    }).unwrap();
    prefs(&f, json!({"assistantMayRead":true}));
    send(
        &f,
        json!({"conversationId":"chat","messageId":"answer","messages":[{"role":"user","content":"find my pet"}],"toolsEnabled":true}),
    );
    let saved = vault_core::load_conversation(f.vault(), "chat").unwrap().unwrap();
    let encoded = serde_json::to_string(&saved).unwrap();
    assert!(
        encoded.contains(r#""kind":"conversation""#) && encoded.contains("sourceId"),
        "{encoded}"
    );
    let citation = saved.messages.iter().flat_map(|m| &m.citations).next().unwrap();
    let vault_core::ConversationCitation::Conversation { conversation_id, .. } =
        citation.target.as_ref().unwrap();
    kinds::chats::edit(
        &f.session,
        from_value(json!({"action":"rename","id":conversation_id,"title":"[Feline] care"}))
            .unwrap(),
        events(),
    )
    .unwrap();
    f.vault().store().write(vault_store::Db::Agent,vault_store::Priority::Interactive,|s|{
        s.execute("INSERT INTO agent_citations(run_id,history_id,canonical_url,url,title) SELECT run_id,999,'https://fixture.test','https://fixture.test','Reference' FROM agent_citations LIMIT 1",())?;
        Ok(())
    }).unwrap();
    for (label, expected) in [(None, "Conversations used"), (Some("引用的對話"), "引用的對話")]
    {
        let request=from_value(json!({"id":"chat","labels":{"untitled":"Untitled","you":"You","assistant":"Assistant","pagesUsed":"Pages used","conversationsUsed":label}})).unwrap();
        let md = vault_core::conversation_markdown(f.vault(), &request).unwrap().unwrap().markdown;
        assert!(md.contains("Pages used:\n\n1. [Reference](<https://fixture.test>)"), "{md}");
        assert!(md.contains(&format!("{expected}:\n\n1. \\[Feline\\] care")), "{md}");
        assert!(!md.contains("pathkeep://"), "{md}");
    }
    prefs(&f, json!({"assistantMayRead":false}));
    p.requests.lock().unwrap().clear();
    send(
        &f,
        json!({"conversationId":"chat","messages":[{"role":"assistant","content":"CITED_MARKER reply"},{"role":"user","content":"follow-up"}]}),
    );
    let requests = p.requests.lock().unwrap();
    let wire = json!(*requests).to_string();
    assert!(wire.contains("follow-up") && !wire.contains("CITED_MARKER"), "{wire}");
}

#[test]
fn a_first_save_with_all_three_grants_off_never_contacts_the_configured_provider() {
    let f = support::Fixture::new(true);
    let p = provider::Provider::new();
    configure(&f, &p);
    save_reviewed_pi(&f, &f.temp.path().join("pi"));
    assert!(p.requests.lock().unwrap().is_empty());
    let status = semantic::status(&f.session, from_value(json!({})).unwrap()).unwrap();
    assert_eq!(status, json!({"enabled":false,"serving":null,"job":null,"keptBytes":"0"}));
    assert!(
        !f.paths()
            .artifact_path(vault_layout::ArtifactId::AgentHistorySearch)
            .join("semantic")
            .exists()
    );
}
