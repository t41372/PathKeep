//! Responsibilities: preserve transcript CRUD and timestamp behaviour.
//! Not responsible for: provider execution or schema migrations.
//! Failure modes: capped lists, replaced birth times, partial deletion and lost messages.

use super::*;

#[test]
fn vault_open_installs_the_agent_schema() {
    let vault = Vault::open_temp().unwrap();
    let count: i64 = query(&vault, "SELECT COUNT(*) FROM conversations", (), |row| Ok(row.get(0)?))
        .expect("query conversations");
    assert_eq!(count, 0);
    assert!(vault.paths().agent_database_path.exists());
}

#[test]
fn save_then_load_roundtrips_messages_in_order() {
    let vault = Vault::open_temp().unwrap();
    let request = SaveAgentConversationRequest {
        id: "conv-1".to_string(),
        title: None,
        provider_id: Some("llm-local".to_string()),
        messages: vec![
            user_message("m1", "when did I read about tauri?"),
            assistant_message("m2", "You read about Tauri in April."),
        ],
    };
    let summary = save_conversation(&vault, &request).expect("save");
    assert_eq!(summary.message_count, 2);
    // Auto-title is derived from the first user message.
    assert_eq!(summary.title, "when did I read about tauri?");
    assert_eq!(summary.provider_id.as_deref(), Some("llm-local"));

    let detail = load_conversation(&vault, "conv-1").expect("load").expect("present");
    assert_eq!(detail.summary.id, "conv-1");
    assert_eq!(detail.messages.len(), 2);
    assert_eq!(detail.messages[0].id, "m1");
    assert_eq!(detail.messages[0].role, "user");
    assert_eq!(detail.messages[1].id, "m2");
    assert_eq!(detail.messages[1].reasoning.as_deref(), Some("thinking"));
    assert_eq!(detail.messages[1].status.as_deref(), Some("done"));
    assert!(detail.messages[1].tool_calls_json.as_deref().unwrap().contains("search_bm25"));
}

#[test]
fn explicit_title_overrides_auto_title_and_save_replaces_transcript() {
    let vault = Vault::open_temp().unwrap();
    let first = SaveAgentConversationRequest {
        id: "conv-2".to_string(),
        title: Some("  Custom title  ".to_string()),
        provider_id: None,
        messages: vec![user_message("m1", "first"), assistant_message("m2", "answer one")],
    };
    let saved = save_conversation(&vault, &first).expect("first save");
    assert_eq!(saved.title, "Custom title");
    let created_at = saved.created_at.clone();

    // Re-save with a longer transcript: it replaces messages and preserves created_at.
    let second = SaveAgentConversationRequest {
        id: "conv-2".to_string(),
        title: Some("Custom title".to_string()),
        provider_id: None,
        messages: vec![
            user_message("m1", "first"),
            assistant_message("m2", "answer one"),
            user_message("m3", "second"),
            assistant_message("m4", "answer two"),
        ],
    };
    let resaved = save_conversation(&vault, &second).expect("second save");
    assert_eq!(resaved.message_count, 4);
    assert_eq!(resaved.created_at, created_at);

    let detail = load_conversation(&vault, "conv-2").expect("load").expect("present");
    assert_eq!(detail.messages.len(), 4);
    assert_eq!(detail.messages[3].id, "m4");
    // No stale duplicate rows from the first save.
    let total: i64 = query(
        &vault,
        "SELECT COUNT(*) FROM messages WHERE conversation_id = 'conv-2'",
        (),
        |row| Ok(row.get(0)?),
    )
    .expect("count");
    assert_eq!(total, 4);
}

