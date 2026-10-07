//! Responsibilities: adapt root-based search export calls to the owner engine.
//! Not responsible for: timeline reads, storage ownership or search implementation.
//! Failure modes: lost search filters, cancellation ignored, legacy database opens.
use super::*;
use crate::{ExportRequest, ExportResult};

pub fn export_search_history(
    paths: &ProjectPaths,
    _config: &AppConfig,
    _key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    export::finish_export(request, |request, job| {
        export::write_export(paths, request, job, None, |_| {
            anyhow::bail!("search export requires a search query")
        })
    })
}
