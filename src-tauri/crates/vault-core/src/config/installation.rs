//! Bind capture choices in a live control to this installation. The random
//! identifier lives outside the archive. Merely opening a foreign archive never
//! creates an identifier or adopts the device that last wrote its choices.
use super::AppConfig;
use anyhow::{Context, Result};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::PathBuf,
};

pub(super) fn has_choices(config: &AppConfig) -> bool {
    config.onboarding_choices.is_some()
        || config.agent_viewer != Default::default()
        || config.capture_policy != Default::default()
        || !config.dismissed_agent_hints.is_empty()
}

fn directory() -> Result<PathBuf> {
    // Owner and lifecycle fixtures give platform services their own installation.
    if cfg!(debug_assertions)
        && let Some(path) = std::env::var_os("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR")
    {
        return Ok(PathBuf::from(path).join("installation"));
    }
    #[cfg(test)]
    {
        static DIR: std::sync::LazyLock<tempfile::TempDir> =
            std::sync::LazyLock::new(|| tempfile::tempdir().unwrap());
        Ok(DIR.path().to_owned())
    }
    #[cfg(not(test))]
    {
        let id = vault_build::CURRENT.identity();
        let dir = directories::ProjectDirs::from(
            id.qualifier,
            id.organization,
            &format!("{}-installation", id.application),
        )
        .context("locating device capture settings")?;
        Ok(dir.data_local_dir().into())
    }
}

pub(super) fn id(create: bool) -> Result<Option<String>> {
    let dir = directory()?;
    let path = dir.join("capture-device");
    match fs::read_to_string(&path) {
        Ok(value) => {
            anyhow::ensure!(
                value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid installation identity"
            );
            return Ok(Some(crate::utils::sha256_hex(value.as_bytes())));
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    if !create {
        return Ok(None);
    }
    fs::create_dir_all(&dir)?;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    let value = hex::encode(bytes);
    let mut file = tempfile::NamedTempFile::new_in(&dir)?;
    file.write_all(value.as_bytes())?;
    crate::durable_io::fsync_file_durably(file.path())?;
    match vault_fs::persist_noclobber(file, &path) {
        Ok(_) => {
            crate::durable_io::fsync_dir(&dir)?;
            Ok(Some(crate::utils::sha256_hex(value.as_bytes())))
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            Ok(Some(id(false)?.context("installation identity disappeared during creation")?))
        }
        Err(e) => Err(e).context("publishing installation identity"),
    }
}
