use vault_core::TakeoutRequest;
use vault_worker;

use super::worker_result;

pub(crate) fn inspect_takeout_impl(
    request: TakeoutRequest,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_result(vault_worker::inspect_takeout_source(&request))
}

pub(crate) fn import_takeout_impl(
    request: TakeoutRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::TakeoutInspection, String> {
    worker_result(vault_worker::import_takeout_source(session_database_key, &request))
}

pub(crate) fn preview_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::preview_import_batch_detail(session_database_key, batch_id))
}

pub(crate) fn revert_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::revert_import_batch_detail(session_database_key, batch_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn restore_import_batch_impl(
    batch_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::ImportBatchDetail, String> {
    worker_result(vault_worker::restore_import_batch_detail(session_database_key, batch_id))
}
