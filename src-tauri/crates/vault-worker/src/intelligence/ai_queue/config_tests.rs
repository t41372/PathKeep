//! Responsibilities: preserve complete fallback isolation after services change while unlocked.
//! Not responsible for: provider quality. Failure case specified before the fix: the worker uses
//! fresh settings for provider selection but stale Vault settings for partial publication.
use super::*;
use std::io::{BufRead, Read, Write};
use std::net::TcpListener;
use vault_core::{AiProviderConfig, AiProviderRuntime};
use vault_store::{Db, Priority};

#[test]
fn fresh_execution_settings_protect_a_complete_fallback_from_a_failed_new_model() {
    let vault = vault_core::Vault::open_temp().unwrap();
    vault.store().write(Db::Archive,Priority::Batch,|s| {
        s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','profile','')",())?;
        for id in 1..=2_i64 {
            let url=format!("https://fixture.test/{id}");
            s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,'fixture.test')",vault_store::params![id,url.as_str()])?;
            s.execute("INSERT INTO urls(id,page_id,url,title,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(?1,?1,?2,?2,1,0,?1,'2026-10-29T00:00:00Z',?1,'2026-10-29T00:00:00Z')",vault_store::params![id,url.as_str()])?;
            s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(?1,?1,1,0,?1,'2026-10-29T00:00:00Z')",[id])?;
        }
        s.execute("UPDATE archive_totals SET value=2 WHERE name='visits'",())?;Ok(())
    }).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let service = std::thread::spawn(move || {
        // Complete model A gets two successful chunks. B gets one durable chunk, then a fatal
        // service response. Every request reaches this real local HTTP endpoint.
        for call in 0..4 {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "expected embedding request {call}"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept fixture request: {error}"),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(request["model"], if call < 2 { "before" } else { "after" });
            let (status, body) = if call == 3 {
                ("400 Bad Request", r#"{"error":{"message":"fixture rejected input"}}"#.to_owned())
            } else {
                ("200 OK", serde_json::json!({"object":"list","model":request["model"],"data":[{"object":"embedding","index":0,"embedding":[1.0,0.0,0.0]}],"usage":{"prompt_tokens":1,"total_tokens":1}}).to_string())
            };
            write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let provider = AiProviderConfig {
        id: "added-after-open".into(),
        default_model: "before".into(),
        purpose: AiProviderPurpose::Embedding,
        enabled: true,
        base_url: Some(format!("http://{address}/v1")),
        ..Default::default()
    };
    let mut config = vault.config().clone();
    config.initialized = true;
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    config.ai.embedding_provider_id = Some(provider.id.clone());
    config.ai.embedding_providers = vec![provider.clone()];
    let current = vault.with_config(config.clone());
    let request = AiIndexRequest {
        provider_id: Some(provider.id.clone()),
        limit: Some(1),
        ..Default::default()
    };
    tokio_runtime()
        .unwrap()
        .block_on(vault_core::build_ai_index(
            &current,
            &AiProviderRuntime { config: provider, api_key: None },
            &request,
        ))
        .unwrap();
    let before = vault
        .store()
        .read(Db::Intelligence, |s| {
            s.query_one("SELECT sidecar_table FROM ai_index_ledger WHERE model='before'", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })
        .unwrap();
    config.ai.embedding_providers[0].default_model = "after".into();
    let current = vault.with_config(config.clone());
    let job = ai_queue::enqueue_index_job(&current, &request, false).unwrap();
    let claimed = ai_queue::claim_ai_job_by_id(&current, job.id, 300).unwrap().unwrap();
    assert!(
        execute_claimed_index_job(&vault, vault.paths(), &config, None, claimed, &request).is_err()
    );
    service.join().unwrap();
    // Service failures can pause a resumable job. Explicit cancellation releases that ownership;
    // complete-fallback isolation must then discard B's abandoned source and provenance.
    let stopped = ai_queue::load_ai_job(&current, job.id).unwrap();
    if matches!(stopped.state.as_str(), "paused" | "queued" | "stale") {
        ai_queue::cancel_ai_job(&current, job.id).unwrap();
    }
    vault_core::collect_index_generations(&current, true).unwrap();
    vault
        .store()
        .read(Db::Intelligence, move |s| {
            assert_eq!(
                s.query_one(
                    "SELECT sidecar_table,state FROM ai_index_ledger WHERE model='before'",
                    (),
                    |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?))
                )?,
                (before, "ready".into())
            );
            assert_eq!(
                s.query_one(
                    "SELECT sidecar_table FROM ai_index_ledger WHERE model='after'",
                    (),
                    |r| Ok(r.get::<String>(0)?)
                )?,
                ""
            );
            assert_eq!(
                s.query_one("SELECT COUNT(*) FROM ai_embeddings WHERE model='after'", (), |r| Ok(
                    r.get::<i64>(0)?
                ))?,
                0,
                "failed replacement rows must be reclaimed beside a complete fallback"
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(
        vault_core::semantic_index::load_state(&current).unwrap().serving.unwrap().model,
        "before"
    );
}
