//! Core Intelligence module registry.
//!
//! The deterministic runtime used to keep its built-in module catalog as an
//! internal static table. This module promotes that catalog into an explicit
//! registry contract so rebuild staging, Settings defaults, runtime status, and
//! explainability ownership all share the same source of truth.

use crate::models::{
    ACTIVITY_MIX_MODULE_ID, ACTIVITY_MIX_MODULE_VERSION, DAILY_ROLLUPS_MODULE_ID,
    DAILY_ROLLUPS_MODULE_VERSION, DOMAIN_DEEP_DIVE_MODULE_ID, DOMAIN_DEEP_DIVE_MODULE_VERSION,
    REFIND_PAGES_MODULE_ID, REFIND_PAGES_MODULE_VERSION, SEARCH_EFFECTIVENESS_MODULE_ID,
    SEARCH_EFFECTIVENESS_MODULE_VERSION, SEARCH_TRAILS_MODULE_ID, SEARCH_TRAILS_MODULE_VERSION,
    SESSIONS_MODULE_ID, SESSIONS_MODULE_VERSION, VISIT_DERIVED_FACTS_MODULE_ID,
    VISIT_DERIVED_FACTS_MODULE_VERSION,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Rebuild stage identifiers used by the Core Intelligence queue/runtime.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum RebuildMode {
    VisitDerive,
    DailyRollup,
    StructuralRebuild,
    FullRebuild,
}

impl RebuildMode {
    /// Parses one queued job type into the corresponding rebuild mode.
    pub fn from_job_type(job_type: &str) -> Result<Self> {
        match job_type {
            "visit-derive" => Ok(Self::VisitDerive),
            "daily-rollup" => Ok(Self::DailyRollup),
            "structural-rebuild" => Ok(Self::StructuralRebuild),
            "full-rebuild" => Ok(Self::FullRebuild),
            _ => bail!("'{job_type}' is not a supported Core Intelligence job type."),
        }
    }

    /// Returns the persisted queue job type string for this rebuild mode.
    pub const fn job_type(self) -> &'static str {
        match self {
            Self::VisitDerive => "visit-derive",
            Self::DailyRollup => "daily-rollup",
            Self::StructuralRebuild => "structural-rebuild",
            Self::FullRebuild => "full-rebuild",
        }
    }

    /// Returns the user-facing runtime label for this rebuild mode.
    pub const fn label(self) -> &'static str {
        match self {
            Self::VisitDerive => "visit-derived facts refresh",
            Self::DailyRollup => "daily rollup refresh",
            Self::StructuralRebuild => "structural entity rebuild",
            Self::FullRebuild => "full Core Intelligence rebuild",
        }
    }

    /// Returns whether this rebuild mode recomputes visit-derived facts.
    pub const fn requires_visit_derived_facts(self) -> bool {
        matches!(self, Self::VisitDerive | Self::FullRebuild)
    }

    /// Returns whether this rebuild mode recomputes daily rollups.
    pub const fn requires_daily_rollups(self) -> bool {
        matches!(self, Self::DailyRollup | Self::FullRebuild)
    }

    /// Returns whether this rebuild mode recomputes structural entities.
    pub const fn requires_structural_entities(self) -> bool {
        matches!(self, Self::StructuralRebuild | Self::FullRebuild)
    }

    /// Returns the built-in module descriptors touched by this rebuild mode.
    pub fn module_descriptors(self) -> Vec<&'static IntelligenceModuleDescriptor> {
        let requested = built_in_intelligence_module_descriptors()
            .iter()
            .copied()
            .filter(|descriptor| descriptor.rebuild_modes.contains(&self))
            .map(|descriptor| descriptor.id)
            .collect::<Vec<_>>();
        resolve_intelligence_module_order(&requested)
            .expect("built-in Core Intelligence registry should have valid dependencies")
    }

    /// Returns the built-in module ids touched by this rebuild mode.
    pub fn module_ids(self) -> Vec<&'static str> {
        self.module_descriptors().into_iter().map(|descriptor| descriptor.id).collect()
    }
}

/// Minimal field-level schema metadata for one module-specific Settings surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSchemaField {
    pub key: &'static str,
    pub label: &'static str,
    pub value_type: &'static str,
    pub required: bool,
}

/// Schema descriptor for Settings-managed module knobs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SettingsSchema {
    pub fields: &'static [SettingsSchemaField],
}

