//! Responsibilities: synthetic files in the native layouts the existing adapter tests
//! and research notes pin: Claude Code main and subagent transcripts with uploads,
//! Codex rollouts with reverted continuations, chains and forks, DeepSeek generations
//! and appended zstd frames, Continue documents, and `.git` folders and worktrees.
//! Not responsible for: running any harness or `git`, or reading the real home directory.
//! Shapes: tests/{claude,codex,deepseek}_adapter.rs, tests/additional_adapters.rs and
//! docs/architecture/research/agent-history/{claude-code,codex,pi-deepseek}.md.
#![allow(dead_code)]

use super::fixtures::jsonl;
use super::write;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

pub const TIME: &str = "2026-11-10T12:00:00.000Z";
pub const EPOCH: i64 = 1_794_312_000_000;
/// Claude's encoded project folder under `projects/`; the name is not a reversible path.
pub const CLAUDE_PROJECT: &str = "projects/--synthetic-contract-project--";

pub fn claude_user(
    session: &str,
    uuid: &str,
    parent: Option<&str>,
    cwd: &str,
    text: &str,
) -> Value {
    json!({"type":"user","uuid":uuid,"parentUuid":parent,"sessionId":session,
        "timestamp":TIME,"cwd":cwd,"version":"2.1.322","entrypoint":"cli",
        "message":{"role":"user","content":text}})
}

pub fn claude_reply(session: &str, uuid: &str, parent: &str, cwd: &str, text: &str) -> Value {
    json!({"type":"assistant","uuid":uuid,"parentUuid":parent,"sessionId":session,
        "timestamp":TIME,"cwd":cwd,"version":"2.1.322","entrypoint":"cli",
        "message":{"id":format!("response-{uuid}"),"role":"assistant","model":"synthetic-model",
            "content":[{"type":"text","text":text}],"usage":{"input_tokens":10,"output_tokens":2}}})
}

/// A tool call whose arguments name folders; arguments never authorize an observation.
pub fn claude_tool_call(session: &str, uuid: &str, parent: &str, cwd: &str, input: Value) -> Value {
    json!({"type":"assistant","uuid":uuid,"parentUuid":parent,"sessionId":session,
        "timestamp":TIME,"cwd":cwd,"version":"2.1.322","entrypoint":"cli",
        "message":{"id":format!("response-{uuid}"),"role":"assistant","model":"synthetic-model",
            "content":[{"type":"tool_use","id":format!("call-{uuid}"),"name":"Bash","input":input}],
            "usage":{"input_tokens":10,"output_tokens":2}}})
}

pub fn claude_title(session: &str, title: &str) -> Value {
    json!({"type":"custom-title","sessionId":session,"customTitle":title})
}

/// A structured attachment record; the path is the reference the adapter resolves.
pub fn claude_attachment(session: &str, uuid: &str, path: &Path) -> Value {
    json!({"type":"attachment","uuid":uuid,"sessionId":session,"timestamp":TIME,
        "attachment":{"type":"file","filename":path.to_str().unwrap()}})
}

/// `projects/<project>/<session>.jsonl`.
pub fn claude_main(root: &Path, session: &str, rows: &[Value]) -> PathBuf {
    let path = root.join(format!("{CLAUDE_PROJECT}/{session}.jsonl"));
    write(&path, &jsonl(rows));
    path
}

/// `projects/<project>/<session>/subagents/<below>`; records carry the parent's
/// `sessionId`, as 7,112 of 7,284 local subagent files do.
pub fn claude_subagent(
    root: &Path,
    session: &str,
    below: &str,
    agent: &str,
    cwd: &str,
    text: &str,
) -> PathBuf {
    let path = root.join(format!("{CLAUDE_PROJECT}/{session}/subagents/{below}"));
    let rows = [
        json!({"type":"user","uuid":format!("{agent}-u"),"parentUuid":null,"sessionId":session,
            "agentId":agent,"isSidechain":true,"timestamp":TIME,"cwd":cwd,
            "message":{"role":"user","content":text}}),
        json!({"type":"assistant","uuid":format!("{agent}-a"),"parentUuid":format!("{agent}-u"),
            "sessionId":session,"agentId":agent,"isSidechain":true,"timestamp":TIME,"cwd":cwd,
            "message":{"id":format!("response-{agent}"),"role":"assistant","model":"synthetic-model",
                "content":[{"type":"text","text":"subagent finished"}],
                "usage":{"input_tokens":5,"output_tokens":1}}}),
    ];
    write(&path, &jsonl(&rows));
    path
}

