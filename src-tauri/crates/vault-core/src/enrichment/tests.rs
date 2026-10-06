//! Responsibilities: retain the parent module's behavior regressions on native storage.
//! Not responsible for: product implementation or external network execution.
//! Failure modes: incorrect boundaries, lost records, replay drift and invalid state transitions.
use super::{
    EMBEDDING_TEXT_TOKEN_CAP, ENRICHMENT_SUMMARY_CHAR_CAP, EnrichmentResult, SiteAdapterResult,
    StoredEnrichment, build_embedding_content_from_parts, build_enrichment_result_from_html,
    build_enrichment_summary, cap_embedding_text, default_enrichment_refetch_after_for_status,
    enrichment_failure_message, enrichment_is_terminal_failure, execute_enrichment_job_by_id,
    fail_unknown_enrichment_plugin, finish_claimed_enrichment_job, finish_if_enrichment_cancelled,
    load_best_enrichment_map_by_history_ids, merge_site_adapter_result,
    preferred_embedding_content, store_enrichment, title_normalization_enrichment,
};
use crate::test_support::native::TestDatabase;
use crate::{
    intelligence_blobs::store_readable_text_blob,
    intelligence_runtime::{
        ENRICHMENT_JOB_TYPE, EnrichmentJobPayload, built_in_enrichment_plugin,
        enqueue_enrichment_job,
    },
    models::{READABLE_CONTENT_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_ID},
};

use vault_store::{Db, Priority, params_owned as params};

#[test]
fn percent_encoded_cjk_url_slug_decodes_to_utf8_not_mojibake() {
    // Regression: `%E4%B8%AD%E6%96%87` is the UTF-8 encoding of "中文". The
    // decoder must reassemble the raw bytes into one string, not emit one
    // Latin-1 codepoint per byte (which produced mojibake `ä¸­æ–‡`).
    let enrichment =
        title_normalization_enrichment("https://example.com/articles/%E4%B8%AD%E6%96%87", None);
    assert_eq!(enrichment.readable_title.as_deref(), Some("中文"));
}

#[test]
fn build_embedding_content_includes_only_non_empty_enrichment_fields() {
    let enriched = build_embedding_content_from_parts(
        "chrome:Default",
        "https://example.com/docs/intro",
        Some("Intro"),
        "2026-04-17T00:00:00Z",
        Some("Readable Intro"),
        Some("A capped summary"),
        Some("First paragraph"),
    );
    assert!(enriched.contains("Readable title: Readable Intro"));
    assert!(enriched.contains("Summary: A capped summary"));
    assert!(enriched.contains("Readable text:\nFirst paragraph"));

    let without_enrichment = build_embedding_content_from_parts(
        "chrome:Default",
        "https://example.com/docs/intro",
        None,
        "2026-04-17T00:00:00Z",
        Some("   "),
        Some("  "),
        Some("  "),
    );
    assert!(without_enrichment.contains("Title: (untitled)"));
    assert!(!without_enrichment.contains("Readable title:"));
    assert!(!without_enrichment.contains("Summary:"));
    assert!(!without_enrichment.contains("Readable text:"));
}

#[test]
fn cap_embedding_text_truncates_long_text_to_the_token_budget() {
    // Short text is unchanged.
    assert_eq!(cap_embedding_text("a short body"), "a short body");
    // A body longer than the ~512-token char budget is truncated; never fed whole into the window.
    let long = "x ".repeat(5_000); // ~10k chars, well over 512 * 4 = 2048
    let capped = cap_embedding_text(&long);
    assert!(capped.chars().count() <= EMBEDDING_TEXT_TOKEN_CAP * 4);
    assert!(capped.chars().count() < long.chars().count());
}

