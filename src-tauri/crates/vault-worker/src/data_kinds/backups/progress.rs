//! Live progress counts committed native units. Identity announcements come from
//! the run-local unique insert. A first save has no known denominator.
use agent_history::capture::BackupProgress;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Units {
    found: u64,
    files: u64,
    bytes: u64,
    roots: u64,
    updated: u64,
    children: u64,
}
impl Units {
    fn apply(&mut self, p: &BackupProgress) {
        match p.phase.as_str() {
            "file_found" => self.found += 1,
            "raw_committed" => {
                self.files += 1;
                self.bytes += p.bytes;
            }
            "new-root" => self.roots += 1,
            "updated-root" => self.updated += 1,
            "new-child" => self.children += 1,
            _ => {}
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolProgress<'a> {
    tool: &'a str,
    files_found: String,
    files_committed: String,
    bytes_committed: String,
    new_roots: String,
    updated_roots: String,
    new_children: String,
}

#[derive(Default)]
pub(super) struct Progress {
    sum: Units,
    tools: BTreeMap<&'static str, Units>,
    last: Option<Instant>,
    first_commit: bool,
}
impl Progress {
    pub fn advance(&mut self, p: &BackupProgress) -> Option<Value> {
        self.sum.apply(p);
        if let Some(tool) = p.harness {
            self.tools.entry(tool.as_str()).or_default().apply(p);
        }
        let commit = p.phase == "raw_committed" && !self.first_commit;
        self.first_commit |= commit;
        if !commit
            && p.phase != "finishing"
            && self.last.is_some_and(|last| last.elapsed() < Duration::from_millis(100))
        {
            return None;
        }
        let (phase, unit, done) = match p.phase.as_str() {
            "file_found" if self.first_commit => ("capture", "files", self.sum.files),
            "file_found" => ("discovery", "files", self.sum.found),
            "raw_committed" => ("capture", "bytes", self.sum.bytes),
            "new-root" | "updated-root" => {
                ("capture", "conversations", self.sum.roots + self.sum.updated)
            }
            "new-child" => ("capture", "children", self.sum.children),
            "index" | "finishing" => (p.phase.as_str(), "files", self.sum.files),
            _ => return None,
        };
        self.last = Some(Instant::now());
        let per_tool = self
            .tools
            .iter()
            .map(|(tool, u)| ToolProgress {
                tool,
                files_found: u.found.to_string(),
                files_committed: u.files.to_string(),
                bytes_committed: u.bytes.to_string(),
                new_roots: u.roots.to_string(),
                updated_roots: u.updated.to_string(),
                new_children: u.children.to_string(),
            })
            .collect::<Vec<_>>();
        Some(
            json!({"phase":phase,"unit":unit,"done":done.to_string(),"total":null,"perTool":per_tool}),
        )
    }
}
