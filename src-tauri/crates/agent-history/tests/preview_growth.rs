//! Calibrate the discovery-only growth estimate from committed, closed archive roles.
//! Fixture generation and native input files are outside the measured archive.
#[allow(dead_code)]
mod support;
use agent_history::{Archive, Harness, SourceRoot};
use serde_json::json;
use std::{fs, io::Write, path::Path};
use support::{
    fixtures::{jsonl, pi_message},
    native::*,
};
use vault_layout::{ArtifactId, path};

fn content(seed: u64) -> String {
    const WORDS: &[&str] = &[
        "source", "output", "impl", "checkout", "model", "select", "reader", "session", "count",
        "return", "assert", "Result", "scope", "index", "page", "tools", "record", "query", "fn",
        "input",
    ];
    let mut n = seed + 1;
    let mut s = format!("fixture {seed}\n");
    for i in 0..140 {
        n = n.wrapping_mul(6364136223846793005).wrapping_add(1);
        s.push_str(WORDS[(n >> 32) as usize % WORDS.len()]);
        if i % 7 == 0 {
            s.push_str(&format!("_{:05x}", n >> 44));
        }
        s.push(if i % 12 == 0 { '\n' } else { ' ' });
    }
    s
}
fn sources(root: &Path, zst: bool) -> Vec<SourceRoot> {
    let tools = if zst {
        vec![Harness::Codex, Harness::Deepseek]
    } else {
        vec![Harness::Codex, Harness::Deepseek, Harness::ClaudeCode, Harness::Pi]
    };
    let mut roots = Vec::new();
    for harness in tools {
        let base = root.join(harness.as_str());
        for i in 0..16 {
            let id = format!("11111111-1111-4111-8111-{i:012}");
            let mut rows = match harness {
                Harness::Codex => vec![codex_meta(&id, "/bench")],
                Harness::Deepseek => vec![deepseek_header(&id, 4, "/bench")],
                Harness::Pi => vec![
                    json!({"type":"session","id":id,"version":3,"cwd":"/bench","timestamp":TIME}),
                ],
                _ => Vec::new(),
            };
            for n in 0..96 {
                let text = content(i * 100 + n);
                let key = format!("{n:08x}");
                let parent = n.checked_sub(1).map(|prev| format!("{prev:08x}"));
                let user = n % 2 == 0;
                let row = match harness {
                    Harness::ClaudeCode if user => {
                        claude_user(&id, &key, parent.as_deref(), "/bench", &text)
                    }
                    Harness::ClaudeCode => {
                        claude_reply(&id, &key, parent.as_deref().unwrap(), "/bench", &text)
                    }
                    Harness::Pi => pi_message(
                        &key,
                        parent.as_deref(),
                        if user { "user" } else { "assistant" },
                        &text,
                    ),
                    Harness::Codex if user => codex_user(&text),
                    Harness::Codex => codex_reply(&id, &key, &text),
                    _ if user => deepseek_user(n, &text),
                    _ => {
                        json!({"type":"assistant/message","seq":n,"time":EPOCH+n as i64,"surfaceOp":"append",
                        "data":{"turn":n/2,"step":1,"stream":[],"message":{"id":key,"role":"assistant",
                        "content":[{"type":"text","text":text}]},"usage":{"inputTokens":10,"cacheReadTokens":2,"outputTokens":3}}})
                    }
                };
                rows.push(row);
            }
            let file = match harness {
                Harness::ClaudeCode => base.join(format!("{CLAUDE_PROJECT}/{id}.jsonl")),
                Harness::Pi => base.join(format!("sessions/{id}.jsonl")),
                Harness::Deepseek => deepseek_folder(&base, &id).join("session.jsonl"),
                _ => {
                    base.join(format!("sessions/2026/11/10/rollout-2026-11-10T12-00-00-{id}.jsonl"))
                }
            };
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            let raw = jsonl(&rows);
            if zst {
                let suffix = if harness == Harness::Deepseek { "jsonl.zstd" } else { "jsonl.zst" };
                fs::write(
                    file.with_extension(suffix),
                    zstd::encode_all(raw.as_slice(), 3).unwrap(),
                )
                .unwrap();
            } else {
                fs::write(file, raw).unwrap();
            }
        }
        roots.push(SourceRoot { harness, root: base });
    }
    roots
}

fn bytes(p: &Path) -> u64 {
    if !p.exists() {
        return 0;
    }
    if p.is_file() {
        return p.metadata().unwrap().len();
    }
    fs::read_dir(p).unwrap().map(|f| bytes(&f.unwrap().path())).sum()
}
fn sizes(root: &Path) -> [u64; 3] {
    [
        ArtifactId::AgentHistoryEvidence,
        ArtifactId::AgentHistoryReadModel,
        ArtifactId::AgentHistorySearch,
    ]
    .map(|role| bytes(&path(root, role)))
}
#[test]
#[ignore = "Linux release calibration of ordinary and native-zstd source bytes"]
fn measure_preview_growth() {
    for zst in [false, true] {
        for encrypted in [false, true] {
            let t = tempfile::tempdir().unwrap();
            let root = t.path().join("archive");
            let source = t.path().join("sources");
            let inputs = sources(&source, zst);
            let a = if encrypted {
                Archive::create_with_params(
                    &root,
                    b"fixture",
                    vault_crypto::KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
                )
                .unwrap()
            } else {
                Archive::create_unencrypted(&root).unwrap()
            };
            a.synchronize_search().unwrap();
            drop(a);
            let before = sizes(&root);
            let a = Archive::open(&root, if encrypted { b"fixture" } else { b"" }).unwrap();
            let report = a.backup(&inputs, &Default::default()).unwrap();
            let count = a.chat_counts(&Default::default()).unwrap();
            assert_eq!(count["savedRoots"], inputs.len() * 16, "{report:?}");
            drop(a);
            let after = sizes(&root);
            let delta = std::array::from_fn::<_, 3, _>(|i| after[i] - before[i]);
            println!(
                "{}",
                json!({"encrypted":encrypted,"nativeZstd":zst,"sessions":inputs.len()*16,"messagesPerSession":96,"sourceBytes":bytes(&source),"emptyBytes":before,
            "evidenceBytes":delta[0],"readModelBytes":delta[1],"indexBytes":delta[2]})
            );
            std::io::stdout().flush().unwrap();
        }
    }
}
