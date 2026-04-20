//! Core Intelligence section metadata registry.
//!
//! The `/intelligence` route now shows one shared evidence/freshness surface
//! for each section. This module owns the backend mapping from route section id
//! to deterministic runtime ownership, source-table provenance, and
//! degraded-state rules so the frontend does not need to guess.

use crate::{
    ProjectPaths,
    intelligence_runtime::load_intelligence_runtime,
    models::{
        AppConfig, CoreIntelligenceSectionMeta, CoreIntelligenceSectionWindow,
        IntelligenceRuntimeSnapshot,
    },
    utils::now_rfc3339,
};
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionDataKind {
    PersistedDerived,
    DirectRead,
    CapabilityGated,
}

#[derive(Debug, Clone, Copy)]
struct CoreIntelligenceSectionDescriptor {
    id: &'static str,
    module_ids: &'static [&'static str],
    source_tables: &'static [&'static str],
    includes_enrichment: bool,
    data_kind: SectionDataKind,
    notes: &'static [&'static str],
    empty_degraded_reason: Option<&'static str>,
}

const CORE_INTELLIGENCE_SECTION_DESCRIPTORS: [CoreIntelligenceSectionDescriptor; 22] = [
    CoreIntelligenceSectionDescriptor {
        id: "digest-summary",
        module_ids: &["daily-rollups"],
        source_tables: &["daily_summary_rollups"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "on-this-day",
        module_ids: &[],
        source_tables: &["archive.visits", "archive.urls", "archive.source_profiles"],
        includes_enrichment: false,
        data_kind: SectionDataKind::DirectRead,
        notes: &["Only prior years for the current local calendar day are considered."],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "top-sites",
        module_ids: &["daily-rollups"],
        source_tables: &["domain_daily_rollups"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "search-activity",
        module_ids: &["daily-rollups", "search-trails"],
        source_tables: &[
            "engine_daily_rollups",
            "search_events",
            "search_event_terms",
            "query_families",
        ],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "refind-pages",
        module_ids: &["refind-pages"],
        source_tables: &["refind_pages"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "activity-mix",
        module_ids: &["activity-mix", "daily-rollups"],
        source_tables: &["category_daily_rollups"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "browsing-rhythm",
        module_ids: &["daily-rollups"],
        source_tables: &["daily_summary_rollups", "category_daily_rollups"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "stable-sources",
        module_ids: &["refind-pages"],
        source_tables: &["source_effectiveness"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "search-effectiveness",
        module_ids: &["search-trails", "refind-pages", "search-effectiveness"],
        source_tables: &[
            "search_events",
            "query_families",
            "source_effectiveness",
            "reopened_investigations",
        ],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "friction-signals",
        module_ids: &["search-trails"],
        source_tables: &["search_trails", "search_trail_members"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "reopened-investigations",
        module_ids: &["search-effectiveness"],
        source_tables: &["reopened_investigations"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "discovery-trend",
        module_ids: &["daily-rollups"],
        source_tables: &["daily_summary_rollups"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "breadth-index",
        module_ids: &["daily-rollups"],
        source_tables: &["daily_summary_rollups"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "path-flows",
        module_ids: &["domain-deep-dive"],
        source_tables: &["path_flows"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "habits",
        module_ids: &["domain-deep-dive"],
        source_tables: &["habit_patterns"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "day-insights",
        module_ids: &["daily-rollups", "search-trails", "refind-pages", "activity-mix"],
        source_tables: &[
            "daily_summary_rollups",
            "domain_daily_rollups",
            "category_daily_rollups",
            "query_families",
            "refind_pages",
        ],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[
            "Day insights reuse the existing deterministic entities for one exact local calendar day.",
        ],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "query-family-detail",
        module_ids: &["search-trails"],
        source_tables: &["query_families", "search_trails", "search_events"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[
            "Query-family detail promotes one search family into a shared route-first review surface.",
        ],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "refind-page-detail",
        module_ids: &["refind-pages", "search-trails"],
        source_tables: &["refind_pages", "visit_derived_facts", "search_trails"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[
            "Refind-page detail keeps evidence, repeat history, and related trails under one route.",
        ],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "compare-sets",
        module_ids: &["search-trails"],
        source_tables: &["search_trails", "search_events"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "multi-browser-diff",
        module_ids: &["daily-rollups", "activity-mix"],
        source_tables: &["domain_daily_rollups", "category_daily_rollups"],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
    CoreIntelligenceSectionDescriptor {
        id: "observed-interactions",
        module_ids: &[],
        source_tables: &[
            "visit_engagement_evidence",
            "archive.visits",
            "archive.urls",
            "archive.source_profiles",
        ],
        includes_enrichment: false,
        data_kind: SectionDataKind::CapabilityGated,
        notes: &["Capability-gated: unsupported browsers legitimately produce no rows here."],
        empty_degraded_reason: Some(
            "No supported browser-reported interaction evidence is available for this scope yet.",
        ),
    },
    CoreIntelligenceSectionDescriptor {
        id: "domain-deep-dive",
        module_ids: &["daily-rollups", "search-trails", "domain-deep-dive"],
        source_tables: &[
            "visit_derived_facts",
            "domain_daily_rollups",
            "search_trails",
            "habit_patterns",
            "path_flows",
        ],
        includes_enrichment: false,
        data_kind: SectionDataKind::PersistedDerived,
        notes: &[],
        empty_degraded_reason: None,
    },
];

fn section_descriptor(section_id: &str) -> Option<&'static CoreIntelligenceSectionDescriptor> {
    CORE_INTELLIGENCE_SECTION_DESCRIPTORS.iter().find(|descriptor| descriptor.id == section_id)
}

fn state_priority(state: &str) -> u8 {
    match state {
        "disabled" => 3,
        "stale" => 2,
        "degraded" => 1,
        _ => 0,
    }
}

fn dedupe_notes(
    static_notes: &'static [&'static str],
    runtime_notes: impl IntoIterator<Item = String>,
    state_reason: Option<&str>,
) -> Vec<String> {
    let mut notes = Vec::<String>::new();
    for note in static_notes {
        if !notes.iter().any(|existing| existing == note) {
            notes.push((*note).to_string());
        }
    }
    for note in runtime_notes {
        if !notes.iter().any(|existing| existing == &note) {
            notes.push(note);
        }
    }
    if let Some(reason) = state_reason {
        if !notes.iter().any(|existing| existing == reason) {
            notes.push(reason.to_string());
        }
    }
    notes
}

fn build_module_backed_meta(
    descriptor: &CoreIntelligenceSectionDescriptor,
    runtime: &crate::models::IntelligenceRuntimeSnapshot,
    window: CoreIntelligenceSectionWindow,
) -> CoreIntelligenceSectionMeta {
    let relevant = descriptor
        .module_ids
        .iter()
        .filter_map(|module_id| {
            runtime.modules.iter().find(|module| module.module_id == *module_id)
        })
        .collect::<Vec<_>>();
    let generated_at =
        relevant.iter().filter_map(|module| module.last_built_at.as_ref()).min().cloned();

    let mut state = "ready".to_string();
    let mut state_reason = None::<String>;
    for module in &relevant {
        let (candidate_state, candidate_reason) = match module.status.as_str() {
            "disabled" => (
                "disabled",
                module
                    .stale_reason
                    .clone()
                    .or_else(|| module.notes.first().cloned())
                    .or_else(|| Some("Disabled in Settings.".to_string())),
            ),
            "stale" => {
                ("stale", module.stale_reason.clone().or_else(|| module.notes.first().cloned()))
            }
            "ready" => ("ready", None),
            _ => {
                ("degraded", module.stale_reason.clone().or_else(|| module.notes.first().cloned()))
            }
        };
        if state_priority(candidate_state) > state_priority(&state) {
            state = candidate_state.to_string();
            state_reason = candidate_reason;
        }
    }

    let notes = dedupe_notes(
        descriptor.notes,
        relevant.iter().flat_map(|module| module.notes.clone()),
        state_reason.as_deref(),
    );

    CoreIntelligenceSectionMeta {
        section_id: descriptor.id.to_string(),
        generated_at,
        window,
        module_ids: descriptor.module_ids.iter().map(|value| (*value).to_string()).collect(),
        source_tables: descriptor.source_tables.iter().map(|value| (*value).to_string()).collect(),
        includes_enrichment: descriptor.includes_enrichment,
        state,
        state_reason,
        notes,
    }
}

fn build_direct_meta(
    descriptor: &CoreIntelligenceSectionDescriptor,
    window: CoreIntelligenceSectionWindow,
    is_empty: bool,
) -> CoreIntelligenceSectionMeta {
    let state_reason = if descriptor.data_kind == SectionDataKind::CapabilityGated && is_empty {
        descriptor.empty_degraded_reason.map(str::to_string)
    } else {
        None
    };
    let notes = dedupe_notes(descriptor.notes, Vec::<String>::new(), state_reason.as_deref());

    CoreIntelligenceSectionMeta {
        section_id: descriptor.id.to_string(),
        generated_at: Some(now_rfc3339()),
        window,
        module_ids: Vec::new(),
        source_tables: descriptor.source_tables.iter().map(|value| (*value).to_string()).collect(),
        includes_enrichment: descriptor.includes_enrichment,
        state: if state_reason.is_some() { "degraded".to_string() } else { "ready".to_string() },
        state_reason,
        notes,
    }
}

/// Builds one section metadata payload using a caller-provided runtime
/// snapshot instead of reloading runtime state for every module-backed section.
pub fn build_core_intelligence_section_meta_with_runtime(
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    is_empty: bool,
    runtime: &IntelligenceRuntimeSnapshot,
) -> Result<CoreIntelligenceSectionMeta> {
    let descriptor = section_descriptor(section_id)
        .ok_or_else(|| anyhow::anyhow!("unknown Core Intelligence section '{section_id}'"))?;
    Ok(if descriptor.module_ids.is_empty() {
        build_direct_meta(descriptor, window, is_empty)
    } else {
        build_module_backed_meta(descriptor, runtime, window)
    })
}

/// Builds one `/intelligence` section metadata payload from the section
/// registry plus the latest runtime snapshot.
pub fn build_core_intelligence_section_meta(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    is_empty: bool,
) -> Result<CoreIntelligenceSectionMeta> {
    let runtime = load_intelligence_runtime(paths, config, key)?;
    build_core_intelligence_section_meta_with_runtime(section_id, window, is_empty, &runtime)
}

#[cfg(test)]
mod tests {
    use super::{
        CoreIntelligenceSectionDescriptor, CoreIntelligenceSectionWindow,
        build_core_intelligence_section_meta_with_runtime, build_direct_meta,
        build_module_backed_meta, section_descriptor,
    };
    use crate::models::{
        DateRange, DeterministicModuleRuntimeStatus, IntelligenceQueueStatus,
        IntelligenceRuntimeSnapshot,
    };

    fn runtime_with_modules(
        modules: Vec<DeterministicModuleRuntimeStatus>,
    ) -> IntelligenceRuntimeSnapshot {
        IntelligenceRuntimeSnapshot {
            queue: IntelligenceQueueStatus::default(),
            plugins: Vec::new(),
            modules,
            recent_jobs: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn persisted_descriptor(id: &str) -> &'static CoreIntelligenceSectionDescriptor {
        section_descriptor(id).expect("section descriptor")
    }

    fn sample_window() -> CoreIntelligenceSectionWindow {
        CoreIntelligenceSectionWindow::DateRange {
            date_range: DateRange {
                start: "2026-04-01".to_string(),
                end: "2026-04-30".to_string(),
            },
        }
    }

    #[test]
    fn section_registry_exposes_search_activity_descriptor() {
        let descriptor = persisted_descriptor("search-activity");
        assert_eq!(descriptor.module_ids, &["daily-rollups", "search-trails"]);
        assert!(descriptor.source_tables.contains(&"query_families"));
    }

    #[test]
    fn section_registry_exposes_day_insights_descriptor() {
        let descriptor = persisted_descriptor("day-insights");
        assert_eq!(
            descriptor.module_ids,
            &["daily-rollups", "search-trails", "refind-pages", "activity-mix"]
        );
        assert!(descriptor.source_tables.contains(&"daily_summary_rollups"));
        assert!(descriptor.source_tables.contains(&"query_families"));
    }

    #[test]
    fn module_backed_meta_prefers_stale_over_ready_and_uses_earliest_build_time() {
        let runtime = runtime_with_modules(vec![
            DeterministicModuleRuntimeStatus {
                module_id: "daily-rollups".to_string(),
                status: "ready".to_string(),
                last_built_at: Some("2026-04-18T11:00:00Z".to_string()),
                notes: vec!["Fresh rollups.".to_string()],
                ..DeterministicModuleRuntimeStatus::default()
            },
            DeterministicModuleRuntimeStatus {
                module_id: "search-trails".to_string(),
                status: "stale".to_string(),
                last_built_at: Some("2026-04-18T09:00:00Z".to_string()),
                stale_reason: Some("Visibility changed after the last rebuild.".to_string()),
                notes: vec!["Manual rebuild required.".to_string()],
                ..DeterministicModuleRuntimeStatus::default()
            },
        ]);

        let meta = build_module_backed_meta(
            persisted_descriptor("search-activity"),
            &runtime,
            sample_window(),
        );

        assert_eq!(meta.state, "stale");
        assert_eq!(meta.generated_at.as_deref(), Some("2026-04-18T09:00:00Z"));
        assert_eq!(
            meta.state_reason.as_deref(),
            Some("Visibility changed after the last rebuild.")
        );
    }

    #[test]
    fn module_backed_meta_marks_idle_modules_as_degraded() {
        let runtime = runtime_with_modules(vec![DeterministicModuleRuntimeStatus {
            module_id: "domain-deep-dive".to_string(),
            status: "idle".to_string(),
            notes: vec!["No successful deterministic rebuild has been recorded yet.".to_string()],
            ..DeterministicModuleRuntimeStatus::default()
        }]);

        let meta =
            build_module_backed_meta(persisted_descriptor("path-flows"), &runtime, sample_window());

        assert_eq!(meta.state, "degraded");
        assert_eq!(
            meta.state_reason.as_deref(),
            Some("No successful deterministic rebuild has been recorded yet.")
        );
        assert!(meta.generated_at.is_none());
    }

    #[test]
    fn direct_meta_uses_query_time_and_capability_gated_empty_reason() {
        let descriptor = persisted_descriptor("observed-interactions");
        let meta = build_direct_meta(
            descriptor,
            CoreIntelligenceSectionWindow::CalendarDayHistory {
                reference_date: "2026-04-18".to_string(),
            },
            true,
        );

        assert_eq!(meta.state, "degraded");
        assert!(meta.generated_at.is_some());
        assert_eq!(
            meta.state_reason.as_deref(),
            Some(
                "No supported browser-reported interaction evidence is available for this scope yet."
            )
        );
    }

    #[test]
    fn batch_meta_builder_reuses_the_provided_runtime_snapshot() {
        let runtime = runtime_with_modules(vec![
            DeterministicModuleRuntimeStatus {
                module_id: "daily-rollups".to_string(),
                status: "ready".to_string(),
                last_built_at: Some("2026-04-18T11:00:00Z".to_string()),
                ..DeterministicModuleRuntimeStatus::default()
            },
            DeterministicModuleRuntimeStatus {
                module_id: "search-trails".to_string(),
                status: "ready".to_string(),
                last_built_at: Some("2026-04-18T10:00:00Z".to_string()),
                ..DeterministicModuleRuntimeStatus::default()
            },
        ]);

        let meta = build_core_intelligence_section_meta_with_runtime(
            "search-activity",
            sample_window(),
            false,
            &runtime,
        )
        .expect("section meta");

        assert_eq!(meta.state, "ready");
        assert_eq!(meta.generated_at.as_deref(), Some("2026-04-18T10:00:00Z"));
        assert_eq!(meta.module_ids, vec!["daily-rollups", "search-trails"]);
    }
}
