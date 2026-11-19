//! Keep the most recently reported file counts and current activity for late subscribers.
//! The transfer registry owns the lifetime; querying progress performs no archive I/O.

use crate::{ModelDownloadFileBytes, ModelDownloadPhase, ModelDownloadState, ModelDownloadUpdate};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Progress {
    pub(super) phase: ModelDownloadPhase,
    current_file: Option<String>,
    files: BTreeMap<String, ModelDownloadFileBytes>,
}

impl Progress {
    pub(super) fn record(&mut self, update: &ModelDownloadUpdate) {
        match update {
            ModelDownloadUpdate::Waiting { reason } => {
                self.phase = ModelDownloadPhase::Waiting { reason: *reason };
                return;
            }
            ModelDownloadUpdate::FileStarted { file, total_bytes } => {
                self.current_file = Some(file.clone());
                self.files.insert(
                    file.clone(),
                    ModelDownloadFileBytes { downloaded_bytes: 0, total_bytes: *total_bytes },
                );
            }
            ModelDownloadUpdate::FileProgress { file, downloaded_bytes, total_bytes } => {
                let counts = self.files.entry(file.clone()).or_default();
                counts.downloaded_bytes = *downloaded_bytes;
                if *total_bytes > 0 {
                    counts.total_bytes = *total_bytes;
                }
            }
            // The stream reports its final byte count before verification; preserve that count
            // even when the HTTP response did not supply a content length.
            ModelDownloadUpdate::FileFinished { .. } => self.current_file = None,
            ModelDownloadUpdate::Done
            | ModelDownloadUpdate::Cancelled
            | ModelDownloadUpdate::Error { .. } => return,
        }
        self.phase = ModelDownloadPhase::Downloading;
    }

    pub(super) fn snapshot(&self, transfer_id: &str) -> ModelDownloadState {
        ModelDownloadState {
            transfer_id: transfer_id.into(),
            phase: self.phase,
            current_file: self.current_file.clone(),
            downloaded_bytes: self
                .files
                .values()
                .fold(0_u64, |sum, file| sum.saturating_add(file.downloaded_bytes)),
            total_bytes: self
                .files
                .values()
                .fold(0_u64, |sum, file| sum.saturating_add(file.total_bytes)),
            files: self.files.clone(),
        }
    }
}
