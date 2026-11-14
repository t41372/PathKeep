//! An independent list of everything PathKeep may leave under its data root.
//!
//! Tests that walk a real data root use this list, not the production table,
//! to decide whether a path is known. The two are written by different hands
//! on purpose: a store that is added to the product but to neither list is
//! caught by the compiler (the table is exhaustive); a store that reaches the
//! disk without a row in the table is caught here, because this list does not
//! know it either and the walk fails. Adding a path therefore takes two
//! deliberate edits, one in each place.
//!
//! Responsibilities: classify a path relative to the data root; say whether
//! its content must be unreadable without the key when the archive is
//! encrypted; say whether "delete all data" may leave it behind.
//! Not responsible for: what any operation does with a path (the production
//! table), or for reading file contents (the tests that use this do).

use std::path::Path;

/// What must be true of a file's bytes when the archive is encrypted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtRest {
    /// Holds user content and must not be readable without the key.
    Encrypted,
    /// Holds user content and is written in the clear today. A known gap
    /// (`WORK-ENCRYPT-AT-REST-ALL`): tests count these so the list can only
    /// shrink, and a new entry here needs a decision, not a shrug.
    PlainKnownGap,
    /// Carries no archived content: locks, markers, key wrapping, settings,
    /// logs, downloaded models.
    NoContent,
    /// A copy kept as it was when taken (safety snapshots, previous
    /// generations, files of older versions). Its protection is whatever the
    /// source had; the at-rest scan reports it separately.
    AsCopied,
}

#[derive(Clone, Copy, Debug)]
pub struct Known {
    /// `/`-separated, relative to the data root. `*` matches within one
    /// segment, `**` matches any number of segments including none.
    pub pattern: &'static str,
    pub what: &'static str,
    pub at_rest: AtRest,
    /// May still exist after "delete all data" has finished.
    pub survives_wipe: bool,
}

const fn k(pattern: &'static str, what: &'static str, at_rest: AtRest, survives_wipe: bool) -> Known {
    Known { pattern, what, at_rest, survives_wipe }
}

use AtRest::{AsCopied, Encrypted, NoContent, PlainKnownGap};