/// Registry descriptor for one built-in Core Intelligence module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntelligenceModuleDescriptor {
    pub id: &'static str,
    pub version: &'static str,
    pub depends_on: &'static [&'static str],
    pub derived_tables: &'static [&'static str],
    pub rebuild_modes: &'static [RebuildMode],
    pub explanation_entity_types: &'static [&'static str],
    pub settings_schema: SettingsSchema,
}

/// Trait-backed marker for one built-in Core Intelligence module.
pub trait IntelligenceModule: Sync {
    fn descriptor(&self) -> &'static IntelligenceModuleDescriptor;
}

macro_rules! define_builtin_module {
    (
        $type_name:ident,
        $instance_name:ident,
        $descriptor_name:ident,
        $id:expr,
        $version:expr,
        $depends_on:expr,
        $derived_tables:expr,
        $rebuild_modes:expr,
        $explanation_entity_types:expr
    ) => {
        #[derive(Debug)]
        struct $type_name;

        impl IntelligenceModule for $type_name {
            fn descriptor(&self) -> &'static IntelligenceModuleDescriptor {
                &$descriptor_name
            }
        }

        static $descriptor_name: IntelligenceModuleDescriptor = IntelligenceModuleDescriptor {
            id: $id,
            version: $version,
            depends_on: $depends_on,
            derived_tables: $derived_tables,
            rebuild_modes: $rebuild_modes,
            explanation_entity_types: $explanation_entity_types,
            settings_schema: SettingsSchema { fields: &[] },
        };

        static $instance_name: $type_name = $type_name;
    };
}

define_builtin_module!(
    VisitDerivedFactsModule,
    VISIT_DERIVED_FACTS_MODULE,
    VISIT_DERIVED_FACTS_DESCRIPTOR,
    VISIT_DERIVED_FACTS_MODULE_ID,
    VISIT_DERIVED_FACTS_MODULE_VERSION,
    &[],
    &["visit_derived_facts"],
    &[RebuildMode::VisitDerive, RebuildMode::FullRebuild],
    &[]
);

define_builtin_module!(
    DailyRollupsModule,
    DAILY_ROLLUPS_MODULE,
    DAILY_ROLLUPS_DESCRIPTOR,
    DAILY_ROLLUPS_MODULE_ID,
    DAILY_ROLLUPS_MODULE_VERSION,
    &[VISIT_DERIVED_FACTS_MODULE_ID],
    &[
        "domain_daily_rollups",
        "category_daily_rollups",
        "engine_daily_rollups",
        "daily_summary_rollups",
    ],
    &[RebuildMode::DailyRollup, RebuildMode::FullRebuild],
    &[]
);

define_builtin_module!(
    SessionsModule,
    SESSIONS_MODULE,
    SESSIONS_DESCRIPTOR,
    SESSIONS_MODULE_ID,
    SESSIONS_MODULE_VERSION,
    &[VISIT_DERIVED_FACTS_MODULE_ID],
    &["sessions"],
    &[RebuildMode::StructuralRebuild, RebuildMode::FullRebuild],
    &["session"]
);

define_builtin_module!(
    SearchTrailsModule,
    SEARCH_TRAILS_MODULE,
    SEARCH_TRAILS_DESCRIPTOR,
    SEARCH_TRAILS_MODULE_ID,
    SEARCH_TRAILS_MODULE_VERSION,
    &[VISIT_DERIVED_FACTS_MODULE_ID, SESSIONS_MODULE_ID],
    &[
        "search_trails",
        "search_trail_members",
        "search_events",
        "search_event_terms",
        "query_families",
    ],
    &[RebuildMode::StructuralRebuild, RebuildMode::FullRebuild],
    &["search_trail", "query_family", "compare_set"]
);

define_builtin_module!(
    RefindPagesModule,
    REFIND_PAGES_MODULE,
    REFIND_PAGES_DESCRIPTOR,
    REFIND_PAGES_MODULE_ID,
    REFIND_PAGES_MODULE_VERSION,
    &[VISIT_DERIVED_FACTS_MODULE_ID, SEARCH_TRAILS_MODULE_ID],
    &["refind_pages", "source_effectiveness"],
    &[RebuildMode::StructuralRebuild, RebuildMode::FullRebuild],
    &["refind_page"]
);

