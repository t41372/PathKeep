pub mod app;
pub mod archive;
pub mod audit;
pub mod import;
pub mod intelligence;
pub mod remote;
pub mod schedule;
pub mod security;

pub use self::{
    app::*, archive::*, audit::*, import::*, intelligence::*, remote::*, schedule::*, security::*,
};

#[cfg(test)]
mod tests {
    use super::{
        AiSearchRequest, AppConfig, InsightStatus, QUERY_GROUPS_MODULE_ID,
        READABLE_CONTENT_PLUGIN_ID, READABLE_CONTENT_PLUGIN_VERSION, REFERENCE_PAGES_MODULE_ID,
        SOURCE_EFFECTIVENESS_MODULE_ID, TEMPLATE_SUMMARIES_MODULE_ID, THREADS_MODULE_ID,
        TITLE_NORMALIZATION_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_VERSION,
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
    fn insight_status_defaults_to_empty_state() {
        let status = InsightStatus::default();
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
        assert!(merged[1].enabled);
    }

    #[test]
    fn enrichment_plugin_states_merge_with_defaults() {
        let merged = merge_enrichment_plugin_states(&[super::EnrichmentPluginState {
            id: READABLE_CONTENT_PLUGIN_ID.to_string(),
            enabled: false,
            version: String::new(),
        }]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, TITLE_NORMALIZATION_PLUGIN_ID);
        assert!(merged[0].enabled);
        assert_eq!(merged[0].version, TITLE_NORMALIZATION_PLUGIN_VERSION);
        assert_eq!(merged[1].id, READABLE_CONTENT_PLUGIN_ID);
        assert!(!merged[1].enabled);
        assert_eq!(merged[1].version, READABLE_CONTENT_PLUGIN_VERSION);
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
        assert_eq!(defaults.len(), 5);
        assert_eq!(defaults[0].id, QUERY_GROUPS_MODULE_ID);
        assert_eq!(defaults[1].id, THREADS_MODULE_ID);
        assert_eq!(defaults[2].id, REFERENCE_PAGES_MODULE_ID);
        assert_eq!(defaults[3].id, SOURCE_EFFECTIVENESS_MODULE_ID);
        assert_eq!(defaults[4].id, TEMPLATE_SUMMARIES_MODULE_ID);
    }

    #[test]
    fn deterministic_module_state_merge_with_defaults() {
        let merged = merge_deterministic_module_states(&[super::DeterministicModuleState {
            id: THREADS_MODULE_ID.to_string(),
            enabled: false,
            version: String::new(),
        }]);
        assert_eq!(merged.len(), 5);
        assert_eq!(merged[1].id, THREADS_MODULE_ID);
        assert!(!merged[1].enabled);
        assert!(!merged[1].version.is_empty());
    }

    #[test]
    fn normalize_app_config_restores_missing_runtime_defaults() {
        let mut config = AppConfig::default();
        config.enrichment.plugins.clear();
        config.ai.enrichment_plugins.clear();
        config.deterministic.modules.clear();
        normalize_app_config(&mut config);

        assert_eq!(config.enrichment.plugins.len(), 2);
        assert_eq!(config.ai.enrichment_plugins.len(), 2);
        assert_eq!(config.deterministic.modules.len(), 5);
        assert!(config.ai.enrichment_enabled);
    }
}
