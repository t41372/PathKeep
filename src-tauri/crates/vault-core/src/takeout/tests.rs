//! Regression tests for the Takeout import boundary.

mod browser_history;

use super::*;
use crate::config::{ensure_paths, project_paths_with_root};
use crate::test_support::store::Probe;
use tempfile::tempdir;

fn chrome_browser_history_payload(records: &[&str]) -> String {
    format!(r#"{{"Browser History":[{}]}}"#, records.join(","))
}

/// Builds temporary project paths for one Takeout test fixture root.
fn sample_paths(root: &Path) -> ProjectPaths {
    project_paths_with_root(root)
}

/// Writes a Takeout folder holding `Chrome/BrowserHistory.json` with these records.
fn write_takeout_fixture_with_name(dir: &Path, name: &str, records: &[&str]) -> PathBuf {
    let source_dir = dir.join(name);
    let chrome_dir = source_dir.join("Chrome");
    fs::create_dir_all(&chrome_dir).expect("create takeout source dir");
    fs::write(chrome_dir.join("BrowserHistory.json"), chrome_browser_history_payload(records))
        .expect("write takeout fixture");
    source_dir
}

#[test]
fn import_takeout_deduplicates_matching_history_from_different_files() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let source_a = write_takeout_fixture_with_name(
        dir.path(),
        "takeout-a",
        &[r#"{"url":"https://example.com/one","title":"One","time_usec":1775037600000000}"#],
    );
    let source_b = write_takeout_fixture_with_name(
        dir.path(),
        "takeout-b",
        &[r#"{"url":"https://example.com/one","title":"One","time_usec":1775037600000000}"#],
    );

    let first = import_takeout(
        &vault,
        &TakeoutRequest { source_path: source_a.display().to_string(), dry_run: false },
    )
    .expect("first import");
    let second = import_takeout(
        &vault,
        &TakeoutRequest { source_path: source_b.display().to_string(), dry_run: false },
    )
    .expect("second import");

    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let visible: i64 = Probe::new(&vault)
        .query_row("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", (), |r| Ok(r.get(0)?))
        .unwrap();
    assert_eq!(visible, 1);

    let archive = Probe::new(&vault);
    let visits: i64 = archive
        .query_row("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", (), |row| {
            Ok(row.get(0)?)
        })
        .expect("visit count");
    assert_eq!(visits, 1);
}

#[test]
fn trailing_zip_checksum_failure_keeps_a_failed_import_prefix() {
    use std::io::{Cursor, Write};
    let vault = crate::Vault::open_temp().unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "Takeout/Chrome/BrowserHistory.json",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(br#"{"Browser History":[{"url":"https://checksum.test","title":"Checksum","time_usec":1711965600000000}]}"#).unwrap();
    zip.write_all(&vec![b' '; 16_384]).unwrap();
    let mut bytes = zip.finish().unwrap().into_inner();
    let directory = bytes.windows(4).position(|b| b == b"PK\x01\x02").unwrap();
    bytes[directory + 16..directory + 20].fill(0);
    let path = vault.paths().app_root.join("bad-checksum.zip");
    std::fs::write(&path, bytes).unwrap();
    let result = import_takeout(
        &vault,
        &TakeoutRequest { source_path: path.display().to_string(), dry_run: false },
    );
    assert!(result.is_err(), "a complete JSON value must not skip the ZIP checksum at EOF");
    let batch = load_import_batches(&vault, None, 16).unwrap().items.remove(0);
    assert_eq!(batch.status, "failed");
    assert_eq!(batch.imported_items, 1);
}