#[test]
fn list_orders_newest_first_and_clamps_limit() {
    let vault = Vault::open_temp().unwrap();
    for (index, id) in ["conv-a", "conv-b", "conv-c"].into_iter().enumerate() {
        // Save with strictly increasing updated_at by nudging the message content; the store
        // stamps now_rfc3339 per save and the loop runs fast, so also assert via re-save order.
        let request = SaveAgentConversationRequest {
            id: id.to_string(),
            title: Some(format!("conversation {index}")),
            provider_id: None,
            messages: vec![user_message(&format!("m{index}"), "hello")],
        };
        save_conversation(&vault, &request).expect("save");
        // Ensure a distinct updated_at ordering: re-save conv-c last so it sorts first.
    }
    // Touch conv-a last so it becomes newest.
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-a".to_string(),
            title: Some("conversation 0".to_string()),
            provider_id: None,
            messages: vec![user_message("m0b", "again")],
        },
    )
    .expect("touch conv-a");

    let all = list_conversations(&vault, None, None).expect("list default").conversations;
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].id, "conv-a", "most recently updated sorts first");

    // A zero/None limit uses the default; a tiny limit caps the page.
    let one = list_conversations(&vault, Some(1), None).expect("list capped").conversations;
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, "conv-a");

    let zero = list_conversations(&vault, Some(0), None)
        .expect("zero falls back to default")
        .conversations;
    assert_eq!(zero.len(), 3);

    // An oversized limit is clamped but still returns all existing rows.
    let huge =
        list_conversations(&vault, Some(10_000), None).expect("oversized limit").conversations;
    assert_eq!(huge.len(), 3);
}

#[test]
fn load_unknown_conversation_returns_none() {
    let vault = Vault::open_temp().unwrap();
    assert!(load_conversation(&vault, "missing").expect("load missing").is_none());
}

#[test]
fn delete_removes_conversation_and_cascades_messages() {
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-del".to_string(),
            title: None,
            provider_id: None,
            messages: vec![user_message("m1", "to be deleted"), assistant_message("m2", "ok")],
        },
    )
    .expect("save");

    assert!(delete_conversation(&vault, "conv-del").expect("delete present"));
    assert!(!delete_conversation(&vault, "conv-del").expect("delete absent again"));
    assert!(load_conversation(&vault, "conv-del").expect("load").is_none());
    let orphan_messages: i64 = query(
        &vault,
        "SELECT COUNT(*) FROM messages WHERE conversation_id = 'conv-del'",
        (),
        |row| Ok(row.get(0)?),
    )
    .expect("orphan count");
    assert_eq!(orphan_messages, 0, "cascade must remove messages");
}

#[test]
fn rename_updates_title_and_rejects_blank() {
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-rn".to_string(),
            title: Some("old".to_string()),
            provider_id: None,
            messages: vec![user_message("m1", "hi")],
        },
    )
    .expect("save");

    let renamed =
        rename_conversation(&vault, "conv-rn", "  brand new  ").expect("rename").expect("present");
    assert_eq!(renamed.title, "brand new");

    let blank = rename_conversation(&vault, "conv-rn", "   ").expect_err("blank rejected");
    assert!(blank.to_string().contains("cannot be blank"));

    assert!(rename_conversation(&vault, "missing", "x").expect("rename missing").is_none());
}

