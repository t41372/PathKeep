//! Responsibilities: observe a structured working directory through a fixed git-file allowlist.
//! Not responsible for: running git, reading worktree contents or resolving config includes.
use crate::{Diagnostic, Error};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// These facts are irreplaceable once a repository or worktree disappears.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitObservation {
    pub top_level: String,
    pub common_dir: String,
    pub branch: Option<String>,
    pub remotes: Vec<String>,
}

pub(super) fn observe(folder: &Path) -> (Option<GitObservation>, Option<Diagnostic>) {
    match inspect(folder) {
        Ok(git) => (git, None),
        Err(error) => {
            let code = match error {
                Error::LimitExceeded(_) => "git_file_too_large",
                Error::InvalidInput(_) => "git_symlink_skipped",
                _ => "git_unreadable",
            };
            (
                None,
                Some(Diagnostic {
                    code: code.into(),
                    path: folder.to_str().map(str::to_owned),
                    ordinal: None,
                    detail: None,
                }),
            )
        }
    }
}

fn inspect(folder: &Path) -> crate::Result<Option<GitObservation>> {
    if !folder.is_absolute() || !folder.is_dir() {
        return Ok(None);
    }
    let resolved = fs::canonicalize(folder)?;
    for top in resolved.ancestors() {
        let entry = top.join(".git");
        let metadata = match fs::symlink_metadata(&entry) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() {
            return Err(Error::InvalidInput("git symlink".into()));
        }
        let admin = if metadata.is_dir() {
            entry
        } else {
            let Some(pointer) = read(&entry)?
                .and_then(|text| text.strip_prefix("gitdir:").map(str::trim).map(str::to_owned))
            else {
                return Ok(None);
            };
            resolve(top, &pointer)
        };
        let common = match read(&admin.join("commondir"))? {
            Some(path) => resolve(&admin, path.trim()),
            None => admin.clone(),
        };
        let head = read(&admin.join("HEAD"))?;
        let branch = head
            .as_deref()
            .and_then(|head| head.trim().strip_prefix("ref: refs/heads/"))
            .map(str::to_owned);
        let config = read(&common.join("config"))?.unwrap_or_default();
        return Ok(Some(GitObservation {
            top_level: top.to_string_lossy().into_owned(),
            common_dir: common.to_string_lossy().into_owned(),
            branch,
            remotes: remotes(&config),
        }));
    }
    Ok(None)
}

fn resolve(base: &Path, target: &str) -> PathBuf {
    let path = base.join(target);
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                output.pop();
            }
            std::path::Component::CurDir => {}
            part => output.push(part.as_os_str()),
        }
    }
    output
}

fn read(path: &Path) -> crate::Result<Option<String>> {
    match super::files::read_git_file(path, 1024 * 1024) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| Error::Unsupported("git file is not valid UTF-8".into())),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn remotes(config: &str) -> Vec<String> {
    let mut remote = false;
    let mut urls = Vec::new();
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            remote = line.starts_with("[remote ") && line.ends_with(']');
            continue;
        }
        if !remote {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("url")
        {
            let value = value.trim().trim_matches('"');
            let clean = if let Ok(mut url) = url::Url::parse(value) {
                if url.set_username("").is_err() || url.set_password(None).is_err() {
                    continue;
                }
                url.to_string()
            } else if let Some((_, host_path)) = value.split_once('@') {
                host_path.into()
            } else {
                value.into()
            };
            urls.push(clean);
        }
    }
    urls.sort();
    urls.dedup();
    urls
}
