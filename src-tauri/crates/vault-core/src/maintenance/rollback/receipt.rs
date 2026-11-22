//! The sibling receipt survives a 0.5 wipe and keeps a later launch from promoting by surprise.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Receipt {
    format: String,
    version: u32,
    operation_id: String,
    source_root: PathBuf,
    source_release: String,
    source_revision: u64,
    target_release: String,
    target_revision: u64,
    pub(super) state: State,
    pub(super) choice: Choice,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum State {
    Prepared,
    Ready,
    HandedOff,
    Resolved,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Choice {
    Undecided,
    Fresh,
    Rejoin,
}

pub(super) fn namespace(root: &Path) -> Result<PathBuf> {
    let mut name =
        root.file_name().context("a volume root cannot contain a sibling handoff")?.to_owned();
    name.push(".pathkeep-rollback");
    Ok(root.with_file_name(name))
}

impl Receipt {
    pub(super) fn create(root: &Path) -> Result<(PathBuf, Self)> {
        let root = fs::canonicalize(root)?;
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes)?;
        let id = bytes.iter().map(|v| format!("{v:02x}")).collect::<String>();
        let base = namespace(&root)?;
        fs::create_dir_all(&base)?;
        ensure!(fs::symlink_metadata(&base)?.is_dir(), "rollback namespace must be a directory");
        let folder = base.join(&id);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(&folder)?;
        }
        #[cfg(not(unix))]
        fs::create_dir(&folder)?;
        Ok((
            folder,
            Self {
                format: "pathkeep.rollback-handoff".into(),
                version: 1,
                operation_id: id,
                source_root: root,
                source_release: crate::APP_VERSION.into(),
                source_revision: crate::admission::PRODUCT_ROOT_LEVEL,
                target_release: "0.5.0".into(),
                target_revision: 0,
                state: State::Prepared,
                choice: Choice::Undecided,
            },
        ))
    }
    pub(super) fn save(&self, dir: &Path) -> Result<()> {
        crate::durable_io::atomic_durable_write(
            &dir.join("handoff.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }
}

pub(super) fn receipts(root: &Path) -> Result<Vec<(PathBuf, Receipt)>> {
    if !root.try_exists()? {
        return Ok(Vec::new());
    }
    let root = fs::canonicalize(root)?;
    if root.file_name().is_none() {
        return Ok(Vec::new());
    }
    let base = namespace(&root)?;
    let entries = match fs::read_dir(&base) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    ensure!(fs::symlink_metadata(&base)?.is_dir(), "unsupported rollback namespace");
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let folder = entry.path();
        let path = folder.join("handoff.json");
        if !path.try_exists()? {
            continue;
        }
        let r: Receipt = serde_json::from_slice(&fs::read(path)?)?;
        ensure!(
            r.format == "pathkeep.rollback-handoff"
                && r.version == 1
                && r.operation_id.len() == 32
                && r.operation_id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && entry.file_name() == r.operation_id.as_str()
                && r.source_root == root,
            "archive-handoff-invalid: receipt identity does not match its folder"
        );
        result.push((folder, r));
    }
    Ok(result)
}

pub(super) fn pending(root: &Path) -> Result<Option<(PathBuf, Receipt)>> {
    let mut entries = receipts(root)?.into_iter().filter(|(_, r)| r.state != State::Resolved);
    let Some((dir, r)) = entries.next() else { return Ok(None) };
    ensure!(
        entries.next().is_none()
            && r.target_release == "0.5.0"
            && r.source_revision == 1
            && r.target_revision == 0,
        "archive-handoff-invalid: unsupported or conflicting handoff"
    );
    Ok(Some((dir, r)))
}
