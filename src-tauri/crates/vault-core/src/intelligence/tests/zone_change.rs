//! Insights keep their own time zone (`docs/architecture/insights-time-zone.md`).
//!
//! Ways it can go wrong, each covered: a backup with no zone chosen puts its visits on days of
//! the computer's zone while the older ones stay on days of the zone the data was built in (a
//! day half in each, visits dropped or counted twice); a newly chosen zone is appended in place
//! instead of rebuilt; the rebuild in the new zone differs from building there from scratch.
//!
//! The visits fall across Taipei's midnight (and in Los Angeles' morning), so every day, hour,
//! session path and day count depends on the zone. Modal and CI run in UTC, so a build that took
//! the computer's zone would put them on other days than Taipei's.
use super::super::build_state::TABLES;
use super::append_aggregates::{Visit, add, build, published_rows, visits};
use super::fixtures::seed_core_intelligence_fixture;
use crate::models::{AppConfig, ArchiveMode};
use crate::test_support::native::TestDatabase;
use vault_store::Db;

const TAIPEI: &str = "Asia/Taipei";
const LOS_ANGELES: &str = "America/Los_Angeles";

/// The fixture's mornings moved to 15:48Z: 23:48 in Taipei, so each trail crosses its midnight.
const LATER_MS: i64 = (6 * 60 + 48) * 60_000;

/// The tables that sit beside the generation tables and are published with them.
const SIDE_TABLES: &[(&str, &str)] = &[
    ("intelligence_seen_domains", "facts"),
    ("intelligence_day_counts", "daily"),
    ("visit_assignments", "navigation"),
    ("session_domains", "navigation"),
    ("session_paths", "navigation"),
    ("site_flows_rows", "navigation"),
];

fn late_visits() -> Vec<Visit> {
    let mut visits = visits();
    for visit in &mut visits {
        visit.at += LATER_MS;
    }
    visits
}

fn config(zone: Option<&str>) -> AppConfig {
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        insights_time_zone: zone.map(str::to_owned),
        ..AppConfig::default()
    }
}

fn seeded(zone: Option<&str>, visits: &[Visit]) -> (crate::Vault, TestDatabase) {
    let vault = crate::Vault::open_temp().unwrap().with_config(config(zone));
    let archive = TestDatabase::new(&vault, Db::Archive);
    archive
        .write(|s| {
            seed_core_intelligence_fixture(s);
            Ok(())
        })
        .unwrap();
    add(&archive, visits);
    (vault, archive)
}

fn built_from_scratch(zone: &str, visits: &[Visit]) -> crate::Vault {
    let (vault, archive) = seeded(Some(zone), visits);
    drop(archive);
    assert_eq!(build(&vault), "fallback-full");
    vault
}

fn published_zone(vault: &crate::Vault) -> Option<String> {
    TestDatabase::new(vault, Db::Intelligence)
        .read(|s| {
            Ok(s.query_one(
                "SELECT time_zone FROM intelligence_generations WHERE profile_id='chrome:Default'",
                (),
                |r| Ok(r.get::<Option<String>>(0)?),
            )?)
        })
        .unwrap()
}

/// Every row Insights publish, without generations and build times.
fn everything(vault: &crate::Vault) -> Vec<(String, Vec<String>)> {
    let generation_tables = TABLES.iter().map(|(table, group)| (format!("{table}_rows"), *group));
    let side_tables = SIDE_TABLES.iter().map(|(table, group)| (table.to_string(), *group));
    generation_tables
        .chain(side_tables)
        .map(|(table, group)| (table.clone(), published_rows(vault, table, group)))
        .collect()
}

fn assert_as_built_from_scratch(vault: &crate::Vault, zone: &str, visits: &[Visit], when: &str) {
    let want = everything(&built_from_scratch(zone, visits));
    for ((table, got), (_, want)) in everything(vault).iter().zip(&want) {
        assert_eq!(got, want, "{table} {when} differs from a build from scratch in {zone}");
    }
}

fn day_keys(vault: &crate::Vault) -> Vec<String> {
    published_rows(vault, "intelligence_day_counts".into(), "daily")
        .into_iter()
        .map(|row| row.split('|').nth(1).unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn a_backup_keeps_the_zone_and_a_new_zone_rebuilds() {
    let all = late_visits();
    let (first, second) = (all.len() / 3 + 2, all.len() * 2 / 3 + 1);

    // The first build in Taipei, chosen in Settings.
    let (taipei, archive) = seeded(Some(TAIPEI), &all[..first]);
    assert_eq!(build(&taipei), "fallback-full");
    assert_eq!(published_zone(&taipei).as_deref(), Some(TAIPEI));

    // No zone chosen any more (the computer's is UTC here): a backup still appends in Taipei.
    let unchosen = taipei.with_config(config(None));
    add(&archive, &all[first..second]);
    assert_eq!(build(&unchosen), "incremental", "a backup does not change the zone");
    assert_eq!(published_zone(&unchosen).as_deref(), Some(TAIPEI));
    assert_as_built_from_scratch(&unchosen, TAIPEI, &all[..second], "after a backup");

    // Los Angeles chosen, with a backup waiting: everything is built again there.
    let los_angeles = taipei.with_config(config(Some(LOS_ANGELES)));
    add(&archive, &all[second..]);
    drop(archive);
    assert_eq!(build(&los_angeles), "fallback-full", "a new zone rebuilds");
    assert_eq!(published_zone(&los_angeles).as_deref(), Some(LOS_ANGELES));
    assert_as_built_from_scratch(&los_angeles, LOS_ANGELES, &all, "after the zone changed");

    // The zone mattered: the same visits fall on other days in Taipei.
    assert_ne!(day_keys(&los_angeles), day_keys(&built_from_scratch(TAIPEI, &all)));
}