/// `uploads/<session>/<name>`, a session-owned file Claude Code discovery captures.
pub fn claude_upload(root: &Path, session: &str, name: &str, bytes: &[u8]) -> PathBuf {
    let path = root.join(format!("uploads/{session}/{name}"));
    write(&path, bytes);
    path
}

pub fn codex_meta(thread: &str, cwd: &str) -> Value {
    codex_meta_at(thread, cwd, TIME)
}

/// Every rollout's `session_meta` records its own time: across the 54 local threads
/// that were reverted, no two files of one thread share `payload.timestamp`.
pub fn codex_meta_at(thread: &str, cwd: &str, time: &str) -> Value {
    json!({"timestamp":time,"type":"session_meta","payload":{
        "id":thread,"session_id":thread,"timestamp":time,"cwd":cwd,
        "originator":"codex_cli_rs","source":"cli","cli_version":"0.167.1",
        "history_mode":"paginated"}})
}

pub fn codex_user(text: &str) -> Value {
    codex_user_at(TIME, text)
}

pub fn codex_user_at(time: &str, text: &str) -> Value {
    json!({"timestamp":time,"type":"response_item","payload":{
        "type":"message","role":"user","content":[{"type":"input_text","text":text}]}})
}

pub fn codex_reply(thread: &str, id: &str, text: &str) -> Value {
    codex_reply_at(TIME, thread, id, text)
}

pub fn codex_reply_at(time: &str, thread: &str, id: &str, text: &str) -> Value {
    json!({"timestamp":time,"type":"event_msg","payload":{
        "type":"item_completed","thread_id":thread,"turn_id":format!("turn-{id}"),
        "item":{"type":"AgentMessage","id":id,"phase":"final_answer",
            "content":[{"type":"Text","text":text}]},"completed_at_ms":EPOCH}})
}

/// `sessions/YYYY/MM/DD/rollout-<time>-<thread>.jsonl`.
pub fn codex_rollout(root: &Path, time: &str, name: &str, rows: &[Value]) -> PathBuf {
    let path = root.join(format!("sessions/2026/11/10/rollout-{time}-{name}.jsonl"));
    write(&path, &jsonl(rows));
    path
}

/// A thread whose last turn was reverted: the second rollout, named
/// `<thread>_<rollout>`, inherits the first `boundary` records of the first rollout
/// through `history_base`, whose offsets are computed from the real base bytes.
pub struct RevertedThread {
    pub base: PathBuf,
    pub continuation: PathBuf,
    pub boundary: u64,
    pub base_rows: Vec<Value>,
    pub continuation_rows: Vec<Value>,
}

