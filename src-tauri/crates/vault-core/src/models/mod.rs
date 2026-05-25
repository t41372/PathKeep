//! Shared backend read/write models.
//!
//! These serde types are the contract between the desktop shell, worker layer,
//! and canonical backend modules. They should stay descriptive and transport
//! friendly rather than smuggling behavior.

pub mod annotations;
pub mod app;
pub mod archive;
pub mod audit;
pub mod core_intelligence;
pub mod import;
pub mod intelligence;
pub mod progress;
pub mod remote;
pub mod schedule;
pub mod security;

/// Re-exports the full backend model surface for the rest of the workspace.
pub use self::{
    annotations::*, app::*, archive::*, audit::*, core_intelligence::*, import::*, intelligence::*,
    progress::*, remote::*, schedule::*, security::*,
};

#[cfg(test)]
mod tests {
    use super::{
        ACTIVITY_MIX_MODULE_ID, AiSearchRequest, AppConfig, DAILY_ROLLUPS_MODULE_ID,
        DOMAIN_DEEP_DIVE_MODULE_ID, IntelligenceStatus, READABLE_CONTENT_PLUGIN_ID,
        READABLE_CONTENT_PLUGIN_VERSION, REFIND_PAGES_MODULE_ID, SEARCH_EFFECTIVENESS_MODULE_ID,
        SEARCH_TRAILS_MODULE_ID, SESSIONS_MODULE_ID, TITLE_NORMALIZATION_PLUGIN_ID,
        TITLE_NORMALIZATION_PLUGIN_VERSION, VISIT_DERIVED_FACTS_MODULE_ID,
        default_deterministic_module_states, default_enrichment_plugin_states,
        merge_deterministic_module_states, merge_enrichment_plugin_preferences,
        merge_enrichment_plugin_states, normalize_app_config,
    };

    #[test]
    fn ai_search_request_defaults_to_eight_results() {
        let request = AiSearchRequest::default();
        assert_eq!(request.query, "");
        assert_eq!(request.profile_id, None);
        assert_eq!(request.domain, None);
        assert_eq!(request.limit, Some(8));
    }

    #[test]
    fn intelligence_status_defaults_to_empty_state() {
        let status = IntelligenceStatus::default();
        assert!(!status.ready);
        assert_eq!(status.cards, 0);
        assert_eq!(status.query_groups, 0);
        assert_eq!(status.content_coverage, 0.0);
    }