define_builtin_module!(
    ActivityMixModule,
    ACTIVITY_MIX_MODULE,
    ACTIVITY_MIX_DESCRIPTOR,
    ACTIVITY_MIX_MODULE_ID,
    ACTIVITY_MIX_MODULE_VERSION,
    &[VISIT_DERIVED_FACTS_MODULE_ID, DAILY_ROLLUPS_MODULE_ID],
    &[],
    &[RebuildMode::DailyRollup, RebuildMode::FullRebuild],
    &[]
);

define_builtin_module!(
    SearchEffectivenessModule,
    SEARCH_EFFECTIVENESS_MODULE,
    SEARCH_EFFECTIVENESS_DESCRIPTOR,
    SEARCH_EFFECTIVENESS_MODULE_ID,
    SEARCH_EFFECTIVENESS_MODULE_VERSION,
    &[SEARCH_TRAILS_MODULE_ID, REFIND_PAGES_MODULE_ID, DAILY_ROLLUPS_MODULE_ID],
    &["reopened_investigations"],
    &[RebuildMode::StructuralRebuild, RebuildMode::FullRebuild],
    &["reopened_investigation"]
);

define_builtin_module!(
    DomainDeepDiveModule,
    DOMAIN_DEEP_DIVE_MODULE,
    DOMAIN_DEEP_DIVE_DESCRIPTOR,
    DOMAIN_DEEP_DIVE_MODULE_ID,
    DOMAIN_DEEP_DIVE_MODULE_VERSION,
    &[VISIT_DERIVED_FACTS_MODULE_ID, DAILY_ROLLUPS_MODULE_ID],
    &["habit_patterns", "path_flows"],
    &[RebuildMode::StructuralRebuild, RebuildMode::FullRebuild],
    &["habit_pattern", "path_flow"]
);

static BUILT_IN_INTELLIGENCE_MODULES: [&'static dyn IntelligenceModule; 8] = [
    &VISIT_DERIVED_FACTS_MODULE,
    &DAILY_ROLLUPS_MODULE,
    &SESSIONS_MODULE,
    &SEARCH_TRAILS_MODULE,
    &REFIND_PAGES_MODULE,
    &ACTIVITY_MIX_MODULE,
    &SEARCH_EFFECTIVENESS_MODULE,
    &DOMAIN_DEEP_DIVE_MODULE,
];

static BUILT_IN_INTELLIGENCE_MODULE_DESCRIPTORS: [&IntelligenceModuleDescriptor; 8] = [
    &VISIT_DERIVED_FACTS_DESCRIPTOR,
    &DAILY_ROLLUPS_DESCRIPTOR,
    &SESSIONS_DESCRIPTOR,
    &SEARCH_TRAILS_DESCRIPTOR,
    &REFIND_PAGES_DESCRIPTOR,
    &ACTIVITY_MIX_DESCRIPTOR,
    &SEARCH_EFFECTIVENESS_DESCRIPTOR,
    &DOMAIN_DEEP_DIVE_DESCRIPTOR,
];

/// Returns the built-in Core Intelligence module registry.
pub(crate) fn built_in_intelligence_modules() -> &'static [&'static dyn IntelligenceModule] {
    &BUILT_IN_INTELLIGENCE_MODULES
}

/// Returns the built-in module descriptor catalog.
pub(crate) fn built_in_intelligence_module_descriptors()
-> &'static [&'static IntelligenceModuleDescriptor] {
    &BUILT_IN_INTELLIGENCE_MODULE_DESCRIPTORS
}

/// Looks up one built-in Core Intelligence module descriptor by module id.
pub(crate) fn built_in_intelligence_module_descriptor(
    module_id: &str,
) -> Option<&'static IntelligenceModuleDescriptor> {
    built_in_intelligence_module_descriptors()
        .iter()
        .copied()
        .find(|descriptor| descriptor.id == module_id)
}

/// Looks up the module that owns explainability for one persisted entity type.
#[cfg(test)]
pub(crate) fn module_descriptor_for_entity_type(
    entity_type: &str,
) -> Option<&'static IntelligenceModuleDescriptor> {
    built_in_intelligence_module_descriptors()
        .iter()
        .copied()
        .find(|descriptor| descriptor.explanation_entity_types.contains(&entity_type))
}

