//! Responsibilities: measure related pages against real archive and sidecar files at 1M visits.
//! Not responsible for: model inference, retrieval-quality judgements or production fixtures.
//! Failure modes: a microbenchmark omits hydration or silently times an unavailable result.
//! Tests: opt-in release measurement below, sharing the semantic-stage benchmark's Vault fixture.
use super::{
    AiProviderRuntime,
    related_pages::{RelatedPages, Timings, measured},
};
use anyhow::{Context, Result, ensure};
use std::{fs::OpenOptions, io::Write};

#[test]
#[ignore = "opt-in 1M related-page measurement; use --release --ignored --nocapture"]
fn related_pages_measurement() -> Result<()> {
    if std::env::var("PATHKEEP_RELATED_BENCH").as_deref() != Ok("1") {
        eprintln!("SKIP: set PATHKEEP_RELATED_BENCH=1");
        return Ok(());
    }
    ensure!(!cfg!(debug_assertions), "measure with --release");
    let dim =
        std::env::var("PATHKEEP_RELATED_DIM").unwrap_or_else(|_| "256".into()).parse::<usize>()?;
    let out = std::env::var_os("PATHKEEP_RELATED_OUT")
        .context("set PATHKEEP_RELATED_OUT to a JSONL artifact")?;
    let provider = AiProviderRuntime {
        config: crate::AiProviderConfig {
            id: "related-bench".into(),
            name: "Related-page measurement".into(),
            purpose: crate::AiProviderPurpose::Embedding,
            enabled: true,
            base_url: Some("http://127.0.0.1:1/v1".into()),
            default_model: "bench".into(),
            dimensions: Some(dim as u32),
            ..Default::default()
        },
        api_key: None,
    };
    let mut config = crate::AppConfig::default();
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    let vault = crate::Vault::open_temp()?.with_config(config);
    let start = std::time::Instant::now();
    super::search_ranking::semantic_stage_bench::seed(&vault, 1_000_000, &provider, dim)?;
    eprintln!(
        "seeded 1M indexed visits / 500k distinct vectors, dim={dim}, {:.2}s; warm OS cache, real plaintext Vault; no embedding calls",
        start.elapsed().as_secs_f64()
    );
    let mut artifact = OpenOptions::new().create(true).append(true).open(out)?;
    for page in [1, 100, 10_000, 200_000, 499_999] {
        let url = format!("https://site{}.example/page/{page}", page % 2000);
        let mut timings = Timings::default();
        let result = measured(&vault, Some(&provider), &url, Some(page * 2), &mut timings)?;
        let RelatedPages::Ready { items, .. } = result else {
            anyhow::bail!("probe unavailable: {result:?}");
        };
        ensure!(items.len() == 6, "expected six related pages");
        let record = serde_json::json!({"visits":1_000_000,"vectors":500_000,"dim":dim,"source_page":page,"timings":timings,"items":items});
        serde_json::to_writer(&mut artifact, &record)?;
        writeln!(artifact)?;
        eprintln!("{}", serde_json::to_string(&timings)?);
    }
    Ok(())
}
