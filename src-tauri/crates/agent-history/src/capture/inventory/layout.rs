//! Metadata classifications for the product picker, including unrecognized files
//! inside transcript trees. Native content interpretation belongs to the adapters.
use crate::{
    Error, Harness, Result, SourceRoot,
    adapters::{codex, deepseek},
};
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum Entry {
    TopLevel,
    Child,
    Ambiguous,
    Ancillary,
    Unknown,
}

pub(super) enum Layout {
    Claude(Option<String>),
    Codex,
    Pi(&'static [&'static str]),
    Deepseek(bool),
}

impl Layout {
    pub(super) fn for_source(
        source: &SourceRoot,
        tree: &crate::capture::Discovery,
    ) -> Result<Self> {
        let root = &source.root;
        Ok(match source.harness {
            Harness::ClaudeCode => {
                let name = root.file_name().and_then(|s| s.to_str()).filter(|s| {
                    matches!(
                        *s,
                        "projects"
                            | "claude-code-sessions"
                            | "local-agent-mode-sessions"
                            | "file-history"
                            | "uploads"
                            | "image-cache"
                            | "paste-cache"
                            | "images"
                    )
                });
                Self::Claude(name.map(str::to_owned))
            }
            Harness::Codex => Self::Codex,
            Harness::Pi => {
                let prefix: &[&str] =
                    if tree.metadata(&root.join("agent")).is_ok_and(|m| m.is_dir())
                        && tree.metadata(&root.join("agent/sessions")).is_ok_and(|m| m.is_dir())
                    {
                        &["agent", "sessions"]
                    } else if tree.metadata(&root.join("sessions")).is_ok_and(|m| m.is_dir())
                        || root.ends_with(".pi/agent")
                    {
                        &["sessions"]
                    } else {
                        &[]
                    };
                Self::Pi(prefix)
            }
            Harness::Deepseek => Self::Deepseek(
                tree.metadata(&root.join("sessions")).is_ok_and(|m| m.is_dir())
                    || root.ends_with(".dsh"),
            ),
            _ => return Err(Error::InvalidInput("unsupported desktop tool".into())),
        })
    }
    pub(super) fn classify(&self, path: &[&str], dir: bool) -> Option<Entry> {
        use Entry::*;
        let name = *path.last()?;
        match self {
            Self::Claude(prefix) => match prefix {
                Some(first) => claude(first, path, dir),
                None => claude(path[0], &path[1..], dir),
            },
            Self::Codex => match path[0] {
                "sessions" | "archived_sessions" => {
                    Some(if codex::is_rollout(Path::new(name)) { Ambiguous } else { Unknown })
                }
                "attachments" | "generated_images" => Some(Ancillary),
                "session_index.jsonl" | "history.jsonl" if path.len() == 1 && !dir => {
                    Some(Ancillary)
                }
                _ if path.len() == 1 && !dir && codex::is_database_name(name) => Some(Ancillary),
                _ => None,
            },
            Self::Pi(prefix) => {
                if prefix.starts_with(path) && dir {
                    return Some(Unknown);
                }
                if !path.starts_with(prefix) {
                    return None;
                }
                Some(if name.ends_with(".jsonl") { Ambiguous } else { Unknown })
            }
            Self::Deepseek(home) => {
                if *home && path[0] != "sessions" {
                    return match path {
                        ["attachments"] | ["attachments", "v1"] if dir => Some(Ancillary),
                        ["attachments", "v1", "objects" | "file-objects", ..] => Some(Ancillary),
                        _ => None,
                    };
                }
                Some(if deepseek::generation_number(name).is_some() { Ambiguous } else { Unknown })
            }
        }
    }
}

fn claude(tree: &str, parts: &[&str], dir: bool) -> Option<Entry> {
    use Entry::*;
    match tree {
        "projects" => {
            if matches!(parts, [_, "memory", ..]) {
                return None;
            }
            let name = parts.last().copied().unwrap_or_default();
            let transcript = name.ends_with(".jsonl") || name.contains(".jsonl.superseded-");
            Some(match parts {
                [_, _] if transcript && !name.starts_with("agent-") => TopLevel,
                [_, _] if transcript => Ambiguous,
                [_, _, "subagents", ..] if transcript && name.starts_with("agent-") => Child,
                [_, _, _, ..] => Ancillary,
                _ if name.ends_with(".json") => Ancillary,
                _ => Unknown,
            })
        }
        "local-agent-mode-sessions" => match parts {
            [] | [_] | [_, _] if dir => Some(Ancillary),
            [_, _, id, tail @ ..] if id.starts_with("local_") => match tail {
                [] if dir || id.ends_with(".json") => Some(Ancillary),
                [".claude"] if dir => Some(Ancillary),
                [".claude", tree, rest @ ..]
                    if matches!(
                        *tree,
                        "projects"
                            | "file-history"
                            | "paste-cache"
                            | "image-cache"
                            | "uploads"
                            | "plans"
                    ) =>
                {
                    claude(tree, rest, dir)
                }
                ["audit.jsonl"] => Some(Ambiguous),
                ["uploads" | "outputs" | "tasks", ..] | [".audit-key"] => Some(Ancillary),
                _ => None,
            },
            _ => None,
        },
        "claude-code-sessions"
        | "file-history"
        | "uploads"
        | "image-cache"
        | "paste-cache"
        | "images"
        | "plans"
        | "tasks"
        | "history.jsonl" => Some(Ancillary),
        _ => None,
    }
}
