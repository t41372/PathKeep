use vault_core::S3CredentialInput;

use super::worker_result;

pub(crate) fn preview_remote_backup_impl() -> Result<vault_core::RemoteBackupPreview, String> {
    worker_result(vault_worker::preview_remote_backup_bundle())
}

pub(crate) fn run_remote_backup_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::RemoteBackupResult, String> {
    worker_result(vault_worker::upload_remote_backup_bundle(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn verify_remote_backup_impl(
    bundle_path: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::RemoteBackupVerification, String> {
    worker_result(vault_worker::verify_remote_backup_bundle(session_database_key, &bundle_path))
}

pub(crate) fn store_s3_credentials_impl(credentials: S3CredentialInput) -> Result<(), String> {
    worker_result(vault_worker::store_s3_credentials(&credentials))
}

pub(crate) fn clear_s3_credentials_impl() -> Result<(), String> {
    worker_result(vault_worker::clear_s3_credentials())
}
