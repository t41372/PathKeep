//! Responsibilities: preserve all cold evidence families through bounded Turso writes.
//! Not responsible for: removed plaintext spool files or SQLCipher key pragmas.
//! Failure modes: omitted optional fields, evidence batches exceed 10,000 rows,
//! encrypted and plaintext stores differ, a failed chunk commits partially.
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
        large.native_entities = vec![large.native_entities[0].clone(); 10_001];
        writer.push(large).unwrap();
        let probe = crate::test_support::store::Probe::for_db(&vault, Db::SourceEvidence);
        for table in [
            "visit_search_evidence",
            "visit_navigation_evidence",
            "visit_engagement_evidence",
            "visit_context_evidence",
        ] {
            let count: i64 = probe
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), (), |r| Ok(r.get(0)?))
                .unwrap();
            assert_eq!(count, 2);
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
