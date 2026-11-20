//! The screen's clock, coverage and pricing contract over saved native transcripts.
#[allow(dead_code)]
mod support;
use agent_history::{
    Archive, Harness, SourceRoot,
    archive::ConversationMark,
    insights::{Preferences, Query},
};
use serde_json::{Value, json};
use std::path::Path;
use support::{
    fixtures::{jsonl, pi_message},
    native::*,
};

fn report(a: &Archive, query: Value, zone: &str, now: &str, prefs: Preferences) -> Value {
    let q: Query = serde_json::from_value(query).unwrap();
    let at = chrono::DateTime::parse_from_rfc3339(now).unwrap().timestamp_millis();
    let mut frames = json!({});
    a.agent_insights(&q, zone, at, &prefs, &Default::default(), |kind, value| {
        frames[kind] = value;
        Ok(())
    })
    .unwrap();
    frames
}
fn pi_log(root: &Path, id: &str, rows: &[Value]) {
    let dir = root.join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    let mut log = vec![json!({"type":"session","id":id,"version":3,"cwd":"/work/report"})];
    log.extend_from_slice(rows);
    std::fs::write(dir.join(format!("{id}.jsonl")), jsonl(&log)).unwrap();
}
#[test]
fn midnight_clips_clocks_and_prices_each_native_reply() {
    let t = tempfile::tempdir().unwrap();
    let claude = t.path().join("claude");
    let pi = t.path().join("pi");
    let mut user = claude_user("c", "u", None, "/work/report", "question");
    user["timestamp"] = json!("2026-11-10T23:50:00Z");
    let mut first = claude_reply("c", "a", "u", "/work/report", "recorded answer");
    first["timestamp"] = json!("2026-11-10T23:59:00Z");
    first["costUSD"] = json!(0.012);
    let mut second = claude_reply("c", "b", "a", "/work/report", "estimated answer");
    second["timestamp"] = json!("2026-11-11T00:02:00Z");
    second["message"]["model"] = json!("claude-sonnet-4-6");
    second["message"]["usage"] = json!({"input_tokens":4,"output_tokens":1});
    claude_main(
        &claude,
        "c",
        &[
            user,
            first,
            second,
            json!({"type":"system","subtype":"turn_duration","durationMs":1200000,"timestamp":"2026-11-11T00:10:00Z","sessionId":"c"}),
        ],
    );
    let mut prompt = pi_message("u", None, "user", "Pi question");
    prompt["timestamp"] = json!("2026-11-10T23:58:00Z");
    let mut reply = pi_message("a", Some("u"), "assistant", "no usage");
    reply["timestamp"] = json!("2026-11-11T00:18:00Z");
    reply["message"].as_object_mut().unwrap().remove("usage");
    pi_log(&pi, "p", &[prompt, reply]);
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    a.backup(
        &[
            SourceRoot { harness: Harness::Pi, root: pi },
            SourceRoot { harness: Harness::ClaudeCode, root: claude },
        ],
        &Default::default(),
    )
    .unwrap();
    let now = "2026-11-11T12:00:00Z";
    let all = report(&a, json!({"range":"7d"}), "UTC", now, Default::default());
    let k = &all["overview"];
    assert_eq!(k["agentBusyMs"], 28 * 60000);
    assert_eq!(k["agentSummedMs"], 40 * 60000);
    assert_eq!(k["presentMs"], 23 * 60000);
    assert_eq!(k["conversationCount"], 2);
    assert_eq!(k["recordedDurationConversations"], 1);
    assert_eq!(k["derivedDurationConversations"], 1);
    assert_eq!(k["conversationsWithUsage"], 1);
    assert_eq!(k["tokens"]["input"], 14);
    assert_eq!(k["tokens"]["output"], 3);
    assert_eq!(k["cost"]["recordedUsd"], 0.012);
    assert!((k["cost"]["estimatedUsd"].as_f64().unwrap() - 0.000027).abs() < 1e-12);
    assert_eq!(k["cost"]["unpricedConversations"], 1);
    let series = all["series"]["buckets"].as_array().unwrap();
    let bins = &series[series.len() - 2..];
    assert_eq!(bins[0]["busyMs"], 600000);
    assert_eq!(bins[1]["summedMs"], 28 * 60000);
    assert_eq!(bins[1]["presentMs"], 13 * 60000);
    assert_eq!(bins[0]["tokens"]["input"], 10);
    assert_eq!(bins[1]["tokens"]["input"], 4);
    let recorded_only =
        report(&a, json!({"range":"7d","includeEstimates":false}), "UTC", now, Default::default());
    assert_eq!(recorded_only["overview"]["cost"]["totalUsd"], 0.012);
    assert!(recorded_only["overview"]["cost"]["estimatedUsd"].is_null());
    assert_eq!(k["cost"]["bothConversations"], 1);
    let today =
        report(&a, json!({"range":"today","part":"report"}), "UTC", now, Default::default());
    assert_eq!(today["overview"]["agentBusyMs"], 18 * 60000);
    assert_eq!(today["overview"]["cost"]["recordedUsd"], Value::Null);
    let taipei = report(&a, json!({"range":"today"}), "Asia/Taipei", now, Default::default());
    assert_eq!(taipei["overview"]["agentBusyMs"], 28 * 60000);
    let unknown = report(
        &a,
        json!({"range":"today","harnesses":["pi"]}),
        "Asia/Taipei",
        now,
        Default::default(),
    );
    assert!(unknown["overview"]["tokens"].is_null());
    assert!(unknown["series"]["buckets"].as_array().unwrap().iter().all(|b| b["tokens"].is_null()));
    assert_eq!(unknown["overview"]["conversationsWithUsage"], 0);
    let id =
        a.chat_list(&Default::default()).unwrap()["items"][0]["id"].as_str().unwrap().to_owned();
    a.mark_conversation(
        &id,
        &ConversationMark { put_away: Some(true), ..Default::default() },
        true,
    )
    .unwrap();
    assert_eq!(
        report(&a, json!({}), "UTC", now, Default::default())["overview"]["conversationCount"],
        2
    );
    a.mark_conversation(&id, &ConversationMark { trash: Some(true), ..Default::default() }, true)
        .unwrap();
    assert_eq!(
        report(&a, json!({}), "UTC", now, Default::default())["overview"]["conversationCount"],
        1
    );
}

