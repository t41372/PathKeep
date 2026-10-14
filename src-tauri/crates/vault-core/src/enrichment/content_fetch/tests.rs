//! Responsibilities: the security and consent gates of content fetch: no request without the
//! user's consent, none to a private address, none while the network switch blocks it.
//! Not responsible for: extraction quality, rate limits or other network behaviour.
//! Failure modes: a page fetched without consent, a redirect followed into the local network,
//! or a test run that reaches the network.
use super::*;
use crate::test_support::native::TestDatabase;
use vault_store::{Db, Priority};

use crate::enrichment::rate_limit;
use crate::intelligence_runtime::enqueue_content_fetch_job;
use crate::models::{AppConfig, ContentFetchDomainRule, ContentFetchExtractorPreference};
use std::collections::HashMap;

/// A scripted fetcher: maps a URL to a canned outcome so every routing branch is deterministic.
struct FakeFetcher {
    html: HashMap<String, FetchOutcome>,
}

impl FakeFetcher {
    fn new() -> Self {
        Self { html: HashMap::new() }
    }
    fn with_html(mut self, url: &str, outcome: FetchOutcome) -> Self {
        self.html.insert(url.to_string(), outcome);
        self
    }
}

impl Fetcher for FakeFetcher {
    fn networked(&self) -> bool {
        false
    }
    fn fetch_html(&self, url: &str) -> FetchOutcome {
        self.html.get(url).cloned().unwrap_or(FetchOutcome::Failed {
            status: "fetch-error".to_string(),
            http_status: None,
            detail: "no fixture".to_string(),
        })
    }
    fn fetch_json(&self, _url: &str, _cap: usize) -> FetchOutcome {
        FetchOutcome::Failed {
            status: "fetch-error".to_string(),
            http_status: None,
            detail: "no fixture".to_string(),
        }
    }
}

fn consenting_config() -> AppConfig {
    let mut config = AppConfig { initialized: true, ..AppConfig::default() };
    config.ai.content_fetch_enabled = true;
    config
}

/// The production fetcher without the private-address check, so that only the network switch
/// can stop a request to the loopback listener; the real address check is tested by
/// `the_production_fetcher_does_not_follow_a_redirect_to_a_private_address`.
fn shared_fetcher() -> SharedClientFetcher {
    SharedClientFetcher {
        client: crate::og_images_fetch::build_guarded_fetch_client(|_| false).expect("client"),
    }
}

#[test]
fn network_switch_blocks_content_fetch_without_connecting() {
    if !cfg!(debug_assertions) {
        return;
    }
    if !crate::test_support::network_test_child() {
        crate::test_support::run_network_test(
            "enrichment::content_fetch::tests::network_switch_blocks_content_fetch_without_connecting",
            Some("1"),
        );
        return;
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    listener.set_nonblocking(true).expect("nonblocking listener");
    let url = format!("http://{}/article", listener.local_addr().unwrap());
    let fetcher = shared_fetcher();
    for outcome in [fetcher.fetch_html(&url), fetcher.fetch_json(&url, 1024)] {
        match outcome {
            FetchOutcome::Failed { status, http_status, .. } => {
                assert_eq!(status, "blocked");
                assert!(http_status.is_none());
            }
            other => panic!("expected blocked, got {other:?}"),
        }
    }
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);

    // Even an exhausted API budget must produce a terminal blocked result,
    // rather than deferring a job that will never be allowed to fetch.
    for _ in 0..100 {
        let _ = rate_limit::acquire_host_token("api.github.com");
    }
    let result = run_content_fetch_with_fetcher(&fetcher, &payload("https://github.com/o/r", 1));
    assert_eq!(result.content_source, "github-repo");
    assert_eq!(result.enrichment.status, "blocked");
    assert!(result.refetch_after.is_none());

    let vault = crate::Vault::open_temp().unwrap();
    let _paths = vault.paths().clone();
    let connection = TestDatabase::new(&vault, Db::Intelligence);
    let job_id =
        enqueue_native(&connection, &payload("https://github.com/o/r", 1)).expect("enqueue");
    assert_eq!(
        execute_content_fetch_job_with_fetcher(
            &{
                seed_page(&vault, "https://github.com/o/r");
                vault.with_config(consenting_config().clone())
            },
            job_id,
            &fetcher
        )
        .expect("execute"),
        ContentFetchJobOutcome::Ran,
    );
    let state: String = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            Ok(row.get(0)?)
        })
        .expect("job state");
    assert_eq!(state, "succeeded");
    let (status, refetch): (String, Option<String>) = connection.query_row(
        "SELECT fetch_status, refetch_after FROM page_content_enrichments WHERE canonical_url = 'https://github.com/o/r' AND content_source = 'github-repo'", (),
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).expect("stored blocked result");
    assert_eq!(status, "blocked");
    assert!(refetch.is_none());
}

/// A unique per-test host so the process-global rate-limit registry never leaks between tests.
fn unique_host(tag: &str) -> String {
    format!("wenrich-{tag}-{}.example", std::process::id())
}

fn payload(url: &str, history_id: i64) -> EnrichmentJobPayload {
    EnrichmentJobPayload {
        history_id: Some(history_id),
        profile_id: Some("chrome:Default".to_string()),
        url: url.to_string(),
        title: None,
    }
}

fn seed_page(vault: &crate::Vault, url: &str) {
    let url = url.to_owned();
    vault.write(Db::Archive,Priority::Batch,move |s| {
        let id=s.query_one("INSERT INTO pages(url,canonical_url) VALUES(?1,?2) ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",params![url,crate::pages::canonical_key(&url)],|r|Ok(r.get::<i64>(0)?))?;
        crate::archive::search_documents::text_changed(s,id)?;Ok(())
    }).unwrap();
}
fn enqueue_native(db: &TestDatabase, payload: &EnrichmentJobPayload) -> anyhow::Result<i64> {
    let payload = payload.clone();
    db.write(move |s| enqueue_content_fetch_job(s, &payload))
}
mod transport;

mod storage;

mod routing;
