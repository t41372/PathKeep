//! Aggregate complete per-root metadata, estimate growth, and issue a ticket
//! for the readable subset. The stream owns delivery and cancellation.
use super::{Binding, CAPACITY, LIFETIME, Permit, REGISTRY, Refusal, Ticket, directory_handle};
use agent_history::{
    SourceRoot,
    capture::{self, InventoryCounts},
};
use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;
use vault_core::{ArchiveMode, ProjectPaths};
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolInventory {
    #[serde(flatten)]
    counts: InventoryCounts,
    readable_roots: u32,
    roots: u32,
    earliest_at: Option<i64>,
    latest_at: Option<i64>,
    reasons: std::collections::BTreeSet<&'static str>,
}
impl ToolInventory {
    fn include(&mut self, row: &capture::RootInventory) {
        self.counts.add(&row.counts);
        self.roots += 1;
        self.readable_roots += u32::from(row.readable);
        self.earliest_at = self.earliest_at.into_iter().chain(row.earliest_at).min();
        self.latest_at = self.latest_at.into_iter().chain(row.latest_at).max();
        self.reasons.extend(row.reason);
    }
}

pub(super) fn enumerate(
    permit: &Permit,
    roots: Vec<SourceRoot>,
    mode: ArchiveMode,
    mut emit: impl FnMut(&str, Value) -> Result<()>,
) -> Result<()> {
    let mut rows = Vec::new();
    let mut by_tool = BTreeMap::<String, ToolInventory>::new();
    let mut totals = InventoryCounts::default();
    let mut approved = Vec::new();
    let mut elapsed = Instant::now();
    let mut visited = 0;
    emit("progress", json!({"phase":"discovery","unit":"entries","done":"0"}))?;
    for root in roots {
        permit.check()?;
        let mut stopped = None;
        let directory = directory_handle(&root.root).ok().map(Arc::new);
        let row = capture::inventory_root_bound(&root, directory.clone(), &permit.cancel, |_| {
            visited += 1u64;
            if elapsed.elapsed() >= Duration::from_millis(100) {
                if let Err(err) = emit(
                    "progress",
                    json!({"phase":"discovery","unit":"entries","done":visited.to_string()}),
                ) {
                    stopped = Some(err);
                    permit.cancel.cancel()
                }
                elapsed = Instant::now();
            }
        });
        if let Some(err) = stopped {
            return Err(err);
        }
        let mut row = row.map_err(|e| match e {
            agent_history::Error::Cancelled => anyhow::Error::from(Refusal::Cancelled),
            other => other.into(),
        })?;
        let mut token = None;
        if row.readable {
            if let Some(directory) = directory.filter(|d| {
                directory_handle(&root.root).is_ok_and(|current| &current == d.as_ref())
            }) {
                token = Some(super::directory_token(
                    &row.root_id,
                    &capture::directory_identity(&directory)?,
                ));
                approved.push(Binding {
                    root_id: row.root_id.clone(),
                    source: root.clone(),
                    canonical: capture::normalized_root(&root.root)?,
                    directory,
                });
            } else {
                row.readable = false;
                row.reason = Some("root-changed")
            }
        }
        totals.add(&row.counts);
        by_tool.entry(root.harness.to_string()).or_default().include(&row);
        let mut wire = serde_json::to_value(&row)?;
        if let Some(token) = token {
            wire["directoryToken"] = json!(token);
        }
        emit("root", wire.clone())?;
        rows.push(wire);
    }
    // Before onboarding the requested archive directory can still be absent.
    let available = target_available_bytes(&permit.paths, |volume| fs4::available_space(volume))?;
    let ordinary = totals.source_bytes - totals.compressed_bytes;
    // Planning allowance only: preserve native bytes, allow one extra copy of
    // input for derived rows and a page per file for database metadata.
    let overhead = totals.files.saturating_mul(4096);
    let derived = totals.source_bytes;
    let estimated = totals.source_bytes.saturating_add(derived).saturating_add(overhead);
    let id = Uuid::new_v4().to_string();
    permit.check()?;
    {
        let mut registry = REGISTRY.lock().unwrap();
        registry.prune();
        if registry.tickets.len() >= CAPACITY
            && let Some(oldest) = registry
                .tickets
                .iter()
                .min_by_key(|(_, ticket)| *ticket.permit.deadline.lock().unwrap())
                .map(|(key, _)| key.clone())
        {
            registry.tickets.remove(&oldest);
        }
        let ticket_permit = Permit {
            deadline: Arc::new(std::sync::Mutex::new(Instant::now() + LIFETIME)),
            ..permit.clone()
        };
        registry.tickets.insert(id.clone(), Ticket { permit: ticket_permit, roots: approved });
    }
    let ms = LIFETIME.as_millis() as i64;
    emit(
        "finished",
        json!({
            "previewId": id,
            "rows": rows,
            "totals": totals,
            "byTool": by_tool,
            "estimatedArchiveBytes": estimated.to_string(),
            "estimateBasis": {
                "method": "source-plus-derived-allowance-v1",
                "nativeCompressedBytes": totals.compressed_bytes.to_string(),
                "ordinaryBytes": ordinary.to_string(),
                "derivedAllowanceBytes": derived.to_string(),
                "overheadAllowanceBytes": overhead.to_string()
            },
            "availableBytes": available.to_string(),
            "archiveMode": mode,
            "expiresAt": chrono::Utc::now().timestamp_millis()+ms
        }),
    )
}

/// Probe the directory that will contain the archive, including before its creation.
fn target_available_bytes(
    paths: &ProjectPaths,
    measure: impl FnOnce(&Path) -> std::io::Result<u64>,
) -> Result<u64> {
    let evidence =
        std::path::absolute(paths.artifact_path(vault_layout::ArtifactId::AgentHistoryEvidence))?;
    let directory = evidence.parent().ok_or(Refusal::Destination)?;
    let volume = directory.ancestors().find(|p| p.exists()).ok_or(Refusal::Destination)?;
    measure(volume).map_err(|_| Refusal::Destination.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_volume_probe_uses_the_destination_before_and_after_setup() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("future/archive");
        let paths = vault_core::project_paths_with_root(&root);
        let archive_directory = root.join("archive");
        for expected in [temp.path(), root.as_path(), archive_directory.as_path()] {
            std::fs::create_dir_all(expected).unwrap();
            target_available_bytes(&paths, |path| {
                assert_eq!(path, expected);
                fs4::available_space(path)
            })
            .unwrap();
        }
    }
}
