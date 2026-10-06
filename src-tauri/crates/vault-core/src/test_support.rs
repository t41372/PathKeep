//! Shared debug overrides for deterministic backend tests and demos.
//!
//! Responsibilities: prevent autonomous backend fetches in isolated debug runs.
//! Not responsible for: user-requested AI provider calls or production network policy.

// Failure modes to test before adding the switch:
// (a) A release build honors the override.
// (b) A forgotten fetch path still connects (og:image, page content, or updates).
// (c) Blocking becomes a real failure that triggers retries/backoff loops.
// (d) Blank or other values enable the override instead of only exactly `1`.
/// Keeps autonomous fetches offline in tests and demos without affecting shipped builds.
///
/// Uses the same release guard as vault-platform's `test_keyring_dir()` override.
pub fn network_is_blocked() -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    std::env::var("PATHKEEP_TEST_BLOCK_NETWORK").is_ok_and(|value| value == "1")
}

/// Slows each exported row so an end-to-end test can watch an export's progress and stop it;
/// the fixture's twenty thousand visits otherwise export before the first progress poll.
///
/// Debug builds only, like `network_is_blocked`; read once, since it is checked per row.
pub fn export_row_delay() -> Option<std::time::Duration> {
    if !cfg!(debug_assertions) {
        return None;
    }
    static DELAY: std::sync::OnceLock<Option<std::time::Duration>> = std::sync::OnceLock::new();
    *DELAY.get_or_init(|| {
        let micros = std::env::var("PATHKEEP_TEST_EXPORT_ROW_DELAY_US").ok()?.parse().ok()?;
        (micros > 0).then(|| std::time::Duration::from_micros(micros))
    })
}

#[cfg(test)]
pub(crate) fn network_test_child() -> bool {
    std::env::var_os("PATHKEEP_NETWORK_TEST_CHILD").is_some()
}

/// Isolates environment overrides from concurrently running socket tests.
#[cfg(test)]
pub(crate) fn run_network_test(test_name: &str, value: Option<&str>) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    command.args(["--exact", test_name, "--nocapture"]);
    command.env("PATHKEEP_NETWORK_TEST_CHILD", "1");
    command.env_remove("PATHKEEP_TEST_BLOCK_NETWORK");
    if let Some(value) = value {
        command.env("PATHKEEP_TEST_BLOCK_NETWORK", value);
    }
    assert!(command.status().expect("isolated network test").success());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_override_is_ignored_without_debug_assertions() {
        if network_test_child() {
            assert_eq!(network_is_blocked(), cfg!(debug_assertions));
        } else {
            run_network_test(
                "test_support::tests::network_override_is_ignored_without_debug_assertions",
                Some("1"),
            );
        }
    }

    #[test]
    fn only_exactly_one_blocks_network() {
        if network_test_child() {
            let enabled = std::env::var("PATHKEEP_TEST_BLOCK_NETWORK").as_deref() == Ok("1");
            assert_eq!(network_is_blocked(), cfg!(debug_assertions) && enabled);
        } else {
            for value in [
                None,
                Some(""),
                Some("0"),
                Some("true"),
                Some("01"),
                Some(" 1"),
                Some("1 "),
                Some("1"),
            ] {
                run_network_test("test_support::tests::only_exactly_one_blocks_network", value);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod store;

#[cfg(any(test, feature = "test-fixtures"))]
pub mod legacy_fixtures;

/// Durable handoff progress remains observable after acknowledged rows are pruned.
#[cfg(test)]
pub(crate) fn search_sequence(vault: &crate::Vault) -> i64 {
    vault
        .store()
        .read(vault_store::Db::Archive, |s| {
            s.query_one(
                "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='search_outbox'),0)",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )
        })
        .unwrap()
}
/// Compares the physical index with independent canonical visibility after one real cycle.
#[cfg(test)]
pub(crate) fn assert_search_handoff(vault: &crate::Vault) {
    vault.search().cycle().unwrap();
    let expected=vault.store().read(vault_store::Db::Archive,|s|s.query("SELECT DISTINCT u.page_id FROM visits v JOIN urls u ON u.id=v.url_id WHERE v.reverted_at IS NULL ORDER BY u.page_id",(),|r|Ok(r.get::<i64>(0)? as u64))).unwrap();
    assert_eq!(vault.search().indexed_ids().unwrap(), expected);
    assert_eq!(vault.search().state().applied_seq, search_sequence(vault) as u64);
}
#[cfg(any(test, feature = "test-fixtures"))]
pub mod native;
#[cfg(any(test, feature = "test-fixtures"))]
pub mod search;
