//! Responsibilities: enumerate ZIP central-directory names independently of export manifests.
//! Not responsible for: decompressing, importing, or validating arbitrary untrusted archives.
//! Small synthetic format-2 bundles have ZIP64 local entries but a classic end record.

use std::{fs, path::Path};

/// Makes the archive itself, rather than its production manifest, the membership oracle.
pub fn zip_entries(path: &Path) -> Vec<String> {
    let bytes = fs::read(path).unwrap();
    let end = bytes.windows(4).rposition(|s| s == b"PK\x05\x06").expect("ZIP end record");
    let u16_at = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
    let u32_at = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let count = u16_at(end + 10);
    let mut at = u32_at(end + 16);
    let mut names = Vec::new();
    for _ in 0..count {
        assert_eq!(&bytes[at..at + 4], b"PK\x01\x02", "central-directory record");
        let len = u16_at(at + 28);
        names.push(String::from_utf8(bytes[at + 46..at + 46 + len].to_vec()).unwrap());
        at += 46 + len + u16_at(at + 30) + u16_at(at + 32);
    }
    names.sort();
    names
}