#[test]
fn resave_with_more_messages_never_truncates_history() {
    // Regression for the data-loss trap: load returned a capped tail, the FE rehydrated that
    // view, and the next save wholesale-replaced the transcript — silently dropping everything
    // beyond the cap. A single chat must survive a re-save with every message intact, even when
    // it is far larger than the old 2_000 tail cap.
    let vault = Vault::open_temp().unwrap();
    let large: Vec<AgentMessage> =
        (0..2_500).map(|i| user_message(&format!("m{i}"), &format!("turn {i}"))).collect();
    let first = SaveAgentConversationRequest {
        id: "conv-big".to_string(),
        title: Some("big chat".to_string()),
        provider_id: None,
        messages: large.clone(),
    };
    let saved = save_conversation(&vault, &first).expect("first save");
    assert_eq!(saved.message_count, 2_500);

    // Load returns the FULL transcript in order (no tail-cap), so a rehydrate-then-resave keeps
    // every message: simulate the FE path by loading, appending one, and re-saving.
    let loaded = load_conversation(&vault, "conv-big").expect("load").expect("present");
    assert_eq!(loaded.messages.len(), 2_500, "load must return all messages, not a tail");
    assert_eq!(loaded.messages.first().expect("first").id, "m0");
    assert_eq!(loaded.messages.last().expect("last").id, "m2499");

    let mut rehydrated = loaded.messages.clone();
    rehydrated.push(user_message("m2500", "one more"));
    let resaved = save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-big".to_string(),
            title: Some("big chat".to_string()),
            provider_id: None,
            messages: rehydrated,
        },
    )
    .expect("resave");
    assert_eq!(resaved.message_count, 2_501);

    // Re-load and assert ALL 2_501 survive, in order, with no loss.
    let after = load_conversation(&vault, "conv-big").expect("load").expect("present");
    assert_eq!(after.messages.len(), 2_501, "re-save must not truncate history");
    for (index, message) in after.messages.iter().enumerate() {
        assert_eq!(message.id, format!("m{index}"), "message {index} out of order or lost");
    }

    // The persisted seq is monotonic and complete (mutation guard: assert the stored column,
    // not the in-memory order).
    let seqs: Vec<i64> = vault
        .store()
        .read(Db::Agent, |s| {
            s.query(
                "SELECT seq FROM messages WHERE conversation_id = 'conv-big' ORDER BY seq ASC",
                (),
                |r| Ok(r.get(0)?),
            )
        })
        .unwrap();
    assert_eq!(seqs, (0..2_501).collect::<Vec<i64>>(), "persisted seq must be 0..N monotonic");
}

#[test]
fn resave_preserves_per_message_created_at_and_stamps_only_new_messages() {
    // Per-message created_at must survive a re-save: restamping every message to `now` made the
    // timestamps meaningless (and is a W-AI-7 trap). Only a genuinely new message id is stamped.
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-ts".to_string(),
            title: None,
            provider_id: None,
            messages: vec![user_message("m1", "first"), assistant_message("m2", "answer")],
        },
    )
    .expect("first save");

    let read_created_at = |id: &str| -> Option<String> {
        query(
            &vault,
            "SELECT created_at FROM messages WHERE id = ?1",
            params![id.to_owned()],
            |row| Ok(row.get(0)?),
        )
        .map(Some)
        .expect("query created_at")
    };
    let m1_created = read_created_at("m1").expect("m1 created");
    let m2_created = read_created_at("m2").expect("m2 created");

    // Re-save with the same two messages plus a new one. The original two keep their stamps;
    // the new one is stamped at save time.
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-ts".to_string(),
            title: None,
            provider_id: None,
            messages: vec![
                user_message("m1", "first"),
                assistant_message("m2", "answer"),
                user_message("m3", "second"),
            ],
        },
    )
    .expect("resave");

    assert_eq!(read_created_at("m1").as_deref(), Some(m1_created.as_str()), "m1 restamped");
    assert_eq!(read_created_at("m2").as_deref(), Some(m2_created.as_str()), "m2 restamped");
    assert!(read_created_at("m3").is_some(), "new message m3 must be stamped");
}

