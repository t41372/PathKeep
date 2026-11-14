//! Responsibilities: substitute nondeterministic IDs in observed names, keeping every file.
//! Not responsible for: recognizing owned paths, filtering files, or generating expectations.
//! Aliases are per parent and retain multiplicity: a second segment or snapshot remains visible.

use std::collections::BTreeMap;

#[derive(Default)]
pub struct Names {
    aliases: BTreeMap<(String, &'static str), BTreeMap<String, usize>>,
}

impl Names {
    pub fn path(&mut self, path: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        for part in path.split('/') {
            let parent = parts.join("/");
            let normalized = if part.starts_with("maintenance.bak-") {
                self.alias(&parent, "maintenance.bak", part)
            } else if parent.ends_with("raw-snapshots/maintenance")
                && part.starts_with("rekey-maintenance-")
            {
                self.alias(&parent, "rekey-maintenance", part)
            } else if parent.ends_with("raw-snapshots/chrome%3ADefault")
                && part.contains('T')
                && part.ends_with("-1")
            {
                self.alias(&parent, "checkpoint", part)
            } else if parent.ends_with("audit/manifests")
                && part.len() == 10
                && part.as_bytes()[4] == b'-'
                && part.as_bytes()[7] == b'-'
            {
                self.alias(&parent, "date", part)
            } else if parent.contains("audit/manifests/")
                && part.starts_with("run-")
                && part.ends_with(".json")
            {
                let (run, hash) = part.trim_end_matches(".json").rsplit_once('-').unwrap();
                if hash.len() == 12 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                    format!("{run}-{}.json", self.alias(&parent, "hash", part))
                } else {
                    part.into()
                }
            } else if parent.ends_with("derived/search-index") && hex_id(part) {
                self.alias(&parent, "generation", part)
            } else if parent.contains("derived/search-index/") {
                match part.split_once('.') {
                    Some((id, extension)) if hex_id(id) => {
                        format!("{}.{}", self.alias(&parent, "segment", id), extension)
                    }
                    _ => part.into(),
                }
            } else {
                part.into()
            };
            parts.push(normalized);
        }
        parts.join("/")
    }

    fn alias(&mut self, parent: &str, kind: &'static str, value: &str) -> String {
        let siblings = self.aliases.entry((parent.into(), kind)).or_default();
        let next = siblings.len() + 1;
        let number = siblings.entry(value.into()).or_insert(next);
        format!("<{kind}-{number}>")
    }
}

fn hex_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
