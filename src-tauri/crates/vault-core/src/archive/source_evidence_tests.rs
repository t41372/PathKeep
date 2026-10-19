//! Responsibilities: preserve all cold evidence families through bounded Turso writes.
//! Not responsible for: removed plaintext spool files or SQLCipher key pragmas.
//! Failure modes: omitted optional fields, evidence batches exceed 10,000 rows,
//! encrypted and plaintext stores differ, a failed chunk commits partially, the same
//! evidence read again is written again, a changed value or another profile's copy is
//! taken for the same evidence, an old-format archive keeps its copies or keys rows
//! differently from the writer.
use super::*;
use browser_history_parser::{
    ContextEvidence, EngagementEvidence, NavigationEvidence, SearchEvidence, TypedEvidenceBatch,
};
use std::collections::BTreeMap;
fn payload_with_all_evidence_families(label: &str) -> SourceEvidencePayload {
    let mut metadata = BTreeMap::new();
    metadata.insert("source".to_string(), label.to_string());
    SourceEvidencePayload {
        typed_evidence: TypedEvidenceBatch {
            search: vec![SearchEvidence {
                source_visit_id: Some(1),
                source_url_id: Some(10),
                evidence_key: "query".to_string(),
                evidence_value: format!("{label} search"),
                normalized_value: Some(label.to_string()),
                source_field: "keyword_search_terms.term".to_string(),
            }],
            navigation: vec![NavigationEvidence {
                source_visit_id: 1,
                edge_kind: "referrer".to_string(),
                target_visit_id: Some(2),
                target_url: Some(format!("https://{label}.example/next")),
                transition: Some(805306368),
                source_field: "visits.from_visit".to_string(),
            }],
            engagement: vec![EngagementEvidence {
                source_visit_id: 1,
                metric_key: "duration_ms".to_string(),
                metric_value_int: Some(42),
                metric_value_real: Some(42.5),
                source_field: "visits.visit_duration".to_string(),
            }],
            context: vec![ContextEvidence {
                source_visit_id: Some(1),
                source_url_id: Some(10),
                context_key: "page_title".to_string(),
                value_json: serde_json::json!({ "title": label }).to_string(),
                source_field: "urls.title".to_string(),
            }],
        },
        native_entities: vec![NativeEntity {
            entity_kind: "history-row".to_string(),
            native_primary_key: label.to_string(),
            parent_native_primary_key: Some("parent".to_string()),
            payload_json: serde_json::json!({ "id": label }).to_string(),
            metadata,
        }],
    }
}

