//! Responsibilities: generate synthetic logs in source-verified native formats.
//! Not responsible for: imitating harness execution or inventing generic transcripts.
//! Schema evidence: docs/architecture/research/agent-history/{claude,codex,pi-deepseek}.md.
use super::write;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const SHARED_ID: &str = "11111111-1111-4111-8111-111111111111";
pub const PRIVATE_TEXT: &str = "archivesecretsentinelquartz";
pub const FUTURE_SENTINEL: &str = "future-event-retained-verbatim";
const TIME: &str = "2026-11-10T12:00:00.000Z";
const EPOCH: i64 = 1_794_312_000_000;

pub struct Fixture {
    pub source_tree: PathBuf,
    pub project: String,
    pub roots: Vec<(&'static str, PathBuf)>,
    pub files: Vec<PathBuf>,
}

impl Fixture {
    pub fn four_harnesses(root: &Path) -> Self {
        let source_tree = root.join("sources");
        let project = root.join("projects/shared-project").to_string_lossy().into_owned();
        let roots = ["claude-code", "codex", "pi", "deepseek"]
            .into_iter()
            .map(|harness| (harness, source_tree.join(harness)))
            .collect::<Vec<_>>();
        let files = vec![
            claude(&roots[0].1, &project),
            codex(&roots[1].1, &project),
            pi(&roots[2].1, &project, SHARED_ID, "pi uniquepineedle"),
            deepseek(&roots[3].1, &project),
        ];
        Self { source_tree, project, roots, files }
    }

    pub fn source_args(&self) -> Vec<String> {
        self.roots
            .iter()
            .flat_map(|(harness, path)| {
                ["--source".into(), format!("{harness}={}", path.display())]
            })
            .collect()
    }

    pub fn description(&self) -> Value {
        json!({
            "schema_version":1,"seed":"agent-history-native-contract-2026-11-11",
            "harnesses":self.roots.iter().map(|(h,_)| h).collect::<Vec<_>>(),
            "shared_native_id":SHARED_ID,"project":self.project,
            "clock":TIME,"private_data":false,
            "source_commits":{
                "codex":"3a89580f08c985e99b9cc99df53c67fb8482ef93",
                "pi":"49fe0a237450eb003434d4020b0bfdf53bc6449c",
                "deepseek":"fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159"
            }
        })
    }
}

pub fn jsonl(rows: &[Value]) -> Vec<u8> {
    let mut bytes = vec![];
    for row in rows {
        serde_json::to_writer(&mut bytes, row).unwrap();
        bytes.push(b'\n');
    }
    bytes
}

fn claude(root: &Path, project: &str) -> PathBuf {
    let path = root.join(format!("projects/--not-a-reversible-path--/{SHARED_ID}.jsonl"));
    let envelope = |kind: &str, id: &str, parent: Option<&str>, message: Value| {
        json!({"type":kind,"uuid":id,"parentUuid":parent,"sessionId":SHARED_ID,
            "timestamp":TIME,"cwd":project,"version":"2.1.322","entrypoint":"cli",
            "message":message})
    };
    write(
        &path,
        &jsonl(&[
            envelope(
                "user",
                "user-1",
                None,
                json!({"role":"user","content":format!("{PRIVATE_TEXT} sharedprojectneedle") }),
            ),
            envelope(
                "assistant",
                "assistant-1",
                Some("user-1"),
                json!({"id":"same-response","role":"assistant","model":"synthetic-model",
                "content":[{"type":"text","text":"branchalphaneedle"}],
                "usage":{"input_tokens":10,"output_tokens":2}}),
            ),
            // Both records are retained even though their provider response ID is the same.
            envelope(
                "assistant",
                "assistant-2",
                Some("assistant-1"),
                json!({"id":"same-response","role":"assistant","model":"synthetic-model",
                "content":[{"type":"text","text":"branchbetaneedle"}],
                "usage":{"input_tokens":10,"output_tokens":2}}),
            ),
            envelope(
                "user",
                "branch-user",
                Some("user-1"),
                json!({"role":"user","content":"unselectedbranchneedle"}),
            ),
            json!({"type":"future-agent-event","uuid":"opaque-1","sessionId":SHARED_ID,
            "future":{"nested":[FUTURE_SENTINEL,{"number":9007199254740991_i64}]}}),
        ]),
    );
    path
}

fn codex(root: &Path, project: &str) -> PathBuf {
    let path =
        root.join(format!("archived_sessions/rollout-2026-11-10T12-00-00-{SHARED_ID}.jsonl.zst"));
    let rows = [
        json!({"timestamp":TIME,"ordinal":0,"type":"session_meta","payload":{
            "id":SHARED_ID,"session_id":SHARED_ID,"timestamp":TIME,"cwd":project,
            "originator":"Codex Desktop","cli_version":"0.167.1","source":"vscode",
            "history_mode":"paginated"}}),
        json!({"timestamp":TIME,"ordinal":1,"type":"response_item","payload":{
            "type":"message","role":"user","content":[{"type":"input_text",
                "text":format!("{PRIVATE_TEXT} sharedprojectneedle uniquecodexneedle")}]}}),
        json!({"timestamp":TIME,"ordinal":2,"type":"event_msg","payload":{
            "type":"item_completed","thread_id":SHARED_ID,"turn_id":"turn-1",
            "item":{"type":"AgentMessage","id":"reply-1","phase":"final_answer",
                "content":[{"type":"Text","text":"codexreplyneedle"}]},
            "completed_at_ms":EPOCH}}),
        // This is copied ancestor metadata, not a new identity for this physical rollout.
        json!({"timestamp":TIME,"ordinal":3,"type":"session_meta","payload":{
            "id":"22222222-2222-4222-8222-222222222222","timestamp":TIME,
            "cwd":"/not-the-current-project","originator":"codex_cli_rs","source":"cli"}}),
    ];
    write(&path, &zstd::stream::encode_all(jsonl(&rows).as_slice(), 1).unwrap());
    path
}

pub fn pi(root: &Path, project: &str, id: &str, text: &str) -> PathBuf {
    let path =
        root.join(format!("sessions/--not-a-reversible-path--/2026-11-10T12-00-00Z_{id}.jsonl"));
    write(
        &path,
        &jsonl(&[
            json!({"type":"session","version":3,"id":id,"timestamp":TIME,"cwd":project}),
            pi_message(
                "00000001",
                None,
                "user",
                &format!("{PRIVATE_TEXT} sharedprojectneedle {text}"),
            ),
            pi_message("00000002", Some("00000001"), "assistant", "pianswerneedle"),
            json!({"type":"custom","id":"00000003","parentId":"00000002","timestamp":TIME,
            "customType":"future-extension","data":{"preserve":FUTURE_SENTINEL}}),
        ]),
    );
    path
}

pub fn pi_message(id: &str, parent: Option<&str>, role: &str, text: &str) -> Value {
    let mut message =
        json!({"role":role,"content":[{"type":"text","text":text}],"timestamp":EPOCH});
    if role == "assistant" {
        message["api"] = json!("anthropic-messages");
        message["provider"] = json!("synthetic-provider");
        message["model"] = json!("synthetic-model");
        message["stopReason"] = json!("stop");
        message["usage"] = json!({"input":10,"output":2,"cacheRead":0,"cacheWrite":0,
            "totalTokens":12,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}});
    }
    json!({"type":"message","id":id,"parentId":parent,"timestamp":TIME,"message":message})
}

