//! Responsibilities: refuse a future source inside an otherwise valid format-2 encrypted bundle.
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

#[test]
fn future_move_source_refuses_before_any_target_root_effect_even_when_target_has_no_vault() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let paths = vault_core::project_paths_with_root(&source);
    fs::create_dir(&source).unwrap();
    fs::write(source.join("config.json"), LEGACY).unwrap();
    let config: AppConfig = serde_json::from_str(LEGACY).unwrap();
    let vault = Vault::open(paths, config, None).unwrap();
    vault.search().set_paused_for_tests(true);
    let original = temp.path().join("current.pkexport");
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
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&entries["pathkeep-export-manifest.json"]).unwrap();
    let row = manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["path"] == "config/config.json")
        .unwrap();
    row["sha256"] = vault_core::utils::sha256_hex(&control).into();
    row["sizeBytes"] = (control.len() as u64).into();
    entries.insert("config/config.json".into(), control);
    let manifest = serde_json::to_vec(&manifest).unwrap();
    entries.insert(
        "pathkeep-export-manifest.sha256".into(),
        vault_core::utils::sha256_hex(&manifest).into_bytes(),
    );
    entries.insert("pathkeep-export-manifest.json".into(), manifest);
    let bundle = temp.path().join("future.pkexport");
    let mut zip = ZipWriter::new(fs::File::create(&bundle).unwrap());
    for (name, bytes) in entries {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
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
    assert!(
        detail.contains("archive-reader-too-old")
            && before == after
            && source_before == vault_core::utils::sha256_hex(&fs::read(&bundle).unwrap()),
        "result={detail}; before={before:?}; after={after:?}"
    );
}