#[test]
fn all_evidence_families_survive_bounded_writes_in_both_key_modes() {
    for key in [None, Some([79; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let input = SourceBatchInput {
            source_profile_id: 7,
            run_id: Some(3),
            source_kind: "fixture".into(),
            browser_version: None,
            schema_version_text: None,
            schema_version_int: None,
            schema_fingerprint: "fixture".into(),
            capability_snapshot: CapabilitySnapshot::default(),
            coverage_stats_json: "{}".into(),
            artifact_refs_json: None,
            notes_json: None,
        };
        let batch = vault
            .write(Db::SourceEvidence, Priority::Batch, move |s| upsert_source_batch(s, &input))
            .unwrap();
        let mut writer = EvidenceWriter::new(&vault, batch, 7);
        writer.push(payload_with_all_evidence_families("first")).unwrap();
        let mut large = payload_with_all_evidence_families("large");
        let entity = large.native_entities[0].clone();
        large.native_entities = (0..10_001)
            .map(|n| NativeEntity { native_primary_key: n.to_string(), ..entity.clone() })
            .collect();
        writer.push(large).unwrap();
        let probe = crate::test_support::store::Probe::for_db(&vault, Db::SourceEvidence);
        // Both payloads hold the same engagement (visit 1's 42 ms): one row.
        for (table, rows) in [
            ("visit_search_evidence", 2),
            ("visit_navigation_evidence", 2),
            ("visit_engagement_evidence", 1),
            ("visit_context_evidence", 2),
        ] {
            let count: i64 = probe
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), (), |r| Ok(r.get(0)?))
                .unwrap();
            assert_eq!(count, rows, "{table}");
        }
        let native: i64 =
            probe.query_row("SELECT COUNT(*) FROM native_entities", (), |r| Ok(r.get(0)?)).unwrap();
        assert_eq!(native, 10_002);
        let values: (Option<i64>, Option<f64>) = probe
            .query_row(
                "SELECT metric_value_int,metric_value_real FROM visit_engagement_evidence LIMIT 1",
                (),
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(values, (Some(42), Some(42.5)));
        assert!(!vault.paths().staging_dir.join("source-evidence-spool").exists());
    }
}

fn batch(vault: &crate::Vault, profile: i64) -> i64 {
    let input = SourceBatchInput {
        source_profile_id: profile,
        run_id: None,
        source_kind: "fixture".into(),
        browser_version: None,
        schema_version_text: None,
        schema_version_int: None,
        schema_fingerprint: "fixture".into(),
        capability_snapshot: CapabilitySnapshot::default(),
        coverage_stats_json: "{}".into(),
        artifact_refs_json: None,
        notes_json: None,
    };
    vault
        .write(Db::SourceEvidence, Priority::Batch, move |s| upsert_source_batch(s, &input))
        .unwrap()
}

fn rows(vault: &crate::Vault) -> Vec<(&'static str, i64)> {
    let probe = crate::test_support::store::Probe::for_db(vault, Db::SourceEvidence);
    FAMILIES
        .iter()
        .map(|family| {
            let count = probe
                .query_row(&format!("SELECT COUNT(*) FROM {}", family.table), (), |r| Ok(r.get(0)?))
                .unwrap();
            (family.table, count)
        })
        .collect()
}

fn each(count: i64) -> Vec<(&'static str, i64)> {
    FAMILIES.iter().map(|family| (family.table, count)).collect()
}

/// A file imported again, an import carried on, a URL a backup scans again: each is a new batch
/// that reads records the archive holds.
#[test]
fn evidence_read_again_by_a_later_batch_is_not_written_again() {
    let vault = crate::Vault::open_temp_with_key(Some([3; 32])).unwrap();
    let mut first = EvidenceWriter::new(&vault, batch(&vault, 7), 7);
    first.push(payload_with_all_evidence_families("page")).unwrap();
    assert_eq!(first.counts().written, 5);
    let mut again = EvidenceWriter::new(&vault, batch(&vault, 7), 7);
    again.push(payload_with_all_evidence_families("page")).unwrap();
    again.push(payload_with_all_evidence_families("page")).unwrap();
    assert_eq!(again.counts().written, 0);
    // What the source held is still counted as read.
    assert_eq!(again.counts().engagement, 2);
    assert_eq!(rows(&vault), each(1));
}

#[test]
fn a_changed_value_and_another_profiles_copy_are_evidence_of_their_own() {
    let vault = crate::Vault::open_temp_with_key(None).unwrap();
    let mut writer = EvidenceWriter::new(&vault, batch(&vault, 7), 7);
    writer.push(payload_with_all_evidence_families("page")).unwrap();
    // Chrome writes a visit's duration when its tab closes: the next backup reads a new value.
    let mut later = payload_with_all_evidence_families("page");
    later.typed_evidence.engagement[0].metric_value_int = Some(90_000);
    writer.push(later).unwrap();
    // The same records in another browser profile.
    let mut other = EvidenceWriter::new(&vault, batch(&vault, 8), 8);
    other.push(payload_with_all_evidence_families("page")).unwrap();
    assert_eq!(
        rows(&vault),
        vec![
            ("visit_search_evidence", 2),
            ("visit_navigation_evidence", 2),
            ("visit_engagement_evidence", 3),
            ("visit_context_evidence", 2),
            ("native_entities", 2),
        ]
    );
}

/// An old-format archive is copied row for row, its copies included, with no keys; the
/// conversion keys every row as the writer does and keeps the first of each.
#[test]
fn an_old_format_archive_keeps_one_row_per_evidence_keyed_like_the_writer() {
    let vault = crate::Vault::open_temp_with_key(Some([5; 32])).unwrap();
    let mut writer = EvidenceWriter::new(&vault, batch(&vault, 7), 7);
    for _ in 0..3 {
        writer.push(payload_with_all_evidence_families("page")).unwrap();
    }
    let mut changed = payload_with_all_evidence_families("page");
    changed.typed_evidence.engagement[0].metric_value_int = Some(90_000);
    writer.push(changed).unwrap();
    let probe = crate::test_support::store::Probe::for_db(&vault, Db::SourceEvidence);
    // As the copy leaves them: no keys, and every import's copy (rows 1 and 2 of each table are
    // the first two imports, written again under new ids).
    for family in &FAMILIES {
        probe.execute(&format!("UPDATE {} SET content_key=NULL", family.table), ()).unwrap();
        let columns = family.content.join(",");
        let others = if family.table == "native_entities" { ",metadata_json" } else { "" };
        for _ in 0..2 {
            probe
                .execute(
                    &format!(
                        "INSERT INTO {table}(source_batch_id,source_profile_id,{columns}{others},recorded_at)
                         SELECT source_batch_id,source_profile_id,{columns}{others},recorded_at
                         FROM {table} WHERE id=(SELECT MIN(id) FROM {table})",
                        table = family.table
                    ),
                    (),
                )
                .unwrap();
        }
    }
    assert_eq!(rows(&vault)[2], ("visit_engagement_evidence", 4));
    let cancel = vault_store::CancelToken::default();
    crate::conversion::dedupe_source_evidence(&vault, &cancel).unwrap();
    let expected = vec![
        ("visit_search_evidence", 1),
        ("visit_navigation_evidence", 1),
        ("visit_engagement_evidence", 2),
        ("visit_context_evidence", 1),
        ("native_entities", 1),
    ];
    assert_eq!(rows(&vault), expected);
    for family in &FAMILIES {
        let (first, unkeyed): (i64, i64) = probe
            .query_row(
                &format!("SELECT MIN(id), SUM(content_key IS NULL) FROM {}", family.table),
                (),
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (first, unkeyed),
            (1, 0),
            "{}: the first row stays, every row keyed",
            family.table
        );
    }
    // Run again, as a conversion carried on after a stop: nothing left to do.
    crate::conversion::dedupe_source_evidence(&vault, &cancel).unwrap();
    assert_eq!(rows(&vault), expected);
    // The next import of the same records finds them by the writer's keys.
    let mut next = EvidenceWriter::new(&vault, batch(&vault, 7), 7);
    next.push(payload_with_all_evidence_families("page")).unwrap();
    assert_eq!(next.counts().written, 0);
}