pub fn codex_reverted_thread(
    root: &Path,
    thread: &str,
    rollout: &str,
    base_cwd: &str,
    continuation_cwd: &str,
) -> RevertedThread {
    let base_rows = vec![
        codex_meta_at(thread, base_cwd, "2026-11-10T12:00:00.000Z"),
        codex_user_at("2026-11-10T12:01:00.000Z", "first question codexbaseneedle"),
        codex_reply_at("2026-11-10T12:02:00.000Z", thread, "reply-1", "first answer"),
        codex_user_at("2026-11-10T12:03:00.000Z", "reverted question codexrevertedneedle"),
        codex_reply_at("2026-11-10T12:04:00.000Z", thread, "reply-2", "reverted answer"),
    ];
    let boundary = 3_u64;
    let end_byte_offset = jsonl(&base_rows[..boundary as usize]).len();
    let base = codex_rollout(root, "2026-11-10T12-00-00", thread, &base_rows);
    let mut meta = codex_meta_at(thread, continuation_cwd, "2026-11-10T13:00:00.000Z");
    // The first rollout's immutable rollout id is the thread id: its file name has no suffix.
    meta["payload"]["history_base"] = json!({"thread_id":thread,
        "end_ordinal_exclusive":boundary,"end_byte_offset":end_byte_offset});
    let continuation_rows = vec![
        meta,
        codex_user_at("2026-11-10T13:01:00.000Z", "replacement question codexcontinuationneedle"),
        codex_reply_at("2026-11-10T13:02:00.000Z", thread, "reply-3", "replacement answer"),
    ];
    let continuation = codex_rollout(
        root,
        "2026-11-10T13-00-00",
        &format!("{thread}_{rollout}"),
        &continuation_rows,
    );
    RevertedThread { base, continuation, boundary, base_rows, continuation_rows }
}

/// One rollout file of a Codex thread, written the way the tool names and stamps them.
pub struct Rollout {
    /// The id other rollouts name in `history_base.thread_id`: the suffix after `_`,
    /// or the thread id for the thread's first, plain-named rollout.
    pub id: String,
    pub path: PathBuf,
    pub rows: Vec<Value>,
    /// How many records this rollout's history holds before its own first record:
    /// what it inherits through `history_base`, counted along the whole chain.
    pub inherited: u64,
}

pub struct RolloutSpec<'a> {
    pub thread: &'a str,
    /// `Some(id)` names the file `<thread>_<id>`, as the tool does after a revert.
    pub rollout: Option<&'a str>,
    /// Hour in the file name (`rollout-2026-11-10T<hour>-00-00-…`).
    pub name_hour: u8,
    /// Hour of the record times; a turn `i` is asked at minute `1 + 2i`, answered at `2 + 2i`.
    pub clock_hour: u8,
    pub cwd: &'a str,
    /// The rollout this one inherits from and how many records of that rollout's own
    /// file it keeps (on top of whatever that rollout itself inherits).
    pub base: Option<(&'a Rollout, u64)>,
    /// Extra `session_meta.payload` fields (`forked_from_id`, `parent_thread_id`, …).
    pub meta: Value,
    /// One user message per turn; each is followed by a reply.
    pub turns: &'a [&'a str],
}

pub fn codex_thread_rollout(root: &Path, spec: RolloutSpec<'_>) -> Rollout {
    let at = |minute: usize| format!("2026-11-10T{:02}:{minute:02}:00.000Z", spec.clock_hour);
    let mut meta = codex_meta_at(spec.thread, spec.cwd, &at(0));
    // As the tool writes it (all 70 local rollouts that name a base, 27 of them naming a
    // base that itself inherits): the ordinal counts along the base's whole history,
    // inherited part included; the byte offset is a position in the base's own file.
    let mut inherited = 0;
    if let Some((base, kept)) = spec.base {
        inherited = base.inherited + kept;
        meta["payload"]["history_base"] = json!({"thread_id":base.id,
            "end_ordinal_exclusive":inherited,
            "end_byte_offset":jsonl(&base.rows[..kept as usize]).len()});
    }
    if let Some(extra) = spec.meta.as_object() {
        for (key, value) in extra {
            meta["payload"][key] = value.clone();
        }
    }
    let id = spec.rollout.unwrap_or(spec.thread).to_owned();
    let mut rows = vec![meta];
    for (index, text) in spec.turns.iter().enumerate() {
        rows.push(codex_user_at(&at(1 + 2 * index), text));
        rows.push(codex_reply_at(
            &at(2 + 2 * index),
            spec.thread,
            &format!("reply-{id}-{index}"),
            "synthetic answer",
        ));
    }
    let name = match spec.rollout {
        Some(rollout) => format!("{}_{rollout}", spec.thread),
        None => spec.thread.to_owned(),
    };
    let time = format!("2026-11-10T{:02}-00-00", spec.name_hour);
    let path = codex_rollout(root, &time, &name, &rows);
    Rollout { id, path, rows, inherited }
}