/// Resolves built-in module dependencies while preserving registry order.
pub(crate) fn resolve_intelligence_module_order(
    module_ids: &[&str],
) -> Result<Vec<&'static IntelligenceModuleDescriptor>> {
    let requested = module_ids.iter().copied().collect::<HashSet<_>>();
    for module_id in &requested {
        if built_in_intelligence_module_descriptor(module_id).is_none() {
            bail!("unknown Core Intelligence module {module_id}");
        }
    }
    let mut ordered = Vec::<&'static IntelligenceModuleDescriptor>::new();
    let mut visiting = HashSet::<&'static str>::new();
    let mut visited = HashSet::<&'static str>::new();

    fn visit(
        module_id: &'static str,
        requested: &HashSet<&str>,
        visiting: &mut HashSet<&'static str>,
        visited: &mut HashSet<&'static str>,
        ordered: &mut Vec<&'static IntelligenceModuleDescriptor>,
    ) -> Result<()> {
        if visited.contains(module_id) || !requested.contains(module_id) {
            return Ok(());
        }
        reject_module_dependency_cycle(visiting, module_id)?;
        let descriptor = built_in_intelligence_module_descriptor(module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown Core Intelligence module {module_id}"))?;
        for dependency in descriptor.depends_on {
            visit(dependency, requested, visiting, visited, ordered)?;
        }
        visiting.remove(module_id);
        visited.insert(module_id);
        ordered.push(descriptor);
        Ok(())
    }

    for module_id in
        built_in_intelligence_module_descriptors().iter().copied().map(|descriptor| descriptor.id)
    {
        visit(module_id, &requested, &mut visiting, &mut visited, &mut ordered)?;
    }

    Ok(ordered)
}

fn reject_module_dependency_cycle(
    visiting: &mut HashSet<&'static str>,
    module_id: &'static str,
) -> Result<()> {
    if !visiting.insert(module_id) {
        bail!("Core Intelligence module dependency cycle detected at {module_id}.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        RebuildMode, built_in_intelligence_modules, module_descriptor_for_entity_type,
        reject_module_dependency_cycle, resolve_intelligence_module_order,
    };
    use crate::models::{
        DOMAIN_DEEP_DIVE_MODULE_ID, REFIND_PAGES_MODULE_ID, SEARCH_TRAILS_MODULE_ID,
        SESSIONS_MODULE_ID, VISIT_DERIVED_FACTS_MODULE_ID,
    };
    use std::collections::HashSet;

    #[test]
    fn structural_rebuild_mode_resolves_modules_in_dependency_order() {
        let descriptors = RebuildMode::StructuralRebuild.module_descriptors();
        let ids = descriptors.into_iter().map(|descriptor| descriptor.id).collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                SESSIONS_MODULE_ID,
                SEARCH_TRAILS_MODULE_ID,
                REFIND_PAGES_MODULE_ID,
                "search-effectiveness",
                DOMAIN_DEEP_DIVE_MODULE_ID,
            ]
        );
    }

    #[test]
    fn rebuild_modes_expose_stable_queue_job_types() {
        assert_eq!(RebuildMode::VisitDerive.job_type(), "visit-derive");
        assert_eq!(RebuildMode::DailyRollup.job_type(), "daily-rollup");
        assert_eq!(RebuildMode::StructuralRebuild.job_type(), "structural-rebuild");
        assert_eq!(RebuildMode::FullRebuild.job_type(), "full-rebuild");
    }

    #[test]
    fn registry_resolves_entity_owners() {
        let descriptor =
            module_descriptor_for_entity_type("path_flow").expect("path_flow descriptor");
        assert_eq!(descriptor.id, DOMAIN_DEEP_DIVE_MODULE_ID);
        let descriptor =
            module_descriptor_for_entity_type("query_family").expect("query_family descriptor");
        assert_eq!(descriptor.id, SEARCH_TRAILS_MODULE_ID);
    }

    #[test]
    fn registry_exposes_trait_backed_built_ins() {
        let ids = built_in_intelligence_modules()
            .iter()
            .map(|module| module.descriptor().id)
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 8);
        assert_eq!(ids[0], "visit-derived-facts");
    }

    #[test]
    fn resolve_intelligence_module_order_rejects_unknown_modules() {
        let error = resolve_intelligence_module_order(&["unknown-module"]).expect_err("error");
        assert!(error.to_string().contains("unknown Core Intelligence module"));
    }

    #[test]
    fn module_dependency_cycle_guard_reports_the_current_node() {
        let mut visiting = HashSet::from([VISIT_DERIVED_FACTS_MODULE_ID]);
        let error = reject_module_dependency_cycle(&mut visiting, VISIT_DERIVED_FACTS_MODULE_ID)
            .expect_err("cycle");
        assert!(error.to_string().contains(VISIT_DERIVED_FACTS_MODULE_ID));
    }
}
