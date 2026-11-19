//! Metadata inventory for the desktop source picker. Layout identifies candidate
//! files only. Parsing, attachment discovery from records, and capture belong to
//! the adapters and require separate consent.
use crate::{Error, Harness, Result, SourceRoot};
use serde::{Serialize, Serializer};
use std::{fs, io, path::Path};
use vault_store::CancelToken;

fn decimal<S: Serializer>(value: &u64, serializer: S) -> std::result::Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// Unrestricted counts are decimal strings on the wire.
#[derive(Default, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryCounts {
    #[serde(serialize_with = "decimal")]
    pub folders: u64,
    #[serde(serialize_with = "decimal")]
    pub files: u64,
    #[serde(serialize_with = "decimal")]
    pub source_bytes: u64,
    #[serde(serialize_with = "decimal")]
    pub compressed_bytes: u64,
    #[serde(serialize_with = "decimal")]
    pub top_level_files: u64,
    #[serde(serialize_with = "decimal")]
    pub child_files: u64,
    #[serde(serialize_with = "decimal")]
    pub ambiguous_files: u64,
    #[serde(serialize_with = "decimal")]
    pub ancillary_files: u64,
    #[serde(serialize_with = "decimal")]
    pub unknown_files: u64,
}

impl InventoryCounts {
    pub fn add(&mut self, other: &Self) {
        self.folders += other.folders;
        self.files += other.files;
        self.source_bytes += other.source_bytes;
        self.compressed_bytes += other.compressed_bytes;
        self.top_level_files += other.top_level_files;
        self.child_files += other.child_files;
        self.ambiguous_files += other.ambiguous_files;
        self.ancillary_files += other.ancillary_files;
        self.unknown_files += other.unknown_files;
    }
}

/// One independent reading so an inaccessible tool cannot erase a healthy sibling.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootInventory {
    pub root_id: String,
    pub tool: Harness,
    pub path: std::path::PathBuf,
    pub readable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    pub counts: InventoryCounts,
    pub count_basis: &'static str,
    pub date_basis: &'static str,
    pub earliest_at: Option<i64>,
    pub latest_at: Option<i64>,
}

/// Presence needs directory access but never enumerates children.
pub fn root_status(path: &Path) -> Option<&'static str> {
    if path.components().any(|c| c.as_os_str().eq_ignore_ascii_case(".git"))
        || super::normalized_root(path)
            .is_ok_and(|p| p.components().any(|c| c.as_os_str().eq_ignore_ascii_case(".git")))
    {
        return Some("scope-denied");
    }
    let status = fs::symlink_metadata(path).and_then(|meta| {
        if !meta.is_dir() {
            return Err(io::ErrorKind::NotADirectory.into());
        }
        super::directory_handle(path).map(drop)
    });
    match status {
        Ok(()) => None,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Some("root-missing"),
        Err(_) => Some("root-unreadable"),
    }
}

/// Streaming consumers may inspect the exact native filenames included in the
/// byte estimate. The desktop only needs the progress callback and totals.
pub trait InventoryObserver {
    fn progress(&mut self, visited: u64);
    fn candidate(&mut self, _file: &crate::Artifact) {}
}
impl<F: FnMut(u64)> InventoryObserver for F {
    fn progress(&mut self, visited: u64) {
        self(visited)
    }
}

/// Enumerates only native history trees. Each open directory owns one iterator;
/// the depth cap bounds memory and descriptors even in a malformed layout.
pub fn inventory_root(
    source: &SourceRoot,
    cancel: &CancelToken,
    progress: impl InventoryObserver,
) -> Result<RootInventory> {
    let pinned = super::directory_handle(&source.root).ok().map(std::sync::Arc::new);
    inventory_root_bound(source, pinned, cancel, progress)
}

