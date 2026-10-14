//! Responsibilities: verify real agent storage pagination and crash durability.
//! Not responsible for: model calls or desktop UI behaviour.
//! Failure modes: tied recency drops chats, a page cap hides old chats/messages,
//! replay duplicates steps, reopen loses completed runs or reconstructed evidence.

use vault_core::{AgentMessage, SaveAgentConversationRequest, Vault, agent_store::*};
use vault_store::{Db, Priority};

#[test]
fn a_failed_replacement_preserves_the_saved_transcript() {
    for key in [None, Some([48; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let request = SaveAgentConversationRequest {
            id: "atomic".into(),
            messages: vec![AgentMessage {
                id: "turn".into(),
                role: "user".into(),
                content: "keep me".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        save_conversation(&vault, &request).unwrap();
        let before = load_conversation(&vault, "atomic").unwrap().unwrap();
        let mut invalid = request.clone();
        invalid.messages.push(invalid.messages[0].clone());
        assert!(save_conversation(&vault, &invalid).is_err());
        assert_eq!(load_conversation(&vault, "atomic").unwrap().unwrap(), before);
    }
}

#[test]
fn committed_agent_journal_survives_process_death() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    for key in [None, Some([49; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "journal_crash_child", "--ignored", "--nocapture"])
            .env("PATHKEEP_AGENT_CRASH_ROOT", &paths.app_root)
            .env("PATHKEEP_AGENT_CRASH_ENCRYPTED", if key.is_some() { "1" } else { "0" })
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut committed = false;
        for line in output.lines() {
            if line.unwrap().contains("agent-journal-committed") {
                committed = true;
                break;
            }
        }
        assert!(committed, "child exited without committing the journal");
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        let reopened = Vault::open(paths, config, key).unwrap();
        let unfinished = load_agent_run(&reopened, "unfinished").unwrap().unwrap();
        assert_eq!(unfinished.status, AgentRunStatus::Running);
        assert_eq!(unfinished.steps.len(), 1);
        assert_eq!(append_agent_step(&reopened, &crash_step()).unwrap(), 0);
        assert_eq!(load_agent_run(&reopened, "unfinished").unwrap().unwrap().steps.len(), 1);
        let finished = load_agent_run(&reopened, "finished").unwrap().unwrap();
        assert_eq!(finished.status, AgentRunStatus::Completed);
        assert_eq!((finished.prompt_tokens, finished.completion_tokens), (23, 7));
        assert_eq!(finished.citations[0].history_id, 17);
    }
}

fn crash_step() -> AppendAgentStep {
    AppendAgentStep {
        run_id: "unfinished".into(),
        turn: 0,
        kind: "tool-result".into(),
        tool_name: Some("search_history".into()),
        tool_call_id: Some("call-1".into()),
        payload: "durable result".into(),
    }
}

#[test]
#[ignore = "subprocess fixture, killed by committed_agent_journal_survives_process_death"]
fn journal_crash_child() {
    use std::io::Write;
    let root = std::env::var_os("PATHKEEP_AGENT_CRASH_ROOT").unwrap();
    let key = (std::env::var("PATHKEEP_AGENT_CRASH_ENCRYPTED").unwrap() == "1").then_some([49; 32]);
    let config = vault_core::AppConfig {
        archive_mode: if key.is_some() {
            vault_core::ArchiveMode::Encrypted
        } else {
            vault_core::ArchiveMode::Plaintext
        },
        ..Default::default()
    };
    let vault =
        Vault::open(vault_core::project_paths_with_root(std::path::Path::new(&root)), config, key)
            .unwrap();
    for id in ["unfinished", "finished"] {
        begin_agent_run(&vault, &BeginAgentRun { id: id.into(), ..Default::default() }).unwrap();
    }
    append_agent_step(&vault, &crash_step()).unwrap();
    record_agent_citations(
        &vault,
        "finished",
        &[AgentCitationRecord {
            history_id: 17,
            canonical_url: "https://example.test/".into(),
            url: "https://example.test/".into(),
            title: None,
            visited_at: None,
            score: Some(0.5),
            profile_id: None,
        }],
    )
    .unwrap();
    finalize_agent_run(&vault, "finished", AgentRunStatus::Completed, 1, 23, 7, None).unwrap();
    println!("agent-journal-committed");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn chats_and_messages_reach_every_row_after_reopen() {
    for key in [None, Some([47; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Agent, Priority::Batch, |s| {
            for id in 0..425 {
                s.execute("INSERT INTO conversations(id,title,created_at,updated_at) VALUES(?1,?1,'same','same')", [format!("chat-{id:04}")])?;
            }
            Ok(())
        }).unwrap();
        let messages = (0..2501)
            .map(|id| AgentMessage {
                id: format!("message-{id}"),
                role: "user".into(),
                content: format!("text-{id}"),
                ..Default::default()
            })
            .collect();
        save_conversation(
            &vault,
            &SaveAgentConversationRequest {
                id: "transcript".into(),
                messages,
                ..Default::default()
            },
        )
        .unwrap();
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let reopened = Vault::open(paths, config, key).unwrap();
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let page = list_conversations(&reopened, Some(37), cursor.as_deref()).unwrap();
            ids.extend(page.conversations.into_iter().map(|row| row.id));
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(ids.len(), 426);
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 426);
        let mut cursor = None;
        let mut messages = Vec::new();
        loop {
            let page =
                list_conversation_messages(&reopened, "transcript", Some(97), cursor).unwrap();
            messages.extend(page.messages);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(messages.len(), 2501);
        for (seq, message) in messages.iter().enumerate() {
            assert_eq!(message.id, format!("message-{seq}"));
            assert_eq!(message.content, format!("text-{seq}"));
        }
        assert_eq!(load_conversation(&reopened, "transcript").unwrap().unwrap().messages, messages);
    }
}