#[test]
fn own_work_child_models_tools_and_cost_preferences() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("claude");
    let cwd = "/work/insights";
    let mut call = claude_tool_call(
        "parent",
        "edit",
        "u",
        cwd,
        json!({"file_path":"file","old_string":"a\n","new_string":"b\nc\n"}),
    );
    call["message"]["content"][0]["name"] = json!("Edit");
    call["message"]["model"] = json!("claude-sonnet-4-6");
    claude_main(&root, "parent", &[claude_user("parent", "u", None, cwd, "edit it"), call]);
    claude_subagent(&root, "parent", "agent-kid.jsonl", "kid", cwd, "subtask");
    a.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &Default::default()).unwrap();
    let now = "2026-11-11T12:00:00Z";
    let all = report(&a, json!({}), "UTC", now, Default::default());
    let k = &all["overview"];
    assert_eq!(k["conversationCount"], 1);
    assert_eq!(k["subagentCount"], 1);
    assert_eq!(k["tokens"]["input"], 15);
    assert_eq!(k["tokens"]["output"], 3);
    assert_eq!(k["cost"]["unpricedConversations"], 1);
    let b = &all["breakdowns"];
    assert_eq!(b["toolCalls"]["top"][0]["name"], "Edit");
    assert_eq!(b["linesAdded"], 2);
    assert_eq!(b["linesRemoved"], 1);
    assert_eq!(b["models"][0]["model"], "claude-sonnet-4-6");
    assert_eq!(b["models"][0]["primaryConversations"], 1);
    assert_eq!(b["models"][1]["primaryConversations"], 0);
    let cells = b["hourOfWeek"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap())
        .map(|n| n.as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(cells, 1);
    let id =
        a.chat_list(&Default::default()).unwrap()["items"][0]["id"].as_str().unwrap().to_owned();
    let child = a.reader_children(&a.reader_header(&id).unwrap(), None, 2).unwrap().pop().unwrap();
    a.mark_conversation(
        &child,
        &ConversationMark { trash: Some(true), ..Default::default() },
        true,
    )
    .unwrap();
    let p = a
        .update_insight_preferences(
            serde_json::from_value(
                json!({"range":"30d","estimateCost":false,"equivalent":"workdays"}),
            )
            .unwrap(),
        )
        .unwrap();
    let changed = report(&a, json!({}), "UTC", now, p);
    assert_eq!(changed["started"]["range"], "30d");
    assert_eq!(changed["overview"]["subagentCount"], 0);
    assert_eq!(changed["overview"]["tokens"]["input"], 10);
    assert!(changed["overview"]["cost"]["totalUsd"].is_null());
    drop(a);
    let a = Archive::open(&t.path().join("archive"), b"").unwrap();
    assert!(!a.insight_preferences().unwrap().estimate_cost);
    let p = a
        .update_insight_preferences(serde_json::from_value(json!({"showCost":false})).unwrap())
        .unwrap();
    assert!(report(&a, json!({}), "UTC", now, p)["overview"].get("cost").is_none());
}
#[test]
fn measured_zero_and_the_two_local_one_oclocks() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let pi = t.path().join("pi");
    let mut u = pi_message("u", None, "user", "work");
    u["timestamp"] = json!("2026-11-01T04:00:00Z");
    let mut r = pi_message("a", Some("u"), "assistant", "one");
    r["timestamp"] = json!("2026-11-01T05:30:00Z");
    r["message"]["usage"] = json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"total":0.}});
    let mut r2 = r.clone();
    r2["id"] = json!("b");
    r2["parentId"] = json!("a");
    r2["timestamp"] = json!("2026-11-01T06:30:00Z");
    r2["message"]["usage"] = json!({"input":2,"output":1,"cacheRead":0,"cacheWrite":0,"totalTokens":3,"cost":{"total":0.008}});
    let mut end = pi_message("z", Some("b"), "assistant", "done");
    end["timestamp"] = json!("2026-11-02T05:00:00Z");
    end["message"].as_object_mut().unwrap().remove("usage");
    pi_log(&pi, "fallback", &[u, r, r2, end]);
    a.backup(&[SourceRoot { harness: Harness::Pi, root: pi }], &Default::default()).unwrap();
    let v = report(
        &a,
        json!({"range":"today"}),
        "America/New_York",
        "2026-11-01T23:00:00Z",
        Default::default(),
    );
    let bins = v["series"]["buckets"].as_array().unwrap();
    assert_eq!(bins.len(), 25);
    assert_eq!(bins[1]["tokens"]["input"], 0);
    assert_eq!(bins[2]["tokens"]["input"], 2);
    assert!(bins[3]["tokens"].is_null());
    assert_eq!(v["overview"]["agentBusyMs"], 25 * 3600000);
    assert_eq!(v["calendar"]["busyMs"], 25 * 3600000);
    assert_eq!(
        v["calendar"]["days"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["day"] == "2026-11-01")
            .unwrap()["busyMs"],
        25 * 3600000
    );
    let c = vault_store::CancelToken::new();
    let mut frames = vec![];
    let result = a.agent_insights(
        &Default::default(),
        "UTC",
        1794355200000,
        &Default::default(),
        &c,
        |ty, _| {
            frames.push(ty.to_owned());
            c.cancel();
            Ok(())
        },
    );
    assert_eq!(result.unwrap_err().code(), "cancelled");
    assert!(!frames.iter().any(|ty| ty == "finished"));
}