#[test]
fn resave_holds_conversation_created_at_and_advances_updated_at_in_db() {
    // Mutation guard: assert the PERSISTED conversation row (created_at held, updated_at moved),
    // not just the Rust-built return summary.
    let vault = Vault::open_temp().unwrap();
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-row".to_string(),
            title: Some("row".to_string()),
            provider_id: None,
            messages: vec![user_message("m1", "hi")],
        },
    )
    .expect("first save");

    let read_row = || -> (String, String, i64) {
        query(
            &vault,
            "SELECT created_at, updated_at, message_count FROM conversations WHERE id = 'conv-row'",
            (),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("query row")
    };
    let (created_first, updated_first, _) = read_row();

    // Force a strictly later wall-clock stamp so the updated_at comparison is meaningful.
    std::thread::sleep(StdDuration::from_millis(5));
    save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "conv-row".to_string(),
            title: Some("row".to_string()),
            provider_id: None,
            messages: vec![user_message("m1", "hi"), assistant_message("m2", "yo")],
        },
    )
    .expect("resave");

    let (created_second, updated_second, count_second) = read_row();
    assert_eq!(created_second, created_first, "persisted created_at must be held across resave");
    assert!(updated_second > updated_first, "persisted updated_at must advance on resave");
    assert_eq!(count_second, 2, "persisted message_count must reflect the resaved transcript");
}

#[test]
fn bounded_conversation_pages_keep_every_row_reachable() {
    let vault = Vault::open_temp().unwrap();
    let now = now_rfc3339();
    let total = MAX_AGENT_PAGE_SIZE as usize + 25;
    for index in 0..total {
        execute(
            &vault,
            "INSERT INTO conversations
             (id, title, provider_id, created_at, updated_at, message_count)
             VALUES (?1, ?2, NULL, ?3, ?4, 0)",
            params![
                format!("c{index:04}"),
                format!("conversation {index}"),
                now.as_str(),
                format!("{now}-{index:04}")
            ],
        )
        .expect("insert conversation");
    }

    // None falls back to the default page size, not the hard ceiling.
    let default_page = list_conversations(&vault, None, None).expect("default page").conversations;
    assert_eq!(
        default_page.len(),
        DEFAULT_AGENT_PAGE_SIZE as usize,
        "None must use the default page size"
    );
    // An oversized explicit limit clamps to the hard ceiling: exactly 200, never all 225.
    let huge = list_conversations(&vault, Some(10_000), None).expect("oversized limit");
    assert_eq!(
        huge.conversations.len(),
        MAX_AGENT_PAGE_SIZE as usize,
        "Some(10_000) must clamp to the hard ceiling of 200"
    );
    let next = list_conversations(&vault, Some(10_000), huge.next_cursor.as_deref()).unwrap();
    assert!(huge.next_cursor.is_some());
    assert!(next.next_cursor.is_none());
    let ids: std::collections::BTreeSet<_> =
        huge.conversations.into_iter().chain(next.conversations).map(|c| c.id).collect();
    assert_eq!(ids.len(), total);
}

#[test]
fn save_requires_non_empty_id() {
    let vault = Vault::open_temp().unwrap();
    let error = save_conversation(
        &vault,
        &SaveAgentConversationRequest {
            id: "  ".to_string(),
            title: None,
            provider_id: None,
            messages: vec![],
        },
    )
    .expect_err("blank id rejected");
    assert!(error.to_string().contains("conversation id is required"));
}

#[test]
fn auto_title_falls_back_and_truncates() {
    // No user message → fallback title.
    assert_eq!(
        derive_title_from_messages(&[assistant_message("m1", "answer only")]),
        UNTITLED_CONVERSATION_TITLE
    );
    // Blank user content is skipped in favor of the fallback.
    assert_eq!(
        derive_title_from_messages(&[user_message("m1", "   ")]),
        UNTITLED_CONVERSATION_TITLE
    );
    // Whitespace is collapsed.
    assert_eq!(
        derive_title_from_messages(&[user_message("m1", "  multi   line\nprompt  ")]),
        "multi line prompt"
    );
    // Long content is truncated on a char boundary with an ellipsis.
    let long = "word ".repeat(40);
    let title = derive_title_from_messages(&[user_message("m1", &long)]);
    assert!(title.ends_with('…'));
    assert!(title.chars().count() <= AUTO_TITLE_MAX_CHARS + 1);
}

// -----------------------------------------------------------------------
// W-AI-7 migration v2: agent run trace journal.
// -----------------------------------------------------------------------
