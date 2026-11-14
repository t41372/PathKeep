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

#[test]
fn installation_members_do_not_alias_or_overlap() {
    for files in [installation_files(true), portable_installation_files()] {
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
    for (_, entry) in portable_files() {
        assert!(entries.insert(entry), "aliased bundle entry");
    }
}