pub fn deepseek_header(id: &str, version: u64, cwd: &str) -> Value {
    json!({"type":"session","version":version,"id":id,"createdAt":EPOCH,"cwd":cwd,
        "isSeeded":false,"delegationDepth":0})
}

pub fn deepseek_user(seq: u64, text: &str) -> Value {
    json!({"type":"user/message","seq":seq,"time":EPOCH + seq as i64 + 1,"surfaceOp":"append",
        "data":{"id":format!("input-{seq}"),"role":"user","source":{"kind":"user"},
            "content":[{"type":"text","text":text}]}})
}

/// One independent zstd frame, as DeepSeek appends them to `session.vN.jsonl.zstd`.
pub fn zstd_frame(rows: &[Value]) -> Vec<u8> {
    zstd::stream::encode_all(jsonl(rows).as_slice(), 1).unwrap()
}

pub fn deepseek_folder(root: &Path, id: &str) -> PathBuf {
    root.join(format!("sessions/--not-a-reversible-path--/{id}"))
}

/// Generation 3 as plain JSONL and generation 4 as zstd frames of one session.
pub fn deepseek_generations(root: &Path, id: &str, cwd: &str, text: &str) -> (PathBuf, PathBuf) {
    let folder = deepseek_folder(root, id);
    let old = folder.join("session.v3.jsonl");
    write(&old, &jsonl(&[deepseek_header(id, 3, cwd), deepseek_user(0, text)]));
    let current = folder.join("session.v4.jsonl.zstd");
    let mut bytes = zstd_frame(&[deepseek_header(id, 4, cwd)]);
    bytes.extend(zstd_frame(&[deepseek_user(0, text)]));
    write(&current, &bytes);
    (old, current)
}

pub fn continue_entry(role: &str, text: &str) -> Value {
    json!({"message":{"role":role,"content":text},"contextItems":[]})
}

/// A Continue session, a document the tool rewrites as a whole.
pub fn continue_session(root: &Path, id: &str, history: &[Value]) -> PathBuf {
    let path = root.join(format!("sessions/{id}.json"));
    let document = json!({"history":history,"workspaceDirectory":"/synthetic/continue",
        "sessionId":id,"chatModelTitle":"synthetic model","title":"Synthetic document",
        "mode":"agent"});
    write(&path, document.to_string().as_bytes());
    path
}

/// A `.git` directory with `HEAD` on `branch` and the given remote URLs in `config`.
pub fn git_repository(top: &Path, branch: &str, remotes: &[(&str, &str)]) -> PathBuf {
    let git = top.join(".git");
    fs::create_dir_all(git.join("refs/heads")).unwrap();
    fs::create_dir_all(git.join("objects")).unwrap();
    write(&git.join("HEAD"), format!("ref: refs/heads/{branch}\n").as_bytes());
    write(&git.join("config"), git_config(remotes).as_bytes());
    git
}

pub fn git_config(remotes: &[(&str, &str)]) -> String {
    let mut config =
        String::from("[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n");
    for (name, url) in remotes {
        config.push_str(&format!(
            "[remote \"{name}\"]\n\turl = {url}\n\tfetch = +refs/heads/*:refs/remotes/{name}/*\n"
        ));
    }
    config
}

/// A linked worktree in git's layout: `<top>/.git` is a file naming
/// `<main>/worktrees/<name>`, which holds `HEAD`, `commondir` and `gitdir`.
pub fn git_worktree(main_git: &Path, name: &str, top: &Path, branch: &str) {
    let admin = main_git.join("worktrees").join(name);
    write(&admin.join("HEAD"), format!("ref: refs/heads/{branch}\n").as_bytes());
    write(&admin.join("commondir"), b"../..\n");
    write(&admin.join("gitdir"), format!("{}\n", top.join(".git").display()).as_bytes());
    write(&top.join(".git"), format!("gitdir: {}\n", admin.display()).as_bytes());
}