/// The picker retains the same directory object that supplies its receipt.
/// Changing the path during the walk cannot redirect metadata enumeration.
pub fn inventory_root_bound(
    source: &SourceRoot,
    pinned: Option<std::sync::Arc<same_file::Handle>>,
    cancel: &CancelToken,
    observer: impl InventoryObserver,
) -> Result<RootInventory> {
    if cancel.check().is_err() {
        return Err(Error::Cancelled);
    }
    let reason = root_status(&source.root).or_else(|| pinned.is_none().then_some("root-changed"));
    let mut row = RootInventory {
        root_id: super::root_identity(source)?,
        tool: source.harness,
        path: source.root.clone(),
        readable: reason.is_none(),
        reason,
        counts: Default::default(),
        count_basis: if reason.is_some() {
            "unavailable"
        } else if source.harness == Harness::ClaudeCode {
            "known-layout"
        } else {
            "file-candidates"
        },
        date_basis: "filesystem-modified",
        earliest_at: None,
        latest_at: None,
    };
    if reason.is_some() {
        return Ok(row);
    }
    if !matches!(
        source.harness,
        Harness::ClaudeCode | Harness::Codex | Harness::Pi | Harness::Deepseek
    ) {
        return Err(Error::InvalidInput("unsupported desktop tool".into()));
    }
    let delay = super::debug_delay::configured("PATHKEEP_TEST_AGENT_PREVIEW_ENTRY_DELAY_US");
    let observer = std::cell::RefCell::new(observer);
    let mut visited = 0;
    let visit = std::cell::RefCell::new(|| {
        visited += 1;
        observer.borrow_mut().progress(visited);
        super::debug_delay::wait(delay, cancel)
    });
    let tick = || visit.borrow_mut()();
    let mut tree = super::Discovery::new(cancel, pinned.map(|h| (source.root.clone(), h)));
    tree.progress = Some(&tick);
    let mut issues = Vec::new();
    crate::adapters::adapter(source.harness).discover_cancellable(
        &tree,
        source,
        &mut |file| {
            let Ok(meta) = tree.metadata(&file.path) else {
                row.reason = Some("root-unreadable");
                return Ok(());
            };
            if !meta.is_file() {
                row.reason = Some("root-changed");
                return Ok(());
            }
            observer.borrow_mut().candidate(&file);
            classify(&mut row.counts, source, &file);
            row.counts.files += 1;
            row.counts.source_bytes += meta.len();
            if matches!(file.path.extension().and_then(|s| s.to_str()), Some("zst" | "zstd")) {
                row.counts.compressed_bytes += meta.len()
            }
            if let Ok(time) = meta.modified() {
                let ms = chrono::DateTime::<chrono::Utc>::from(time.into_std()).timestamp_millis();
                row.earliest_at = Some(row.earliest_at.map_or(ms, |old| old.min(ms)));
                row.latest_at = Some(row.latest_at.map_or(ms, |old| old.max(ms)));
            }
            Ok(())
        },
        &mut issues,
    )?;
    (row.counts.folders, row.counts.unknown_files) = tree.inventory_counts();
    for issue in issues {
        if issue.code.contains("unreadable") || issue.code.contains("unavailable") {
            row.reason = Some("root-unreadable")
        } else if issue.code.contains("depth") {
            row.reason = Some("source-depth-limit")
        } else if issue.code.contains("utf8") {
            row.reason = Some("unsupported-format")
        }
    }
    Ok(row)
}

fn classify(counts: &mut InventoryCounts, source: &SourceRoot, file: &crate::Artifact) {
    if file.kind != crate::ArtifactKind::Transcript {
        counts.ancillary_files += 1;
        return;
    }
    if source.harness != Harness::ClaudeCode {
        counts.ambiguous_files += 1;
        return;
    }
    let path = file.relative_path.as_str();
    let project_path = if source.root.ends_with("projects") {
        Some(path)
    } else {
        path.strip_prefix("projects/")
            .or_else(|| path.split_once("/.claude/projects/").map(|(_, tail)| tail))
    };
    match project_path {
        Some(relative) if relative.split('/').nth(2) == Some("subagents") => {
            counts.child_files += 1
        }
        Some(_)
            if !file
                .path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("agent-")) =>
        {
            counts.top_level_files += 1
        }
        _ => counts.ambiguous_files += 1,
    }
}
