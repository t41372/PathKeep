//! Retained-source recovery tests, written before the inventory implementation.
//!
//! A preview's capped live-source list cannot recover raw-only history after its
//! original directory disappears. These tests use a real encrypted archive,
//! retain more than 256 files without invoking a parser, remove the input tree,
//! and recover the bytes by walking the archive's public inventory.
//! Other failure modes: filter reuse accepts the wrong cursor, new sources alter
//! a walk already in progress, and long portable paths defeat page byte bounds.

use std::{collections::BTreeSet, fs, path::Path};

use agent_history::{
    Archive, Error,
    model::{Artifact, ArtifactKind, Harness, SourceRoot},
    query::{SessionQuery, SourceQuery},
};
use sha2::{Digest, Sha256};
use vault_crypto::KdfParams;

fn archive() -> (tempfile::TempDir, Archive) {
    let directory = tempfile::tempdir().unwrap();
    let archive = Archive::create_with_params(
        &directory.path().join("archive"),
        b"inventory-test-password",
        KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
    )
    .unwrap();
    (directory, archive)
}

fn retain(
    archive: &Archive,
    root: &Path,
    id: &str,
    harness: Harness,
    kind: ArtifactKind,
    bytes: &[u8],
) -> i64 {
    let source = SourceRoot { harness, root: root.into() };
    let artifact = Artifact {
        path: root.join(format!("{id}.native")),
        relative_path: format!("{id}.native"),
        kind,
        project_hint: None,
        generation: None,
        current: true,
    };
    let revision = archive.begin_revision(id, &source, &artifact).unwrap();
    archive.append_chunk(revision, 0, bytes.to_vec()).unwrap();
    archive
        .finish_revision(revision, &hex::encode(Sha256::digest(bytes)), bytes.len() as u64)
        .unwrap()
        .revision_id
}

fn native_bytes(index: usize) -> Vec<u8> {
    let mut bytes = format!("unknown native bytes {index}").into_bytes();
    bytes.extend([0, 255]);
    bytes
}

#[test]
fn every_raw_only_source_remains_discoverable_after_the_input_tree_is_deleted() {
    let (directory, archive) = archive();
    let input = directory.path().join("native-history");
    fs::create_dir(&input).unwrap();
    for index in 0..259 {
        let id = format!("source-{index:04}");
        let bytes = native_bytes(index);
        fs::write(input.join(format!("{id}.native")), &bytes).unwrap();
        retain(
            &archive,
            &input,
            &id,
            if index % 2 == 0 { Harness::Codex } else { Harness::Pi },
            if index % 3 == 0 { ArtifactKind::Attachment } else { ArtifactKind::Transcript },
            &bytes,
        );
    }
    fs::remove_dir_all(&input).unwrap();
    assert!(archive.list_sessions(&SessionQuery::default()).unwrap().items.is_empty());

    let mut query = SourceQuery { limit: 37, ..Default::default() };
    let mut ids = BTreeSet::new();
    loop {
        let page = archive.list_sources(&query).unwrap();
        assert!(!page.items.is_empty());
        assert!(page.items.len() <= 37);
        for source in page.items {
            assert!(ids.insert(source.id.clone()), "source appeared twice");
            assert_eq!(source.root, input.to_str().unwrap());
            assert!(!Path::new(&source.root).exists());
            let index: usize = source.id.trim_start_matches("source-").parse().unwrap();
            let mut recovered = Vec::new();
            archive.write_raw(source.latest_revision_id.unwrap(), &mut recovered).unwrap();
            assert_eq!(recovered, native_bytes(index));
        }
        query.after = page.next_cursor;
        if query.after.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), 259);

    let filtered = archive
        .list_sources(&SourceQuery {
            harness: Some(Harness::Codex),
            kind: Some(ArtifactKind::Attachment),
            limit: 500,
            after: None,
        })
        .unwrap();
    assert_eq!(filtered.items.len(), (0..259).filter(|index| index % 6 == 0).count());
    assert!(
        filtered
            .items
            .iter()
            .all(|source| source.harness == "codex" && source.kind == "attachment")
    );
}

#[test]
fn a_walk_excludes_later_registrations_and_rejects_a_different_filter() {
    let (_directory, archive) = archive();
    let root = Path::new("/removed-history");
    for id in ["b", "d", "f"] {
        retain(&archive, root, id, Harness::Pi, ArtifactKind::Metadata, id.as_bytes());
    }
    let query = SourceQuery { limit: 2, ..Default::default() };
    let first = archive.list_sources(&query).unwrap();
    assert_eq!(first.items.iter().map(|source| source.id.as_str()).collect::<Vec<_>>(), ["b", "d"]);
    let cursor = first.next_cursor.unwrap();
    for id in ["a", "e", "z"] {
        retain(&archive, root, id, Harness::Pi, ArtifactKind::Metadata, id.as_bytes());
    }
    let next = archive
        .list_sources(&SourceQuery { after: Some(cursor.clone()), ..query.clone() })
        .unwrap();
    assert_eq!(next.items.iter().map(|source| source.id.as_str()).collect::<Vec<_>>(), ["f"]);
    assert!(next.next_cursor.is_none());
    assert_eq!(archive.list_sources(&SourceQuery::default()).unwrap().items.len(), 6);
    assert!(matches!(
        archive.list_sources(&SourceQuery {
            kind: Some(ArtifactKind::Metadata),
            after: Some(cursor),
            ..query
        }),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        archive.list_sources(&SourceQuery { limit: 501, ..Default::default() }),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn inventory_pages_bound_long_cross_platform_provenance_without_truncation() {
    let (_directory, archive) = archive();
    // A Windows extended path may contain up to 32,767 UTF-16 units. Its UTF-8
    // representation can therefore exceed the host's own path-length limit.
    // The archive stores portable provenance; this test never opens that path.
    let component = "資料".repeat(100);
    let root = format!("\\\\?\\C:\\{}", vec![component; 100].join("\\"));
    for index in 0..90 {
        retain(
            &archive,
            Path::new(&root),
            &format!("long-{index:03}"),
            Harness::Pi,
            ArtifactKind::Attachment,
            b"retained",
        );
    }
    let first = archive.list_sources(&SourceQuery { limit: 500, ..Default::default() }).unwrap();
    assert!(!first.items.is_empty());
    assert!(first.items.len() < 90);
    assert!(first.items.iter().all(|source| source.root == root));
    let second = archive
        .list_sources(&SourceQuery { limit: 500, after: first.next_cursor, ..Default::default() })
        .unwrap();
    assert_eq!(first.items.len() + second.items.len(), 90);
    assert!(second.next_cursor.is_none());
}
