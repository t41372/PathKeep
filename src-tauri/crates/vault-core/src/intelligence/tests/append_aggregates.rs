//! Insights after an append equal Insights built from scratch.
//!
//! ## Responsibilities
//! - Build one archive in four steps (a full build, then three appended backups) and another
//!   with the same visits in one full build, and compare every aggregate table row for row.
//! - Cross the append boundaries where an aggregate keeps state: a session and a search trail
//!   that continue into the next backup, a page reopened on later days, a site visited on days
//!   on both sides, a search that joins a family started earlier, paths through the same sites.
//!
//! ## Not responsible for
//! - Facts, daily rollups and sessions; their own equivalence tests cover them.
//! - Speed: the 14.4M benchmark measures that.
//!
//! The aggregates are updated in place after an append (only what the new visits touch), so
//! this test is what proves the update and the full build agree.
use super::super::intelligence_rebuild::run_core_intelligence;
use super::fixtures::{append_fixture_visit, seed_core_intelligence_fixture, sync_archive_totals};
use crate::models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest};
use crate::test_support::native::TestDatabase;
use vault_store::{Db, Value};

const DAY_MS: i64 = 86_400_000;
/// 2024-04-03T09:00:00Z, after the seed fixture's last visit.
const START_MS: i64 = 1_712_134_800_000;

const AGGREGATE_TABLES: &[&str] = &[
    "query_families",
    "query_family_queries",
    "refind_pages",
    "reopened_investigations",
    "path_flows",
    "compare_sets",
];

pub(super) struct Visit {
    id: i64,
    url: String,
    title: String,
    at: i64,
    from: Option<i64>,
    search: Option<String>,
}

/// Thirty days of mornings: a search, the docs it led to, two issues read back and forth (a
/// compare set), a blog, sometimes back to the docs. Queries reword one another so families
/// grow over time; the same few pages and sites come back day after day. The cuts fall inside
/// the morning trails of days 12, 18 and 24.
pub(super) fn visits() -> Vec<Visit> {
    let topics = ["rust async runtime", "rust async runtimes compared", "tokio vs smol"];
    let mut visits = Vec::new();
    let mut id = 100;
    for day in 0..30_i64 {
        let base = START_MS + day * DAY_MS;
        let query = topics[(day % 3) as usize];
        let steps: Vec<(String, String, Option<String>)> = vec![
            (
                format!("https://www.google.com/search?q={}", query.replace(' ', "+")),
                format!("{query} - Google Search"),
                Some(query.to_string()),
            ),
            (
                format!("https://docs.rs/tokio/latest/page-{}", day % 4),
                format!("tokio docs {}", day % 4),
                None,
            ),
            (format!("https://github.com/tokio-rs/tokio/issues/{}", day % 5), "Issue".into(), None),
            (
                format!("https://github.com/tokio-rs/tokio/issues/{}", day % 5 + 10),
                "Other issue".into(),
                None,
            ),
            (format!("https://github.com/tokio-rs/tokio/issues/{}", day % 5), "Issue".into(), None),
            (
                format!("https://blog.example.com/post-{}", day % 6),
                format!("Post {}", day % 6),
                None,
            ),
            (
                format!("https://docs.rs/tokio/latest/page-{}", day % 4),
                format!("tokio docs {}", day % 4),
                None,
            ),
        ];
        let mut previous = None;
        for (step, (url, title, search)) in steps.into_iter().enumerate() {
            if day % 7 == 3 && step == 6 {
                continue; // Some days end early.
            }
            visits.push(Visit {
                id,
                url,
                title,
                at: base + step as i64 * 4 * 60_000,
                from: if search.is_some() { None } else { previous },
                search,
            });
            previous = Some(id);
            id += 1;
        }
    }
    visits
}

pub(super) fn add(db: &TestDatabase, batch: &[Visit]) {
    let rows = batch
        .iter()
        .map(|v| (v.id, v.url.clone(), v.title.clone(), v.at, v.from, v.search.clone()))
        .collect::<Vec<_>>();
    db.write(move |s| {
        for (id, url, title, at, from, search) in &rows {
            append_fixture_visit(s, *id, url, title, *at, *from, search.as_deref());
        }
        Ok(())
    })
    .unwrap();
    sync_archive_totals(db);
}

pub(super) fn build(vault: &crate::Vault) -> String {
    run_core_intelligence(vault, &CoreIntelligenceRebuildRequest::default())
        .expect("build")
        .execution_mode
        .unwrap_or_default()
}

/// Every aggregate row of the published generation, without the generation and build time.
fn aggregates(vault: &crate::Vault) -> Vec<(String, Vec<String>)> {
    published(vault, AGGREGATE_TABLES, "aggregates")
}

/// Every row of `tables` in the published generation of `group`, without the generation and
/// build time, each as one sorted line.
pub(super) fn published(
    vault: &crate::Vault,
    tables: &[&str],
    group: &'static str,
) -> Vec<(String, Vec<String>)> {
    let db = TestDatabase::new(vault, Db::Intelligence);
    tables
        .iter()
        .map(|table| {
            let name = format!("{table}_rows");
            let rows = db
                .read(move |s| {
                    let columns = s
                        .table_columns(&name)?
                        .into_iter()
                        .filter(|column| column != "generation" && column != "computed_at")
                        .collect::<Vec<_>>();
                    let sql = format!(
                        "SELECT {} FROM {name} WHERE generation=(SELECT {group} FROM \
                         intelligence_generations WHERE profile_id='chrome:Default')",
                        columns.join(",")
                    );
                    let width = columns.len();
                    let mut rows = s.query(&sql, (), move |r| {
                        (0..width)
                            .map(|index| {
                                Ok(match r.get_value(index).map_err(vault_store::Error::from)? {
                                    Value::Real(real) => format!("{real:.6}"),
                                    other => format!("{other:?}"),
                                })
                            })
                            .collect::<vault_store::Result<Vec<_>>>()
                            .map(|values| values.join("|"))
                    })?;
                    rows.sort();
                    Ok(rows)
                })
                .unwrap();
            (table.to_string(), rows)
        })
        .collect()
}

pub(super) fn vault() -> crate::Vault {
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    crate::Vault::open_temp().unwrap().with_config(config)
}

#[test]
fn aggregates_after_appends_equal_a_full_build() {
    let all = visits();
    // Cut inside a day, mid-session and mid-trail, so state crosses every boundary.
    let cuts = [all.len() * 2 / 5 + 2, all.len() * 3 / 5 + 1, all.len() * 4 / 5 + 3];

    let appended = vault();
    let archive = TestDatabase::new(&appended, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, &all[..cuts[0]]);
    assert_eq!(build(&appended), "fallback-full", "the first build starts from nothing");
    for window in [cuts[0]..cuts[1], cuts[1]..cuts[2], cuts[2]..all.len()] {
        add(&archive, &all[window]);
        assert_eq!(build(&appended), "incremental", "each backup after that appends");
    }
    drop(archive);

    let whole = vault();
    let archive = TestDatabase::new(&whole, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, &all);
    drop(archive);
    build(&whole);

    let expected = aggregates(&whole);
    for (table, rows) in &expected {
        assert!(!rows.is_empty(), "the fixture gives {table} something to hold");
    }
    for ((table, got), (_, want)) in aggregates(&appended).iter().zip(&expected) {
        assert_eq!(got, want, "{table} after appends differs from a full build");
    }
}
