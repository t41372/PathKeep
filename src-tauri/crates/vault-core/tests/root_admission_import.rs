//! Responsibilities: refuse a future source inside an otherwise valid format-2 encrypted bundle,
//! and a file whose format or archive schema only a newer build reads.
//! Not responsible for: publishing product stamps or treating this current-tree export as v0.5.
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
};
use vault_core::{AppConfig, Vault};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
#[path = "../../../../tests/root_admission/support/tree.rs"]
mod tree;
const LEGACY: &str = include_str!("../../../../tests/root_admission/fixtures/legacy.json");
// The test authors the stamped bytes; the encryption codec only wraps those bytes.
const SOURCE_CONTROL: &[u8] = br#"{"initialized":true,"archiveMode":{"formatVersion":2,"mode":"Encrypted","compatibility":{"revision":2,"minReader":1,"minWriter":2}},"gitEnabled":false}"#;

/// The entries of a file this build exports from a new archive under `dir`.
fn exported_entries(dir: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    let source = dir.join("source");
    let paths = vault_core::project_paths_with_root(&source);
    fs::create_dir(&source).unwrap();
    fs::write(source.join("config.json"), LEGACY).unwrap();
    let config: AppConfig = serde_json::from_str(LEGACY).unwrap();
    let vault = Vault::open(paths, config, None).unwrap();
    vault.search().set_paused_for_tests(true);
    let original = dir.join("current.pkexport");
    vault_core::migration::native::export(
        &vault,
        &original,
        "fixture password",
        Default::default(),
        |_| {},
    )
    .unwrap();
    vault.close().unwrap();
    let mut zip = ZipArchive::new(fs::File::open(original).unwrap()).unwrap();
    let mut entries = BTreeMap::new();
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        entries.insert(entry.name().unwrap().into_owned(), bytes);
    }
    entries
}

fn manifest_of(entries: &BTreeMap<String, Vec<u8>>) -> serde_json::Value {
    serde_json::from_slice(&entries["pathkeep-export-manifest.json"]).unwrap()
}

/// Puts `manifest` in place with the digest the file keeps beside it, as the build that wrote the
/// file would have.
fn reseal_manifest(entries: &mut BTreeMap<String, Vec<u8>>, manifest: &serde_json::Value) {
    let manifest = serde_json::to_vec(manifest).unwrap();
    entries.insert(
        "pathkeep-export-manifest.sha256".into(),
        vault_core::utils::sha256_hex(&manifest).into_bytes(),
    );
    entries.insert("pathkeep-export-manifest.json".into(), manifest);
}

fn write_bundle(bundle: &std::path::Path, entries: &BTreeMap<String, Vec<u8>>) {
    let mut zip = ZipWriter::new(fs::File::create(bundle).unwrap());
    for (name, bytes) in entries {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// A file made by a newer PathKeep, by its format or by its archive's schema, is refused at the
/// check as one, so the window can say to update PathKeep; this folder is left as it was. A number
/// this build does not know that is not newer is only unsupported.
#[test]
fn a_move_file_only_a_newer_build_reads_is_refused_at_the_check_as_newer() {
    let temp = tempfile::tempdir().unwrap();
    let entries = exported_entries(temp.path());
    let schema = manifest_of(&entries)["archiveSchemaVersion"].as_i64().unwrap();
    let target = temp.path().join("target");
    fs::create_dir(&target).unwrap();
    let before = tree::census(&target, vault_core::utils::sha256_hex);
    for (field, value, expected) in [
        ("formatVersion", 4, "move-file-too-new: "),
        ("archiveSchemaVersion", schema + 1, "move-file-too-new: "),
        ("formatVersion", 0, "unsupported bundle format 0"),
    ] {
        let mut changed = entries.clone();
        let mut manifest = manifest_of(&changed);
        manifest[field] = value.into();
        reseal_manifest(&mut changed, &manifest);
        let bundle = temp.path().join(format!("{field}-{value}.pkexport"));
        write_bundle(&bundle, &changed);
        let error = vault_core::migration::native::preview(
            &vault_core::project_paths_with_root(&target),
            &bundle,
        )
        .expect_err("refused");
        let error = format!("{error:#}");
        assert!(error.contains(expected), "{field} {value}: {error}");
        if !expected.starts_with("move-file-too-new") {
            assert!(!error.contains("move-file-too-new"), "{field} {value}: {error}");
        }
    }
    assert_eq!(tree::census(&target, vault_core::utils::sha256_hex), before);
}

#[test]
fn future_move_source_refuses_before_any_target_root_effect_even_when_target_has_no_vault() {
    let temp = tempfile::tempdir().unwrap();
    let mut entries = exported_entries(temp.path());
    let key = vault_crypto::Envelope::parse(&entries["archive/keys.envelope"])
        .unwrap()
        .unwrap(b"fixture password")
        .unwrap();
    let sealed = temp.path().join("sealed");
    fs::create_dir_all(sealed.join("config")).unwrap();
    vault_crypto::encrypt_artifact(
        &sealed,
        "config/config.json",
        &key,
        &mut Cursor::new(SOURCE_CONTROL),
        || false,
    )
    .unwrap();
    let control = fs::read(sealed.join("config/config.json")).unwrap();
    let mut manifest = manifest_of(&entries);
    let row = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["path"] == "config/config.json")
        .unwrap();
    row["sha256"] = vault_core::utils::sha256_hex(&control).into();
    row["sizeBytes"] = (control.len() as u64).into();
    entries.insert("config/config.json".into(), control);
    reseal_manifest(&mut entries, &manifest);
    let bundle = temp.path().join("future.pkexport");
    write_bundle(&bundle, &entries);
    vault_core::migration::read_and_validate_manifest(&bundle).unwrap();
    let target = temp.path().join("target");
    fs::create_dir(&target).unwrap();
    let before = tree::census(&target, vault_core::utils::sha256_hex);
    let source_before = vault_core::utils::sha256_hex(&fs::read(&bundle).unwrap());
    let result = vault_core::migration::native::import(
        &vault_core::project_paths_with_root(&target),
        None,
        None,
        &bundle,
        &vault_core::ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some("fixture password".into()),
        },
        Default::default(),
    );
    let detail = match result {
        Ok(_) => "SUCCESS".into(),
        Err(error) => format!("{error:#}"),
    };
    let after = tree::census(&target, vault_core::utils::sha256_hex);
    // The source declares floors 1/2: a build of level 1 can read it but not write it.
    assert!(
        detail.contains("archive-writer-too-old")
            && before == after
            && source_before == vault_core::utils::sha256_hex(&fs::read(&bundle).unwrap()),
        "result={detail}; before={before:?}; after={after:?}"
    );
}
