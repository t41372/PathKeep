//! Responsibilities: observe every entry in a synthetic tree, including empty directories.
//! Not responsible for: recognizing production paths, following symlinks or deciding admission.
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, PartialEq, Eq)]
pub struct Census {
    pub hash: String,
    pub entries: BTreeMap<String, String>,
}

pub fn census(root: &Path, hash: impl Fn(&[u8]) -> String) -> Census {
    fn visit(
        root: &Path,
        path: &Path,
        rows: &mut BTreeMap<String, String>,
        hash: &impl Fn(&[u8]) -> String,
    ) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        let kind = if metadata.file_type().is_symlink() {
            format!("link:{}", fs::read_link(path).unwrap().display())
        } else if metadata.is_dir() {
            "directory".into()
        } else if metadata.is_file() {
            format!("file:{}:{}", metadata.len(), hash(&fs::read(path).unwrap()))
        } else {
            "special".into()
        };
        #[cfg(unix)]
        let kind = {
            use std::os::unix::fs::PermissionsExt;
            format!("{kind}:mode={:o}", metadata.permissions().mode() & 0o777)
        };
        rows.insert(relative, kind);
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), rows, hash);
            }
        }
    }
    let mut entries = BTreeMap::new();
    if root.exists() {
        visit(root, root, &mut entries, &hash);
    }
    let mut encoded = Vec::new();
    for (path, value) in &entries {
        encoded.extend_from_slice(&(path.len() as u64).to_le_bytes());
        encoded.extend_from_slice(path.as_bytes());
        encoded.extend_from_slice(&(value.len() as u64).to_le_bytes());
        encoded.extend_from_slice(value.as_bytes());
    }
    Census { hash: hash(&encoded), entries }
}
