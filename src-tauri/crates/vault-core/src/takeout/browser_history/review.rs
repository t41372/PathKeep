//! Responsibilities: preserve file reports, parser warnings and failure notes for partial browser imports.
//! Not responsible for: import run state, parsing, progress totals or successful finalization.
//! Failure modes: early errors lose files, late errors lose warnings, saving a review masks the original error.
//! Tests: failed_browser_import_retains_recognized_files_and_failure_notes and review_failure_keeps_the_original_error_and_type.
use super::*;
pub(super) fn save_failure(
    vault: &Vault,
    batch_id: i64,
    source: &str,
    review: Option<TakeoutInspection>,
    error: &anyhow::Error,
) -> Result<()> {
    let mut inspection = match review {
        Some(review) => review,
        None => {
            let detail = vault.read(Db::Archive, move |s| {
                super::super::batch_review::load_import_batch_detail(s, batch_id)
            })?;
            TakeoutInspection {
                source_path: source.into(),
                recognized_files: detail.recognized_files,
                quarantined_files: detail.quarantined_files,
                notes: detail.notes,
                note_details: detail.note_details,
                preview_range_start: detail.preview_range_start,
                preview_range_end: detail.preview_range_end,
                ..Default::default()
            }
        }
    };
    inspection.push_note(super::super::inspect::payload_parse_failed_note(source, error));
    for file in &mut inspection.recognized_files {
        file.status = "parse-error".into();
    }
    vault.write(Db::Archive, Priority::Batch, move |s| {
        batches::save_partial_review(s, batch_id, &inspection)
    })
}