#[test]
fn a_prompt_without_a_duration_or_meter_stays_unknown_in_the_project_scope() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("pi");
    pi_log(&root, "open", &[pi_message("u", None, "user", "still waiting")]);
    a.backup(&[SourceRoot { harness: Harness::Pi, root }], &Default::default()).unwrap();
    let project = a.project_list().unwrap()["items"][0]["id"].clone();
    a.edit_project(
        serde_json::from_value(
            json!({"version":0,"action":{"kind":"rename","id":project,"name":"Current name"}}),
        )
        .unwrap(),
    )
    .unwrap();
    let r = report(&a, json!({"projectId":project}), "UTC", TIME, Default::default());
    let k = &r["overview"];
    assert_eq!(k["unknownDurationConversations"], 1);
    assert_eq!(k["conversationCount"], 1);
    for field in [
        "tokens",
        "averageTokensPerMeasuredConversation",
        "agentSummedMs",
        "agentBusyMs",
        "concurrency",
    ] {
        assert_eq!(k[field], Value::Null, "{field}");
    }
    assert_eq!(r["breakdowns"]["topProjects"][0]["project"]["name"], "Current name");
    let empty =
        report(&a, json!({"range":"today"}), "UTC", "2026-11-20T00:00:00Z", Default::default());
    assert_eq!(empty["overview"]["conversationCount"], 0);
    assert_eq!(empty["calendar"]["activeDays"], 1);
}

#[test]
fn skipped_local_midnight_starts_the_day_at_its_first_instant() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let pi = t.path().join("pi");
    let mut u = pi_message("u", None, "user", "before the jump");
    u["timestamp"] = json!("2018-11-04T02:59:30Z");
    let mut r = pi_message("r", Some("u"), "assistant", "after the jump");
    r["timestamp"] = json!("2018-11-04T03:00:30Z");
    pi_log(&pi, "spring", &[u, r]);
    a.backup(&[SourceRoot { harness: Harness::Pi, root: pi }], &Default::default()).unwrap();
    let data = report(
        &a,
        json!({"range":"today"}),
        "America/Sao_Paulo",
        "2018-11-04T12:00:00Z",
        Default::default(),
    );
    assert_eq!(data["series"]["buckets"].as_array().unwrap().len(), 23);
    assert_eq!(data["overview"]["agentBusyMs"], 30000);
    assert_eq!(data["overview"]["presentMs"], 30000);
    assert_eq!(data["overview"]["conversationsWithUsage"], 1);
    assert_eq!(data["series"]["buckets"][0]["tokens"]["input"], 10);
}