    #[test]
    fn enrichment_preferences_merge_with_defaults() {
        let merged = merge_enrichment_plugin_preferences(&[super::EnrichmentPluginPreference {
            plugin_id: TITLE_NORMALIZATION_PLUGIN_ID.to_string(),
            enabled: false,
        }]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].plugin_id, TITLE_NORMALIZATION_PLUGIN_ID);
        assert!(!merged[0].enabled);
        assert_eq!(merged[1].plugin_id, READABLE_CONTENT_PLUGIN_ID);
        assert!(!merged[1].enabled);
    }

    #[test]
    fn ai_settings_deserialization_uses_current_enrichment_defaults() {
        let settings: super::AiSettings =
            serde_json::from_str("{}").expect("deserialize default ai settings");
        assert!(settings.enrichment_enabled);
        assert_eq!(settings.enrichment_plugins.len(), 2);
    }

    #[test]
    fn enrichment_plugin_states_merge_with_defaults() {
        let merged = merge_enrichment_plugin_states(&[
            super::EnrichmentPluginState {
                id: READABLE_CONTENT_PLUGIN_ID.to_string(),
                enabled: false,
                version: String::new(),
            },
            super::EnrichmentPluginState {
                id: "custom-plugin".to_string(),
                enabled: true,
                version: "local-v1".to_string(),
            },
        ]);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].id, TITLE_NORMALIZATION_PLUGIN_ID);
        assert!(merged[0].enabled);
        assert_eq!(merged[0].version, TITLE_NORMALIZATION_PLUGIN_VERSION);
        assert_eq!(merged[1].id, READABLE_CONTENT_PLUGIN_ID);
        assert!(!merged[1].enabled);
        assert_eq!(merged[1].version, READABLE_CONTENT_PLUGIN_VERSION);
        assert_eq!(merged[2].id, "custom-plugin");
        assert_eq!(merged[2].version, "local-v1");
    }

    #[test]
    fn enrichment_plugin_state_defaults_include_both_built_ins() {
        let defaults = default_enrichment_plugin_states();
        assert_eq!(defaults.len(), 2);
        assert_eq!(defaults[0].id, TITLE_NORMALIZATION_PLUGIN_ID);
        assert_eq!(defaults[1].id, READABLE_CONTENT_PLUGIN_ID);
    }

    #[test]
    fn deterministic_module_defaults_include_all_built_ins() {
        let defaults = default_deterministic_module_states();
        assert_eq!(defaults.len(), 8);
        assert_eq!(defaults[0].id, VISIT_DERIVED_FACTS_MODULE_ID);
        assert_eq!(defaults[1].id, DAILY_ROLLUPS_MODULE_ID);
        assert_eq!(defaults[2].id, SESSIONS_MODULE_ID);
        assert_eq!(defaults[3].id, SEARCH_TRAILS_MODULE_ID);
        assert_eq!(defaults[4].id, REFIND_PAGES_MODULE_ID);
        assert_eq!(defaults[5].id, ACTIVITY_MIX_MODULE_ID);
        assert_eq!(defaults[6].id, SEARCH_EFFECTIVENESS_MODULE_ID);
        assert_eq!(defaults[7].id, DOMAIN_DEEP_DIVE_MODULE_ID);
    }

    #[test]
    fn deterministic_module_state_merge_with_defaults() {
        let merged = merge_deterministic_module_states(&[super::DeterministicModuleState {
            id: SEARCH_TRAILS_MODULE_ID.to_string(),
            enabled: false,
            version: String::new(),
        }]);
        assert_eq!(merged.len(), 8);
        let search_trails = merged
            .iter()
            .find(|item| item.id == SEARCH_TRAILS_MODULE_ID)
            .expect("search trails module");
        assert!(!search_trails.enabled);
        assert!(!search_trails.version.is_empty());
    }

    #[test]
    fn deterministic_module_state_merge_discards_legacy_module_ids() {
        let merged = merge_deterministic_module_states(&[
            super::DeterministicModuleState {
                id: "query-groups".to_string(),
                enabled: false,
                version: "m5b-v1".to_string(),
            },
            super::DeterministicModuleState {
                id: SEARCH_TRAILS_MODULE_ID.to_string(),
                enabled: false,
                version: String::new(),
            },
        ]);

        assert_eq!(merged.len(), 8);
        assert!(merged.iter().all(|module| module.id != "query-groups"));
        let search_trails = merged
            .iter()
            .find(|module| module.id == SEARCH_TRAILS_MODULE_ID)
            .expect("search trails module");
        assert!(!search_trails.enabled);
    }

    #[test]
    fn normalize_app_config_restores_missing_runtime_defaults() {
        let mut config = AppConfig::default();
        config.enrichment.plugins.clear();
        config.ai.enrichment_plugins.clear();
        config.deterministic.modules.clear();
        config.explorer_background_prefetch_pages = 99;
        normalize_app_config(&mut config);

        assert_eq!(config.enrichment.plugins.len(), 2);
        assert_eq!(config.ai.enrichment_plugins.len(), 2);
        assert_eq!(config.deterministic.modules.len(), 8);
        assert!(config.ai.enrichment_enabled);
        assert_eq!(config.explorer_background_prefetch_pages, 10);
    }

    // ─── OgImage settings / fetch-mode coverage ──────────────────────

    #[test]
    fn og_image_fetch_mode_default_is_background() {
        // Background is the user-friendly default: it warms the cache
        // without surprising fetch bursts. The other variants are
        // explicit user opt-ins so a regression on the default would
        // silently downgrade everyone to OnDemand.
        let mode = super::OgImageFetchMode::default();
        assert_eq!(mode, super::OgImageFetchMode::Background);
    }

    #[test]
    fn og_image_fetch_mode_serializes_as_snake_case() {
        let pairs = [
            (super::OgImageFetchMode::Off, "\"off\""),
            (super::OgImageFetchMode::OnDemand, "\"on_demand\""),
            (super::OgImageFetchMode::Background, "\"background\""),
        ];
        for (mode, expected) in pairs {
            let json = serde_json::to_string(&mode).expect("serialize mode");
            assert_eq!(json, expected, "{mode:?} should serialize to {expected}");
            let round_trip: super::OgImageFetchMode =
                serde_json::from_str(&json).expect("deserialize mode");
            assert_eq!(round_trip, mode);
        }
    }

    #[test]
    fn og_image_fetch_mode_rejects_unknown_variants() {
        let bad = serde_json::from_str::<super::OgImageFetchMode>("\"hyperdrive\"");
        assert!(bad.is_err(), "unknown mode tag must fail to deserialize");
    }

    #[test]
    fn og_image_settings_default_matches_doc_contract() {
        // Defaults reach the user as the Settings UI starting state and
        // as the value the post-backup tick uses when config.json is
        // missing — they're load-bearing in two places, so pin them.
        let defaults = super::OgImageSettings::default();
        assert!(defaults.fetch_enabled);
        assert_eq!(defaults.fetch_mode, super::OgImageFetchMode::Background);
        assert_eq!(defaults.daily_refetch_budget, 50);
        assert_eq!(defaults.new_visit_prefetch_budget, 100);
        assert!(defaults.blocked_hosts.is_empty());
    }

    #[test]
    fn og_image_settings_effective_mode_truth_table() {
        // (fetch_enabled, fetch_mode) → expected effective_mode
        let cases = [
            (true, super::OgImageFetchMode::Background, super::OgImageFetchMode::Background),
            (true, super::OgImageFetchMode::OnDemand, super::OgImageFetchMode::OnDemand),
            (true, super::OgImageFetchMode::Off, super::OgImageFetchMode::Off),
            // Kill switch wins regardless of mode.
            (false, super::OgImageFetchMode::Background, super::OgImageFetchMode::Off),
            (false, super::OgImageFetchMode::OnDemand, super::OgImageFetchMode::Off),
            (false, super::OgImageFetchMode::Off, super::OgImageFetchMode::Off),
        ];
        for (fetch_enabled, fetch_mode, expected) in cases {
            let settings = super::OgImageSettings {
                fetch_enabled,
                fetch_mode,
                ..super::OgImageSettings::default()
            };
            assert_eq!(
                settings.effective_mode(),
                expected,
                "fetch_enabled={fetch_enabled} fetch_mode={fetch_mode:?} should resolve to {expected:?}",
            );
        }
    }

    #[test]
    fn og_image_settings_deserializes_empty_object_to_defaults() {
        // Older saved configs were written before fetch_mode +
        // daily_refetch_budget existed. Serde must fall through to the
        // Default impl for every missing field so the upgrade path is
        // silent — we use `#[serde(default)]` on the struct for exactly
        // this reason; the test pins that behaviour from regressing.
        let parsed: super::OgImageSettings =
            serde_json::from_str("{}").expect("deserialize empty og:image settings");
        assert_eq!(parsed, super::OgImageSettings::default());
    }

    #[test]
    fn og_image_settings_round_trips_through_json() {
        let original = super::OgImageSettings {
            fetch_enabled: false,
            fetch_mode: super::OgImageFetchMode::OnDemand,
            daily_refetch_budget: 123,
            new_visit_prefetch_budget: 456,
            blocked_hosts: vec!["a.test".to_string(), "b.test".to_string()],
            cleanup: super::OgImageCleanupMode::default(),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: super::OgImageSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn og_image_settings_partial_json_keeps_explicit_fields() {
        // Mix of explicit overrides + defaults via `#[serde(default)]`.
        let json = r#"{ "fetchEnabled": false, "newVisitPrefetchBudget": 7 }"#;
        let parsed: super::OgImageSettings =
            serde_json::from_str(json).expect("deserialize partial");
        assert!(!parsed.fetch_enabled);
        assert_eq!(parsed.new_visit_prefetch_budget, 7);
        // Unspecified fields fall back to defaults.
        assert_eq!(parsed.fetch_mode, super::OgImageFetchMode::Background);
        assert_eq!(parsed.daily_refetch_budget, 50);
    }
}
