//! Metadata inventory for the desktop source picker. Layout identifies candidate
//! files only. Parsing, attachment discovery from records, and capture belong to
//! the adapters and require separate consent.
use crate::{Error, Harness, Result, SourceRoot};
use serde::{Serialize, Serializer};
use std::{fs, io, path::Path};
use vault_store::CancelToken;
mod layout;
use layout::{Entry, Layout};

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

/// Enumerates only native history trees. Each open directory owns one iterator;
/// the depth cap bounds memory and descriptors even in a malformed layout.
pub fn inventory_root(
    source: &SourceRoot,
    cancel: &CancelToken,
    progress: impl FnMut(u64),
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
    mut progress: impl FnMut(u64),
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
    let tree = super::Discovery::new(cancel, pinned.map(|handle| (source.root.clone(), handle)));
    let layout = Layout::for_source(source, &tree)?;
    let entries = match tree.read_dir(&source.root) {
        Ok(entries) => entries,
        Err(_) => {
            row.reason = Some("root-unreadable");
            row.readable = false;
            row.count_basis = "unavailable";
            return Ok(row);
        }
    };
    let delay = super::debug_delay::configured("PATHKEEP_TEST_AGENT_PREVIEW_ENTRY_DELAY_US");
    let mut stack = vec![entries];
    let mut visited = 0;
    while let Some(dir) = stack.last_mut() {
        let Some(entry) = dir.next() else {
            stack.pop();
            continue;
        };
        visited += 1;
        progress(visited);
        super::debug_delay::wait(delay, cancel)?;
        if cancel.check().is_err() {
            return Err(Error::Cancelled);
        }
        let Ok(entry) = entry else {
            row.reason = Some("root-unreadable");
            continue;
        };
        // Check the name before asking for its type or opening a directory handle.
        if entry.file_name().eq_ignore_ascii_case(".git") {
            continue;
        }
        let Ok(ty) = entry.file_type() else {
            row.reason = Some("root-unreadable");
            continue;
        };
        if !(ty.is_dir() || ty.is_file()) {
            continue;
        }
        let path = entry.path();
        let relative = path.strip_prefix(&source.root).expect("child of selected root");
        let Some(parts) =
            relative.components().map(|c| c.as_os_str().to_str()).collect::<Option<Vec<_>>>()
        else {
            row.reason = Some("unsupported-format");
            continue;
        };
        let Some(class) = layout.classify(&parts, ty.is_dir()) else { continue };
        if ty.is_dir() {
            row.counts.folders += 1;
            if stack.len() >= 64 {
                row.reason = Some("source-depth-limit");
                continue;
            }
            match tree.read_dir(&path) {
                Ok(child) => stack.push(child),
                Err(_) => row.reason = Some("root-unreadable"),
            }
        } else {
            let Ok(meta) = entry.metadata() else {
                row.reason = Some("root-unreadable");
                continue;
            };
            match class {
                Entry::TopLevel => row.counts.top_level_files += 1,
                Entry::Child => row.counts.child_files += 1,
                Entry::Ambiguous => row.counts.ambiguous_files += 1,
                Entry::Ancillary => row.counts.ancillary_files += 1,
                Entry::Unknown => row.counts.unknown_files += 1,
            }
            row.counts.files += 1;
            row.counts.source_bytes += meta.len();
            if matches!(path.extension().and_then(|s| s.to_str()), Some("zst" | "zstd")) {
                row.counts.compressed_bytes += meta.len()
            }
            if let Ok(modified) = meta.modified() {
                let ms =
                    chrono::DateTime::<chrono::Utc>::from(modified.into_std()).timestamp_millis();
                row.earliest_at = Some(row.earliest_at.map_or(ms, |old| old.min(ms)));
                row.latest_at = Some(row.latest_at.map_or(ms, |old| old.max(ms)));
            }
        }
    }
    Ok(row)
}