#[test]
fn build_enrichment_summary_caps_and_normalizes() {
    // Whitespace is collapsed; an empty result is None.
    assert_eq!(build_enrichment_summary(Some("  a   b  ")).as_deref(), Some("a b"));
    assert_eq!(build_enrichment_summary(Some("   ")), None);
    assert_eq!(build_enrichment_summary(None), None);
    // A long summary is capped to ~280 chars.
    let long = "word ".repeat(200); // 1000 chars
    let capped = build_enrichment_summary(Some(&long)).expect("capped");
    assert!(capped.chars().count() <= ENRICHMENT_SUMMARY_CHAR_CAP);
}

#[test]
fn default_enrichment_refetch_after_for_status_matches_negative_cache_cadence() {
    // Success / empty / blocked never schedule a retry.
    assert!(default_enrichment_refetch_after_for_status("success").is_none());
    assert!(default_enrichment_refetch_after_for_status("empty").is_none());
    assert!(default_enrichment_refetch_after_for_status("blocked").is_none());
    // Transient + structural failures schedule a (future) retry timestamp.
    for status in ["fetch-error", "rate-limited", "decode-error", "unsupported-content"] {
        let when = default_enrichment_refetch_after_for_status(status);
        assert!(when.is_some(), "{status} must schedule a retry");
        assert!(when.unwrap().ends_with('Z'));
    }
    // Unknown status stays dormant (no retry storm).
    assert!(default_enrichment_refetch_after_for_status("mystery").is_none());
}

#[test]
fn load_best_enrichment_map_prefers_success_then_source_priority() {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths().clone();
    let connection = TestDatabase::new(&vault, Db::Intelligence);

    let capture_blob =
        store_readable_text_blob(&paths, Some("capture body")).expect("capture blob");
    let title_blob = store_readable_text_blob(&paths, Some("title body")).expect("title blob");
    connection
        .execute(
            "INSERT INTO page_content_enrichments
             (canonical_url, content_source, fetch_status, fetched_at, readable_title,
              readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
              extraction_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '{}', 'test-v1')",
            params![
                "https://example.com/1",
                "title-normalization",
                "success",
                "2026-04-17T10:00:00Z",
                "Title Fallback",
                title_blob.as_ref().map(|blob| blob.relative_path.as_str()),
                title_blob.as_ref().map(|blob| blob.byte_len as i64).unwrap_or(0),
                title_blob.as_ref().map(|blob| blob.content_hash.as_str()),
                "[\"title snippet\"]",
            ],
        )
        .expect("title row");
    connection
        .execute(
            "INSERT INTO page_content_enrichments
             (canonical_url, content_source, fetch_status, fetched_at, readable_title,
              readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
              extraction_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '{}', 'test-v1')",
            params![
                "https://example.com/1",
                "capture",
                "success",
                "2026-04-16T09:00:00Z",
                "Captured Title",
                capture_blob.as_ref().map(|blob| blob.relative_path.as_str()),
                capture_blob.as_ref().map(|blob| blob.byte_len as i64).unwrap_or(0),
                capture_blob.as_ref().map(|blob| blob.content_hash.as_str()),
                "[\"capture snippet\"]",
            ],
        )
        .expect("capture row");
    connection
        .execute(
            "INSERT INTO page_content_enrichments
             (canonical_url, content_source, fetch_status, fetched_at, readable_title,
              readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
              extraction_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, NULL, ?6, '{}', 'test-v1')",
            params![
                "https://example.com/2",
                "readable-content-refetch",
                "fetch-error",
                "2026-04-17T12:00:00Z",
                Option::<String>::None,
                "[\"failed snippet\"]",
            ],
        )
        .expect("failed row");
    connection
        .execute(
            "INSERT INTO page_content_enrichments
             (canonical_url, content_source, fetch_status, fetched_at, readable_title,
              readable_text_blob_path, readable_text_bytes, text_hash, snippet_json,
              extraction_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, NULL, ?6, '{}', 'test-v1')",
            params![
                "https://example.com/2",
                "title-normalization",
                "empty",
                "2026-04-17T13:00:00Z",
                Option::<String>::None,
                "[\"empty snippet\"]",
            ],
        )
        .expect("empty row");

    seed_native_visits(&vault);
    let loaded = connection
        .read(move |session| load_best_enrichment_map_by_history_ids(&paths, session, &[1, 2]))
        .unwrap();
    assert_eq!(
        loaded.get(&1),
        Some(&StoredEnrichment {
            fetch_status: "success".to_string(),
            fetched_at: "2026-04-16T09:00:00Z".to_string(),
            readable_title: Some("Captured Title".to_string()),
            readable_text: Some("capture body".to_string()),
            snippet_json: "[\"capture snippet\"]".to_string(),
            enrichment_summary: None,
            extractor_version: None,
        })
    );
    assert_eq!(
        loaded.get(&2),
        Some(&StoredEnrichment {
            fetch_status: "empty".to_string(),
            fetched_at: "2026-04-17T13:00:00Z".to_string(),
            readable_title: None,
            readable_text: None,
            snippet_json: "[\"empty snippet\"]".to_string(),
            enrichment_summary: None,
            extractor_version: None,
        })
    );
}

