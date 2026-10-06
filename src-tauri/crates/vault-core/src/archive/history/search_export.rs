//! Responsibilities: hand search exports to the existing area-3 recall function.
//! Not responsible for: timeline reads or export formatting.
//! Failure modes: search filters lost, cancellation ignored; legacy search tests retain coverage.
use super::*;
use crate::{ExportRequest, ExportResult};

pub fn export_search_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    export::finish_export(request, |request, job| {
        let connection = open_archive_connection(paths, config, key)?;
        connection.pragma_update(None, "temp_store", "FILE")?;
        if let Some(job) = job {
            let job = std::sync::Arc::clone(job);
            connection.progress_handler(1000, Some(move || job.is_cancelled()))?;
        }
        export::write_export(paths, request, job, None, |query| {
            list_history_on_connection(&connection, query)
        })
    })
}
