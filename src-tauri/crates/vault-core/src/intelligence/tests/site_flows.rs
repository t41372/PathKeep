//! How a site's visits began and where its links led (`site_flows`), against a count by hand,
//! built at once and with an append that continues the session.
//!
//! Ways it can go wrong, each covered: moving between pages of one site counts as arriving; a
//! visit from a browser that does not record openers (no `from_visit` at all) counts as typed;
//! a search's results are not told apart from a link; an opener PathKeep never saw is dropped
//! or put on some site; the site a link led to is not counted as the opener's onward site; an
//! append in place counts the open session's earlier visits again, or loses the openers they
//! hold for the new ones.
use super::append_aggregates::{build, published, vault};
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture, sync_archive_totals};
use crate::intelligence::local_date_key;
use crate::test_support::native::TestDatabase;
use vault_store::Db;

/// 2024-04-03T10:00:00Z.
const START_MS: i64 = 1_712_138_400_000;

/// `(id, url, opener, search)`: the opener is the browser's id of the visit that opened it (the
/// fixture's ids are the browser's), `Some(0)` for none, `None` when the browser does not say.
const VISITS: &[(i64, &str, Option<i64>, Option<&str>)] = &[
    (101, "https://www.google.com/search?q=rust+docs", Some(0), Some("rust docs")),
    (102, "https://docs.rs/tokio/latest/a", Some(101), None),
    (103, "https://docs.rs/tokio/latest/b", Some(102), None),
    (104, "https://github.com/tokio-rs/tokio/issues/1", Some(103), None),
    (105, "https://docs.rs/tokio/latest/a", Some(0), None),
    (106, "https://blog.example.com/post", Some(999), None),
    (107, "https://github.com/tokio-rs/tokio/issues/2", Some(104), None),
    (108, "https://docs.rs/tokio/latest/c", None, None),
];

fn add(db: &TestDatabase, visits: &[(i64, &str, Option<i64>, Option<&str>)]) {
    let rows = visits
        .iter()
        .map(|(id, url, from, search)| (*id, url.to_string(), *from, search.map(str::to_string)))
        .collect::<Vec<_>>();
    db.write(move |s| {
        for (id, url, from, search) in &rows {
            let at = START_MS + (id - 100) * 60_000;
            append_fixture_visit(s, *id, url, "Page", at, *from, search.as_deref());
        }
        Ok(())
    })
    .unwrap();
    sync_archive_totals(db);
}

/// The rows a count by hand gives, as `published` prints them.
fn by_hand() -> Vec<String> {
    let row = |at: i64, site: &str, kind: &str, other: &str| {
        format!(
            "Text(\"chrome:Default\")|Text(\"{}\")|Text(\"{site}\")|Text(\"{kind}\")|\
             Text(\"{other}\")|Integer(1)",
            local_date_key(at)
        )
    };
    let seeded = 1_711_929_660_000;
    let day = START_MS + 60_000;
    let mut rows = vec![
        // The seed: its search's result on GitHub; its other visits' browser does not say.
        row(seeded, "github.com", "search", "google.com"),
        row(seeded, "google.com", "onward", "github.com"),
        // 101: a search typed into the browser.
        row(day, "google.com", "direct", ""),
        // 102: opened from its results, so Google's onward site too; 103 stays on docs.rs.
        row(day, "docs.rs", "search", "google.com"),
        row(day, "google.com", "onward", "docs.rs"),
        // 104: a link on docs.rs.
        row(day, "github.com", "link", "docs.rs"),
        row(day, "docs.rs", "onward", "github.com"),
        // 105: typed again.
        row(day, "docs.rs", "direct", ""),
        // 106: opened from a page PathKeep never saw.
        row(day, "example.com", "link", ""),
        // 107 stays on github.com; 108's browser does not say what opened it.
    ];
    rows.sort();
    rows
}

fn site_flows(vault: &crate::Vault) -> Vec<String> {
    published(vault, &["site_flows"], "navigation").remove(0).1
}

#[test]
fn site_flows_match_a_count_by_hand_built_at_once_and_appended() {
    let whole = vault();
    let archive = TestDatabase::new(&whole, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, VISITS);
    drop(archive);
    build(&whole);
    assert_eq!(site_flows(&whole), by_hand(), "a build from scratch");

    // Cut after 104: the session, and the openers 105 to 107 need, continue into the append.
    let appended = vault();
    let archive = TestDatabase::new(&appended, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, &VISITS[..4]);
    assert_eq!(build(&appended), "fallback-full");
    add(&archive, &VISITS[4..]);
    assert_eq!(build(&appended), "incremental");
    drop(archive);
    assert_eq!(site_flows(&appended), by_hand(), "a build with an append");
}