#[test]
fn load_best_enrichment_map_skips_empty_requests() {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths().clone();
    let connection = TestDatabase::new(&vault, Db::Intelligence);

    let loaded = connection
        .read(move |session| load_best_enrichment_map_by_history_ids(&paths, session, &[]))
        .unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn title_normalization_failure_messages_and_storage_cover_enrichment_edges() {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths().clone();
    let connection = TestDatabase::new(&vault, Db::Intelligence);

    let fallback =
        title_normalization_enrichment("https://example.com/docs/pathkeep%20archive+replay", None);
    assert_eq!(fallback.status, "success");
    assert_eq!(fallback.readable_title.as_deref(), Some("pathkeep archive replay"));
    assert_eq!(fallback.extraction["strategy"], "url-fallback");

    let invalid_escape =
        title_normalization_enrichment("https://example.com/docs/bad%zztitle", None);
    assert_eq!(invalid_escape.readable_title.as_deref(), Some("bad%zztitle"));

    let empty = title_normalization_enrichment("notaurl", Some(" \n\t "));
    assert_eq!(empty.status, "empty");
    assert!(empty.snippets.is_empty());

    let unsupported = EnrichmentResult {
        status: "unsupported-content".to_string(),
        extraction: serde_json::json!({ "contentType": "image/png" }),
        ..EnrichmentResult::default()
    };
    assert!(enrichment_is_terminal_failure(&unsupported));
    assert!(enrichment_failure_message(&unsupported).contains("image/png"));

    let fetch_error = EnrichmentResult {
        status: "fetch-error".to_string(),
        extraction: serde_json::json!({ "error": "connection refused" }),
        ..EnrichmentResult::default()
    };
    assert!(enrichment_failure_message(&fetch_error).contains("connection refused"));

    let decode_with_context = EnrichmentResult {
        status: "decode-error".to_string(),
        extraction: serde_json::json!({
            "contentType": "application/gzip",
            "error": "invalid gzip header"
        }),
        ..EnrichmentResult::default()
    };
    assert!(enrichment_failure_message(&decode_with_context).contains("application/gzip"));
    let decode_without_context = EnrichmentResult {
        status: "decode-error".to_string(),
        extraction: serde_json::json!({}),
        ..EnrichmentResult::default()
    };
    assert_eq!(
        enrichment_failure_message(&decode_without_context),
        "Could not decode the response body."
    );
    let decode_content_only = EnrichmentResult {
        status: "decode-error".to_string(),
        extraction: serde_json::json!({ "contentType": "text/html" }),
        ..EnrichmentResult::default()
    };
    assert_eq!(
        enrichment_failure_message(&decode_content_only),
        "Could not decode the response body (text/html)."
    );
    let decode_error_only = EnrichmentResult {
        status: "decode-error".to_string(),
        extraction: serde_json::json!({ "error": "bad bytes" }),
        ..EnrichmentResult::default()
    };
    assert_eq!(
        enrichment_failure_message(&decode_error_only),
        "Could not decode the response body. bad bytes"
    );
    let unknown = EnrichmentResult {
        status: "empty".to_string(),
        extraction: serde_json::json!({ "error": "no readable text" }),
        ..EnrichmentResult::default()
    };
    assert_eq!(enrichment_failure_message(&unknown), "no readable text");
    let unknown_without_error = EnrichmentResult {
        status: "empty".to_string(),
        extraction: serde_json::json!({}),
        ..EnrichmentResult::default()
    };
    assert_eq!(
        enrichment_failure_message(&unknown_without_error),
        "Enrichment failed with status empty"
    );

    seed_native_visits(&vault);
    let cache_paths = paths.clone();
    connection
        .write(move |session| {
            store_enrichment(
                &cache_paths,
                session,
                "https://example.com/docs",
                "capture",
                &EnrichmentResult {
                    status: "success".to_string(),
                    final_url: Some("https://example.com/docs".to_string()),
                    language: Some("en".to_string()),
                    readable_title: Some("Readable Docs".to_string()),
                    readable_text: Some("Long readable body".to_string()),
                    snippets: vec!["Long readable body".to_string()],
                    extraction: serde_json::json!({ "strategy": "test" }),
                    ..EnrichmentResult::default()
                },
            )
        })
        .expect("store enrichment");

    let content = connection
        .read(move |session| {
            preferred_embedding_content(
                &paths,
                session,
                55,
                "chrome:Default",
                "https://example.com/docs",
                Some("Browser Docs"),
                "2026-04-17T00:00:00Z",
            )
        })
        .expect("preferred content");
    assert!(content.contains("Readable title: Readable Docs"));
    assert!(content.contains("Readable text:\nLong readable body"));
}

#[test]
fn html_enrichment_result_covers_fallback_adapter_and_truncation_paths() {
    let fallback = build_enrichment_result_from_html(
        "https://example.com/fallback",
        Some("https://example.com/fallback".to_string()),
        "text/html".to_string(),
        "<html><body> Body fallback only </body></html>",
    );
    assert_eq!(fallback.status, "success");
    assert_eq!(fallback.snippets, vec!["Body fallback only".to_string()]);

    let long_text = "x".repeat(12_010);
    let truncated = build_enrichment_result_from_html(
        "https://example.com/long",
        Some("https://example.com/long".to_string()),
        "text/html".to_string(),
        &format!("<html><body><main><p>{long_text}</p></main></body></html>"),
    );
    assert_eq!(truncated.readable_text.as_deref().map(str::len), Some(12_000));

    let youtube = build_enrichment_result_from_html(
        "https://www.youtube.com/watch?v=abc123",
        Some("https://www.youtube.com/watch?v=abc123".to_string()),
        "text/html".to_string(),
        r#"
        <html lang="en">
          <head>
            <title>Generic title</title>
            <script type="application/ld+json">
            {
              "@context": "https://schema.org",
              "@type": "VideoObject",
              "name": "PathKeep demo",
              "description": "A walkthrough of local history backup.",
              "author": { "name": "PathKeep" },
              "duration": "PT1H2M3S",
              "uploadDate": "2026-04-17"
            }
            </script>
          </head>
          <body><main><p>Generic body text.</p></main></body>
        </html>
        "#,
    );
    assert_eq!(youtube.status, "success");
    assert_eq!(youtube.readable_title.as_deref(), Some("PathKeep demo"));
    assert!(
        youtube.readable_text.as_deref().is_some_and(|text| text.contains("Video: PathKeep demo"))
    );
    assert_eq!(youtube.extraction["siteAdapter"]["id"], "youtube-video");
    assert_eq!(youtube.extraction["siteAdapter"]["metadata"]["videoId"], "abc123");
}

#[test]
fn site_adapter_merge_falls_back_to_generic_snippets_and_status() {
    let merged = merge_site_adapter_result(
        EnrichmentResult {
            status: "empty".to_string(),
            final_url: Some("https://video.example/watch".to_string()),
            language: Some("en".to_string()),
            readable_title: Some("Generic title".to_string()),
            readable_text: None,
            snippets: vec!["Generic snippet".to_string()],
            extraction: serde_json::json!({}),
            ..EnrichmentResult::default()
        },
        "text/html",
        SiteAdapterResult {
            adapter_id: "fixture-video",
            readable_title: None,
            readable_text: None,
            snippets: Vec::new(),
            metadata: serde_json::json!({ "fixture": true }),
        },
    );

    assert_eq!(merged.status, "empty");
    assert_eq!(merged.readable_title.as_deref(), Some("Generic title"));
    assert_eq!(merged.snippets, vec!["Generic snippet".to_string()]);
    assert_eq!(merged.extraction["siteAdapter"]["id"], "fixture-video");
}

fn seed_native_visits(vault: &crate::Vault) {
    vault.write(Db::Archive,Priority::Batch,|s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','')",())?;
        for (id,url) in [(1_i64,"https://example.com/1"),(2,"https://example.com/2"),(55,"https://example.com/docs")] {
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'example.com')",params![id,url])?;
            s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,1,0,1,'',1,'')",params![id,url])?;
            s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,1,'')",[id])?;
        }Ok(())
    }).unwrap();
}

