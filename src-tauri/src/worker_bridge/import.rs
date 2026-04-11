//! Worker-bridge helpers for Takeout and import-batch flows.

use vault_core::TakeoutRequest;

use super::worker_result;

/// Inspects one Takeout source without mutating the archive.
pub(crate) fn inspect_takeout_impl(
    request: TakeoutRequest,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_result(vault_worker::inspect_takeout_source(&request))
}

/// Imports one Takeout source into the archive.
pub(crate) fn import_takeout_impl(
    request: TakeoutRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_result(vault_worker::import_takeout_source(session_database_key, &request))
}

/// Loads the detailed preview for one import batch.
pub(crate) fn preview_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::preview_import_batch_detail(session_database_key, batch_id))
}

/// Hides one import batch from the visible archive surface.
pub(crate) fn revert_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::revert_import_batch_detail(session_database_key, batch_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Restores a previously reverted import batch to visible status.
pub(crate) fn restore_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::restore_import_batch_detail(session_database_key, batch_id))
}
