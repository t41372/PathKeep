//! Responsibilities: keep every message of a chat and every chat in the list.
//! Not responsible for: provider execution or schema migrations.
//! Failure modes: a re-save that drops old messages, and conversations past a page the
//! cursor never reaches.

use super::*;

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

    // An oversized limit still pages: the rows past the ceiling come through the cursor.
    let huge = list_conversations(&vault, Some(10_000), None).expect("oversized limit");
    let next = list_conversations(&vault, Some(10_000), huge.next_cursor.as_deref()).unwrap();
    assert!(huge.next_cursor.is_some());
    assert!(next.next_cursor.is_none());
    let ids: std::collections::BTreeSet<_> =
        huge.conversations.into_iter().chain(next.conversations).map(|c| c.id).collect();
    assert_eq!(ids.len(), total);
}