fn enqueue(vault: &crate::Vault, id: i64, plugin_id: &str, url: &str) -> i64 {
    let (plugin_id, url) = (plugin_id.to_owned(), url.to_owned());
    vault
        .write(Db::Intelligence, Priority::Batch, move |s| {
            let plugin = built_in_enrichment_plugin(&plugin_id).unwrap();
            let payload = EnrichmentJobPayload {
                history_id: Some(id),
                profile_id: Some("chrome:Default".into()),
                url,
                title: None,
            };
            enqueue_enrichment_job(s, 41, plugin, &payload)?;
            Ok(s.query_one(
                "SELECT id FROM intelligence_jobs WHERE dedupe_key=?1",
                [format!("{}:{}", plugin_id, crate::pages::canonical_key(&payload.url))],
                |r| Ok(r.get::<i64>(0)?),
            )?)
        })
        .unwrap()
}
fn state(vault: &crate::Vault, id: i64) -> String {
    vault
        .read(Db::Intelligence, move |s| {
            Ok(s.query_one("SELECT state FROM intelligence_jobs WHERE id=?1", [id], |r| {
                Ok(r.get::<String>(0)?)
            })?)
        })
        .unwrap()
}

#[test]
fn execute_enrichment_job_by_id_runs_outside_legacy_insights_module() {
    for key in [None, Some([31; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let id = enqueue(
            &vault,
            7,
            TITLE_NORMALIZATION_PLUGIN_ID,
            "https://example.com/docs/pathkeep-archive-replay",
        );
        assert!(execute_enrichment_job_by_id(&vault, id).unwrap());
        assert_eq!(state(&vault, id), "succeeded");
        let title=vault.read(Db::Intelligence,|s|Ok(s.query_one("SELECT readable_title FROM page_content_enrichments WHERE canonical_url='https://example.com/docs/pathkeep-archive-replay' AND content_source=?1",[TITLE_NORMALIZATION_PLUGIN_ID],|r|Ok(r.get::<String>(0)?))?)).unwrap();
        assert_eq!(title, "pathkeep archive replay");
    }
}

#[test]
fn execute_enrichment_job_by_id_covers_cancellation_and_failure_edges() {
    for key in [None, Some([32; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        assert!(!execute_enrichment_job_by_id(&vault, 404).unwrap());
        // Readable-content work remains explicitly deferred without a network fetch.
        let deferred = enqueue(&vault, 104, READABLE_CONTENT_PLUGIN_ID, "not a url");
        assert!(execute_enrichment_job_by_id(&vault, deferred).unwrap());
        assert_eq!(state(&vault, deferred), "succeeded");
        // A cancellation arriving after claim commits before the result is stored.
        for (id, status) in
            [(101, "success"), (103, "empty"), (105, "deferred"), (107, "fetch-error")]
        {
            let job_id = enqueue(
                &vault,
                id,
                TITLE_NORMALIZATION_PLUGIN_ID,
                &format!("https://example.com/{id}"),
            );
            let job = vault
                .write(Db::Intelligence, Priority::Interactive, move |s| {
                    crate::intelligence_runtime::claim_enrichment_job_by_id(s, job_id)
                })
                .unwrap()
                .unwrap();
            vault
                .write(Db::Intelligence, Priority::Interactive, move |s| {
                    s.execute(
                        "UPDATE intelligence_jobs SET stop_requested=1 WHERE id=?1",
                        [job_id],
                    )?;
                    Ok(())
                })
                .unwrap();
            let paths = vault.paths().clone();
            let status = status.to_owned();
            vault
                .write(Db::Intelligence, Priority::Batch, move |s| {
                    finish_claimed_enrichment_job(
                        &paths,
                        s,
                        &job,
                        &EnrichmentResult { status, ..Default::default() },
                    )
                })
                .unwrap();
            assert_eq!(state(&vault, job_id), "cancelled");
            let rows = vault
                .read(Db::Intelligence, move |s| {
                    Ok(s.query_one(
                        "SELECT COUNT(*) FROM page_content_enrichments WHERE canonical_url=?1",
                        [format!("https://example.com/{id}")],
                        |r| Ok(r.get::<i64>(0)?),
                    )?)
                })
                .unwrap();
            assert_eq!(rows, 0);
        }
        // Unknown plugins fail unless an already-requested cancellation takes precedence.
        for stopped in [false, true] {
            let id=vault.write(Db::Intelligence,Priority::Batch,move |s| {
                let payload=serde_json::to_string(&EnrichmentJobPayload {history_id:Some(102),profile_id:None,url:"https://example.com/unknown".into(),title:None})?;
                Ok(s.query_one("INSERT INTO intelligence_jobs(job_type,plugin_id,state,priority,attempt,stop_requested,dedupe_key,payload_json,artifact_json,created_at,scheduled_at,updated_at) VALUES(?1,'unknown','running',10,1,?2,?3,?4,'{}','','','') RETURNING id",params![ENRICHMENT_JOB_TYPE,i64::from(stopped),format!("unknown:{stopped}"),payload],|r|Ok(r.get::<i64>(0)?))?)
            }).unwrap();
            vault
                .write(Db::Intelligence, Priority::Interactive, move |s| {
                    fail_unknown_enrichment_plugin(s, id, "unknown")
                })
                .unwrap();
            assert_eq!(state(&vault, id), if stopped { "cancelled" } else { "failed" });
        }
        let id = enqueue(&vault, 109, TITLE_NORMALIZATION_PLUGIN_ID, "https://example.com/stopped");
        vault
            .write(Db::Intelligence, Priority::Interactive, move |s| {
                s.execute(
                    "UPDATE intelligence_jobs SET state='running',stop_requested=1 WHERE id=?1",
                    [id],
                )?;
                assert_eq!(finish_if_enrichment_cancelled(s, id)?, Some(true));
                Ok(())
            })
            .unwrap();
        assert_eq!(state(&vault, id), "cancelled");
        // Terminal failures retain their result and fail the durable job.
        let id = enqueue(&vault, 111, READABLE_CONTENT_PLUGIN_ID, "https://example.com/failure");
        let paths = vault.paths().clone();
        vault
            .write(Db::Intelligence, Priority::Batch, move |s| {
                let job = crate::intelligence_runtime::claim_enrichment_job_by_id(s, id)?.unwrap();
                finish_claimed_enrichment_job(
                    &paths,
                    s,
                    &job,
                    &EnrichmentResult { status: "fetch-error".into(), ..Default::default() },
                )?;
                Ok(())
            })
            .unwrap();
        assert_eq!(state(&vault, id), "failed");
    }
}