/// First match wins, so a specific pattern goes before a general one.
pub const KNOWN: &[Known] = &[
    // Settings and App Lock.
    k("config.json", "app settings and the archive's format", NoContent, false),
    k("app-lock-state.json", "App Lock state", NoContent, false),
    k("app-lock-passcode.json", "App Lock passcode hash", NoContent, false),
    // Browsing history: original data.
    k("archive/history-vault.db", "browsing history", Encrypted, false),
    k("archive/source-evidence.db", "raw browser evidence", Encrypted, false),
    // Agent history: original evidence, its read model, its search index.
    k("archive/agent-history.db", "agent chat evidence", Encrypted, false),
    k("derived/agent-history-read-model.db", "agent chat read model", Encrypted, false),
    k("derived/agent-history-search/**", "agent chat search index", Encrypted, false),
    // Keys.
    k("archive/keys.envelope", "password-wrapped data key", NoContent, false),
    k("archive/keys.envelope.pending", "envelope before its first promotion", NoContent, false),
    k("archive/.keys.envelope.pk-tmp", "envelope being replaced", NoContent, false),
    // Locks and crash markers.
    k("archive/.pk-owner.lock", "owner process lock", NoContent, true),
    k("archive/.pk-archive-write.lock", "cross-process write lock", NoContent, true),
    k("archive/.pk-import-journal.json", "install journal", NoContent, false),
    k("archive/.pk-import-journal.legacy-stale.json", "a v0.3 journal set aside", NoContent, false),
    k("archive/.pk-rekey-journal.json", "v0.3 rekey journal", NoContent, false),
    k("archive/.pk-restore-journal.json", "v0.3 restore journal", NoContent, false),
    k(".pk-wipe-in-progress.json", "delete-all marker", NoContent, false),
    // Derived stores.
    k("derived/intelligence.db", "Insights read model, queues, and the user's site and search-engine edits", Encrypted, false),
    k("derived/agent.db", "Ask chats", Encrypted, false),
    k("derived/scratch.db", "stand-in for temporary tables", Encrypted, false),
    k("derived/search-index/**", "browsing search index", Encrypted, false),
    k("derived/vectors/**", "semantic index planes", PlainKnownGap, false),
    k("sidecars/intelligence-blobs/**", "saved page text", PlainKnownGap, false),
    k("sidecars/semantic-index/**", "leftover of an older vector layout", PlainKnownGap, false),
    // Copies.
    k("raw-snapshots/maintenance/**", "whole-archive safety snapshots", AsCopied, false),
    k("raw-snapshots/rekey/**", "v0.3 safety snapshots", AsCopied, false),
    k("raw-snapshots/*/*.sqlite", "v0.3 safety snapshots", AsCopied, false),
    k("raw-snapshots/**", "copies of the browsers' own history files", PlainKnownGap, false),
    k("maintenance.bak-*/**", "previous generation kept after a restore, import or conversion", AsCopied, false),
    k("*.bak-*", "v0.3 import backstop", AsCopied, false),
    k("archive/*.bak-*", "v0.3 import backstop", AsCopied, false),
    k("archive/*.sqlite", "v0.3 database kept after conversion", AsCopied, false),
    k("derived/*.sqlite", "v0.3 database kept after conversion", AsCopied, false),
    k("quarantine/**", "v0.3 quarantined originals", AsCopied, false),
    k("staging/**", "an operation's working files", AsCopied, false),
    // Records of what PathKeep did.
    k("audit/**", "run manifests and import audits", PlainKnownGap, false),
    k("schedule/**", "scheduled-run attempts and logs", NoContent, false),
    k("integrations/**", "generated Insights bundle", PlainKnownGap, false),
    k("exports/**", "reserved for exports", PlainKnownGap, false),
    // Things that hold nothing archived.
    k("models/**", "downloaded embedding models", NoContent, false),
    k("vault.hold", "pre-0.5 Stronghold file", NoContent, false),
    k("stronghold-salt.txt", "pre-0.5 Stronghold file", NoContent, false),
    k("ipc/**", "owner endpoint", NoContent, true),
    k("logs/**", "logs, kept on purpose", NoContent, true),
    k("diagnostics/**", "crash reports, kept on purpose", NoContent, true),
];

/// Files a database engine keeps beside a database, classified as that database.
const COMPANIONS: &[&str] = &["-wal", "-shm"];

fn segment_matches(pattern: &str, segment: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == segment,
        Some((head, tail)) => {
            segment.len() >= head.len() + tail.len() && segment.starts_with(head) && segment.ends_with(tail)
        }
    }
}

fn matches(pattern: &[&str], path: &[&str]) -> bool {
    match (pattern.first(), path.first()) {
        (None, None) => true,
        (Some(&"**"), _) => matches(&pattern[1..], path) || (!path.is_empty() && matches(pattern, &path[1..])),
        (Some(p), Some(s)) => segment_matches(p, s) && matches(&pattern[1..], &path[1..]),
        _ => false,
    }
}

/// The entry that claims `relative` (a `/`-separated path under the data
/// root), or `None` when nothing PathKeep is known to write goes there.
pub fn classify(relative: &str) -> Option<&'static Known> {
    let base = COMPANIONS
        .iter()
        .find_map(|suffix| relative.strip_suffix(suffix).filter(|stem| stem.ends_with(".db")))
        .unwrap_or(relative);
    let path: Vec<&str> = base.split('/').collect();
    KNOWN.iter().find(|known| matches(&known.pattern.split('/').collect::<Vec<_>>(), &path))
}

