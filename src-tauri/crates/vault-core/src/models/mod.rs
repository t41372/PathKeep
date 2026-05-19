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
    annotations::*, app::*, archive::*, audit::*, core_intelligence::*, import::*,
    intelligence::*, progress::*, remote::*, schedule::*, security::*,
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
}
