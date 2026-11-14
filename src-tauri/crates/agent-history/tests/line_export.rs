//! Responsibilities: reject an export whose inherited source is republished after its header.
//! Not responsible for: testing ordinary event paging or modifying any contract fixture.
//! Failure mode written before the fix: guarding only the current rollout misses a base reparse.
#[allow(dead_code)]
mod support;
use agent_history::{Archive, Harness, SourceRoot, capture::BackupOptions, export::ExportFormat};
use std::io::Write;
use support::native::*;
use vault_crypto::KdfParams;

#[test]
fn republishing_an_inherited_source_during_export_invalidates_the_export() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("codex");
    let thread = "33333333-3333-4333-8333-333333333333";
    let rollout = "44444444-4444-4444-8444-444444444444";
    let files = codex_reverted_thread(&root, thread, rollout, "/synthetic/a", "/synthetic/a");
    let archive = Archive::create_with_params(
        &temp.path().join("archive"),
        b"synthetic line export password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    archive
        .backup(
            &[SourceRoot { harness: Harness::Codex, root: root.clone() }],
            &BackupOptions::default(),
        )
        .unwrap();
    let conversation = archive.list_sessions(&Default::default()).unwrap().items.remove(0);
    let base = archive
        .list_sources(&Default::default())
        .unwrap()
        .items
        .into_iter()
        .find(|source| root.join(&source.relative_path) == files.base)
        .unwrap()
        .id;
    struct Reparse<'a> {
        archive: &'a Archive,
        base: Option<String>,
    }
    impl Write for Reparse<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(base) = self.base.take() {
                self.archive.reparse_source(&base, &BackupOptions::default()).unwrap();
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Reparse { archive: &archive, base: Some(base) };
    let result = archive.write_normalized(&conversation.id, ExportFormat::Jsonl, &mut output);
    assert!(matches!(result, Err(agent_history::Error::StaleCursor)));
}
