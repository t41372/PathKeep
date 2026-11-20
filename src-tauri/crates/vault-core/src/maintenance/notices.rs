//! Recovery notices stay in the owner process until the foreground window consumes them.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryNotice {
    pub code: &'static str,
    pub path: Option<PathBuf>,
}

static NOTICES: Mutex<BTreeMap<PathBuf, Vec<RecoveryNotice>>> = Mutex::new(BTreeMap::new());

/// Coalesce repeated dispositions of files in one retained folder into one user notice.
pub(crate) fn remember(root: &Path, code: &'static str, path: Option<&Path>) {
    let item = RecoveryNotice { code, path: path.map(Path::to_owned) };
    let mut pending = NOTICES.lock().unwrap();
    let notices = pending.entry(root.to_owned()).or_default();
    if !notices.contains(&item) {
        notices.push(item);
    }
}

/// Drain only this archive's notices. Reading the settings snapshot never acknowledges one.
pub fn take(root: &Path) -> Vec<RecoveryNotice> {
    NOTICES.lock().unwrap().remove(root).unwrap_or_default()
}
