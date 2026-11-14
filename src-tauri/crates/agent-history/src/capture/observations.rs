//! Responsibilities: revisit every structured conversation folder during a backup.
//! Not responsible for: deriving paths from text or loading conversation bodies.
use super::{BackupOptions, BackupReport, check_cancel, git};
use crate::{Archive, Result, adapters::common};
use vault_store::Db;

pub(super) fn folders(
    archive: &Archive,
    options: &BackupOptions,
    report: &mut BackupReport,
) -> Result<()> {
    let mut after = String::new();
    loop {
        check_cancel(options)?;
        let key = after.clone();
        let paths = archive.store.read(Db::Archive, move |tx| {
            tx.query(
                "
            SELECT DISTINCT project_path FROM ah_sessions WHERE project_path>?1 AND harness IN
            ('claude-code', 'codex' , 'pi' , 'deepseek' ) ORDER BY project_path LIMIT 128",
                [key],
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
            archive.observe_folder(&path, exists, facts)?;
            after = path;
        }
    }
}
