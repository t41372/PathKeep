//! Run against a mounted FAT32 or exFAT filesystem, with the same released archive on
//! each volume. The advisory is available before onboarding creates the requested folder.
use std::{path::Path, process::Command};

#[test]
#[ignore = "requires PATHKEEP_TEST_FAT_VOLUME, a writable FAT32 or exFAT mount"]
fn warns_for_both_the_archive_and_a_new_folder_on_the_same_fat_volume() {
    let mount = std::env::var_os("PATHKEEP_TEST_FAT_VOLUME").expect("FAT32 or exFAT test mount");
    let root = tempfile::tempdir_in(&mount).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/v0.5-archive/data.tar.gz");
    assert!(
        Command::new("tar")
            .arg("-xzf")
            .arg(fixture)
            .arg("-C")
            .arg(root.path())
            .status()
            .unwrap()
            .success()
    );
    let paths = vault_core::project_paths_with_root(root.path());
    assert_eq!(
        vault_core::load_config(&paths).unwrap().archive_mode,
        vault_core::ArchiveMode::Encrypted
    );
    for folder in [root.path().to_owned(), root.path().join("new-data-folder")] {
        assert!(vault_core::storage_volume::fat_data_folder(&folder).unwrap());
    }
    assert!(!root.path().join("new-data-folder").exists());
}
