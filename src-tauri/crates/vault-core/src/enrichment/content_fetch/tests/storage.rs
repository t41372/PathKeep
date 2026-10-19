//! Responsibilities: a queued fetch whose consent was turned off is parked, not run.
//! Not responsible for: other test groups or production code.
//! Failure modes: a page fetched after the user turned content fetch off.
use super::*;

#[test]
fn execute_content_fetch_job_parks_when_consent_disabled() {
    let vault = crate::Vault::open_temp().unwrap();
    let _paths = vault.paths().clone();
    let connection = TestDatabase::new(&vault, Db::Intelligence);

    let payload = EnrichmentJobPayload {
        history_id: Some(8),
        profile_id: Some("chrome:Default".to_string()),
        url: "https://example.com/denied".to_string(),
        title: None,
    };
    let job_id = enqueue_native(&connection, &payload).expect("enqueue");

    // Master switch OFF (default): the job is parked (cancelled), not run, and nothing is fetched.
    let off = AppConfig { initialized: true, ..AppConfig::default() };
    let fetcher = FakeFetcher::new();
    assert_eq!(
        execute_content_fetch_job_with_fetcher(
            &{
                seed_page(&vault, &payload.url);
                vault.with_config(off.clone())
            },
            job_id,
            &fetcher
        )
        .expect("execute"),
        ContentFetchJobOutcome::Ran
    );
    let state: String = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            Ok(row.get(0)?)
        })
        .expect("state");
    assert_eq!(state, "cancelled");
}
