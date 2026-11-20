//! Responsibilities: revisit structured conversation folders from the approved roots during a backup.
//! Not responsible for: deriving paths from text or loading conversation bodies.
use super::{BackupOptions, BackupReport, check_cancel, git};
use crate::{Archive, Result, SourceRoot, adapters::common};
use vault_store::{Db, params};

pub(super) fn folders(
    archive: &Archive,
    source: &SourceRoot,
    options: &BackupOptions,
    report: &mut BackupReport,
) -> Result<()> {
    let mut after = String::new();
    loop {
        check_cancel(options)?;
        let key = after.clone();
        let harness = source.harness.to_string();
        let root = source.root.to_string_lossy().into_owned();
        let paths = archive.store.read(Db::AgentHistoryReadModel, move |tx| {
            tx.query(
                "
            SELECT DISTINCT project_path FROM ah_sessions
            WHERE harness=?1 AND source_root=?2 AND project_path>?3 ORDER BY project_path LIMIT 128",
                params![harness, root, key],
                |r| Ok(r.get::<String>(0)?),
            )
        })?;
        if paths.is_empty() {
            return Ok(());
        }
        for path in paths {
            check_cancel(options)?;
            let folder = std::path::Path::new(&path);
            let exists = folder.is_dir();
            let (facts, issue) =
                if options.read_git && exists { git::observe(folder) } else { (None, None) };
            if let Some(issue) = issue {
                common::push_diagnostic(&mut report.issues, issue);
            }
            archive.observe_folder(&path, exists, options.read_git, facts)?;
            after = path;
        }
    }
}