fn deepseek(root: &Path, project: &str) -> PathBuf {
    let folder = root.join(format!("sessions/--not-a-reversible-path--/{SHARED_ID}"));
    let header = json!({"type":"session","version":4,"id":SHARED_ID,
        "createdAt":EPOCH,"cwd":project,"isSeeded":false,"delegationDepth":0});
    let mut old_header = header.clone();
    old_header["version"] = json!(3);
    write(&folder.join("session.v3.jsonl"), &jsonl(&[old_header]));
    // Lifecycle shape is from upstream relationships.spec.ts's ordinary tool exchange.
    let bodies = [
        ("turn/start", json!({"turn":1})),
        ("step/start", json!({"turn":1,"step":1})),
        (
            "user/message",
            json!({"id":"input","role":"user","source":{"kind":"user"},
            "content":[{"type":"text","text":format!("{PRIVATE_TEXT} sharedprojectneedle")}] }),
        ),
        (
            "request/header",
            json!({"reason":"initial","header":{"config":{"provider":"mock","model":"mock"}}}),
        ),
        (
            "assistant/message",
            json!({"turn":1,"step":1,"stream":[],"message":{
            "id":"assistant","role":"assistant","source":{"kind":"model","provider":"mock","model":"mock"},
            "content":[{"type":"tool-call","id":"call","name":"read","arguments":"{}"}]}}),
        ),
        ("tool/call", json!({"turn":1,"step":1,"callId":"call","name":"read","arguments":"{}"})),
        (
            "tool/result",
            json!({"turn":1,"step":1,"message":{
            "id":"result","role":"tool","source":{"kind":"tool","callId":"call"},
            "toolCallId":"call","content":[{"type":"text","text":"uniquedeepseekneedle"}]}}),
        ),
        ("step/end", json!({"turn":1,"step":1})),
        ("turn/end", json!({"turn":1,"reason":{"kind":"completed"}})),
    ];
    let mut rows = vec![];
    for (seq, (kind, data)) in bodies.into_iter().enumerate() {
        let mut event = json!({"type":kind,"seq":seq,"time":EPOCH + seq as i64 + 1,"data":data});
        if ["user/message", "assistant/message", "tool/result"].contains(&kind) {
            event["surfaceOp"] = json!("append");
        }
        rows.push(event);
    }
    let path = folder.join("session.v4.jsonl.zstd");
    // Native storage appends independent Zstd frames: one header, then event batches.
    let mut compressed = zstd::stream::encode_all(jsonl(&[header]).as_slice(), 1).unwrap();
    compressed.extend(zstd::stream::encode_all(jsonl(&rows).as_slice(), 1).unwrap());
    write(&path, &compressed);
    path
}
