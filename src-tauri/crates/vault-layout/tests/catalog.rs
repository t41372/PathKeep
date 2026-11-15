//! Responsibilities: reject declared path/key aliases and invalid startup dependencies.
//! Not responsible for: inferring on-disk membership or generating functional expectations.

use std::collections::HashSet;
use vault_layout::*;

#[test]
fn database_descriptors_are_unambiguous_and_dependencies_start_first() {
    let mut identities = HashSet::new();
    let mut paths = HashSet::new();
    let mut keys = HashSet::new();
    let startup = startup_databases();
    for database in databases() {
        assert!(identities.insert(database.db), "duplicate database identity");
        assert!(paths.insert(database_path(database.db)), "aliased database path");
        assert!(keys.insert(database.key), "aliased database purpose");
        assert_eq!(database_spec(database.db), database);
        let position = startup.iter().position(|db| *db == database.db).unwrap();
        for dependency in database.dependencies {
            let before = startup.iter().position(|db| db == dependency).unwrap();
            assert!(before < position, "dependency must start before its consumer");
        }
    }
    for file in artifacts().filter_map(|s| s.file_key) {
        assert!(keys.insert(file), "aliased non-database key purpose");
    }
    assert_eq!(identities.len(), Db::COUNT);
    assert_eq!(databases().filter(|db| db.primary).count(), 1);
}

const LAYOUTS: [ArchiveLayout; 2] = [ArchiveLayout::BrowsingV1, ArchiveLayout::AgentHistoryV2];

#[test]
fn installation_members_do_not_alias_or_overlap() {
    for files in LAYOUTS
        .into_iter()
        .flat_map(|layout| [installation_files(layout, true), portable_installation_files(layout)])
    {
        for (index, file) in files.iter().enumerate() {
            assert!(!file.is_absolute());
            assert!(!file.components().any(|part| part == std::path::Component::ParentDir));
            for other in &files[..index] {
                assert!(
                    !file.starts_with(other) && !other.starts_with(file),
                    "{file:?} overlaps {other:?}"
                );
            }
        }
    }
    let mut entries = HashSet::new();
    for (_, entry) in portable_files(ArchiveLayout::NEWEST) {
        assert!(entries.insert(entry), "aliased bundle entry");
    }
}

// A newer layout adds; it never drops what an older one holds. Restoring or importing an older
// archive into a newer one relies on that: everything the old one has, the new one has a row for.
#[test]
fn each_layout_holds_everything_the_one_before_held() {
    for pair in LAYOUTS.windows(2) {
        let (older, newer) = (pair[0], pair[1]);
        let rows = |layout| artifacts_in(layout).map(|s| s.id).collect::<HashSet<_>>();
        assert!(rows(older).is_subset(&rows(newer)), "{newer:?} drops a row of {older:?}");
        for (old, new) in [
            (databases_in(older), databases_in(newer)),
            (snapshot_databases(older).collect(), snapshot_databases(newer).collect()),
            (mode_change_databases(older).collect(), mode_change_databases(newer).collect()),
            (portable_databases(older).collect(), portable_databases(newer).collect()),
        ] {
            let (old, new): (HashSet<Db>, HashSet<Db>) =
                (old.into_iter().collect(), new.into_iter().collect());
            assert!(old.is_subset(&new), "{newer:?} drops a store of {older:?}");
        }
    }
}

// The released layout's stores are exactly the five PathKeep 0.4 and 0.5 open. A row added for a
// newer layout that leaked into this one would make every older archive look incomplete.
#[test]
fn the_released_layout_has_exactly_the_released_stores() {
    let mut stores = databases_in(ArchiveLayout::BrowsingV1);
    stores.sort_by_key(|db| *db as usize);
    assert_eq!(stores, [Db::Archive, Db::SourceEvidence, Db::Intelligence, Db::Agent, Db::Scratch]);
    assert_eq!(ArchiveLayout::of_revision(0), ArchiveLayout::BrowsingV1);
    assert_eq!(ArchiveLayout::of_revision(1), ArchiveLayout::AgentHistoryV2);
    assert_eq!(ArchiveLayout::of_revision(u64::MAX), ArchiveLayout::NEWEST);
}