/// Every file under `root`, as a `/`-separated relative path, sorted.
/// Directories are not listed; an empty directory holds nothing to classify.
pub fn files_under(root: &Path) -> std::io::Result<Vec<String>> {
    fn visit(dir: &Path, root: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                visit(&entry.path(), root, out)?;
            } else {
                let relative = entry.path().strip_prefix(root).expect("under root").to_path_buf();
                out.push(relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

/// The files under `root` that nothing claims. A test over a real data root
/// asserts this is empty.
pub fn unknown_files(root: &Path) -> std::io::Result<Vec<String>> {
    Ok(files_under(root)?.into_iter().filter(|path| classify(path).is_none()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn what(path: &str) -> Option<&'static str> {
        classify(path).map(|known| known.what)
    }

    #[test]
    fn databases_and_their_companions_are_one_entry() {
        assert_eq!(what("archive/history-vault.db"), Some("browsing history"));
        assert_eq!(what("archive/history-vault.db-wal"), Some("browsing history"));
        assert_eq!(what("archive/agent-history.db-shm"), Some("agent chat evidence"));
        assert_eq!(what("derived/agent.db"), Some("Ask chats"));
        // Ask chats and agent chats must never be taken for each other.
        assert_ne!(what("derived/agent.db"), what("archive/agent-history.db"));
    }

    #[test]
    fn a_specific_copy_is_not_taken_for_a_browser_checkpoint() {
        assert_eq!(classify("raw-snapshots/maintenance/2026-11-14/history-vault.db").unwrap().at_rest, AsCopied);
        assert_eq!(classify("raw-snapshots/rekey/old.sqlite").unwrap().at_rest, AsCopied);
        assert_eq!(classify("raw-snapshots/chrome-default/2026-11-14-run7/History").unwrap().at_rest, PlainKnownGap);
    }

    #[test]
    fn nothing_claims_a_file_pathkeep_does_not_write() {
        for stray in [
            "notes.txt",
            "archive/another.db",
            "archive/another.db-wal",
            "derived/agent-history.db",
            "derived/new-kind/store.db",
            "new-kind/store.db",
            "archive/keys.envelope.old",
            "my-export.pathkeep",
        ] {
            assert!(classify(stray).is_none(), "{stray} was claimed by {:?}", what(stray));
        }
    }

    #[test]
    fn a_companion_suffix_does_not_launder_an_unknown_file() {
        // `-wal` is only stripped from a database name.
        assert!(classify("archive/keys.envelope-wal").is_none());
        assert!(classify("config.json-shm").is_none());
    }

    #[test]
    fn only_runtime_files_and_logs_outlive_a_wipe() {
        let kept: Vec<&str> = KNOWN.iter().filter(|known| known.survives_wipe).map(|known| known.pattern).collect();
        assert_eq!(kept, ["archive/.pk-owner.lock", "archive/.pk-archive-write.lock", "ipc/**", "logs/**", "diagnostics/**"]);
        // Nothing that holds content may be on that list.
        assert!(KNOWN.iter().filter(|known| known.survives_wipe).all(|known| known.at_rest == NoContent));
    }

    #[test]
    fn the_known_plaintext_list_is_exactly_these() {
        let gaps: Vec<&str> = KNOWN.iter().filter(|known| known.at_rest == PlainKnownGap).map(|known| known.pattern).collect();
        assert_eq!(
            gaps,
            [
                "derived/vectors/**",
                "sidecars/intelligence-blobs/**",
                "sidecars/semantic-index/**",
                "raw-snapshots/**",
                "audit/**",
                "integrations/**",
                "exports/**",
            ]
        );
    }

    #[test]
    fn no_pattern_is_shadowed_by_an_earlier_one() {
        // A literal path must be claimed by its own entry, not by a wildcard above it.
        for (index, known) in KNOWN.iter().enumerate() {
            if known.pattern.contains('*') {
                continue;
            }
            let claimed = classify(known.pattern).expect("a literal pattern matches itself");
            assert_eq!(claimed.pattern, known.pattern, "entry {index} is shadowed");
        }
    }
}
