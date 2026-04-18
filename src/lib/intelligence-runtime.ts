/**
 * This module turns deterministic-module and enrichment runtime data into UI-facing labels and update helpers.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `enrichmentPluginLabel`
 * - `enrichmentPluginDescription`
 * - `enrichmentPluginBoundaryLabel`
 * - `intelligenceRuntimeJobStateLabel`
 * - `deterministicModuleLabel`
 * - `deterministicModuleDescription`
 * - `deterministicModuleStatusLabel`
 * - `upsertDeterministicModuleState`
 * - `upsertEnrichmentPluginPreference`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { TranslationKey } from './i18n'
import type {
  DeterministicModuleState,
  EnrichmentPluginPreference,
} from './types'

/**
 * Defines the type-level contract for translator.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
type Translator = (
  key: TranslationKey,
  vars?: Record<string, string | number>,
) => string

const CORE_INTELLIGENCE_MODULE_ORDER = [
  'visit-derived-facts',
  'daily-rollups',
  'sessions',
  'search-trails',
  'refind-pages',
  'activity-mix',
  'search-effectiveness',
  'domain-deep-dive',
] as const

const CORE_INTELLIGENCE_MODULE_METADATA: Record<
  (typeof CORE_INTELLIGENCE_MODULE_ORDER)[number],
  {
    labelKey: TranslationKey
    descriptionKey: TranslationKey
    version: string
  }
> = {
  'visit-derived-facts': {
    labelKey: 'visitDerivedFactsModule',
    descriptionKey: 'visitDerivedFactsModuleDescription',
    version: 'ci-v1',
  },
  'daily-rollups': {
    labelKey: 'dailyRollupsModule',
    descriptionKey: 'dailyRollupsModuleDescription',
    version: 'ci-v1',
  },
  sessions: {
    labelKey: 'sessionsModule',
    descriptionKey: 'sessionsModuleDescription',
    version: 'ci-v1',
  },
  'search-trails': {
    labelKey: 'searchTrailsModule',
    descriptionKey: 'searchTrailsModuleDescription',
    version: 'ci-v1',
  },
  'refind-pages': {
    labelKey: 'refindPagesModule',
    descriptionKey: 'refindPagesModuleDescription',
    version: 'ci-v1',
  },
  'activity-mix': {
    labelKey: 'activityMixModule',
    descriptionKey: 'activityMixModuleDescription',
    version: 'ci-v1',
  },
  'search-effectiveness': {
    labelKey: 'searchEffectivenessModule',
    descriptionKey: 'searchEffectivenessModuleDescription',
    version: 'ci-v1',
  },
  'domain-deep-dive': {
    labelKey: 'domainDeepDiveModule',
    descriptionKey: 'domainDeepDiveModuleDescription',
    version: 'ci-v1',
  },
}

/**
 * Explains how enrichment plugin label works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function enrichmentPluginLabel(pluginId: string, t: Translator) {
  switch (pluginId) {
    case 'title-normalization':
      return t('titleNormalizationPlugin')
    case 'readable-content-refetch':
      return t('readableContentPlugin')
    case 'deterministic-rebuild':
      return t('deterministicRebuildJobLabel')
    default:
      return pluginId
  }
}

/**
 * Explains how enrichment plugin description works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function enrichmentPluginDescription(pluginId: string, t: Translator) {
  switch (pluginId) {
    case 'title-normalization':
      return t('titleNormalizationDescription')
    case 'readable-content-refetch':
      return t('readableContentDescription')
    default:
      return t('enrichmentPluginFallbackDescription')
  }
}

/**
 * Explains how enrichment plugin boundary label works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function enrichmentPluginBoundaryLabel(
  sourceKind: string,
  t: Translator,
) {
  return sourceKind === 'network' ? t('networkAccess') : t('localOnly')
}

/**
 * Explains how intelligence runtime job state label works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function intelligenceRuntimeJobStateLabel(state: string, t: Translator) {
  switch (state) {
    case 'queued':
      return t('runtimeStateQueued')
    case 'running':
      return t('runtimeStateRunning')
    case 'succeeded':
      return t('runtimeStateSucceeded')
    case 'failed':
      return t('runtimeStateFailed')
    case 'cancelled':
      return t('runtimeStateCancelled')
    default:
      return state
  }
}

/**
 * Explains how deterministic module label works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function deterministicModuleLabel(moduleId: string, t: Translator) {
  const metadata =
    CORE_INTELLIGENCE_MODULE_METADATA[
      moduleId as keyof typeof CORE_INTELLIGENCE_MODULE_METADATA
    ]
  return metadata ? t(metadata.labelKey) : moduleId
}

/**
 * Explains how deterministic module description works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function deterministicModuleDescription(
  moduleId: string,
  t: Translator,
) {
  const metadata =
    CORE_INTELLIGENCE_MODULE_METADATA[
      moduleId as keyof typeof CORE_INTELLIGENCE_MODULE_METADATA
    ]
  return metadata
    ? t(metadata.descriptionKey)
    : t('deterministicModuleFallbackDescription')
}

/**
 * Explains how deterministic module status label works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function deterministicModuleStatusLabel(status: string, t: Translator) {
  switch (status) {
    case 'ready':
      return t('deterministicModuleReady')
    case 'stale':
      return t('deterministicModuleStale')
    case 'disabled':
      return t('deterministicModuleDisabled')
    case 'idle':
      return t('deterministicModuleIdle')
    default:
      return status
  }
}

/**
 * Merges deterministic module state into an existing collection without losing stable identifiers.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function upsertDeterministicModuleState(
  states: DeterministicModuleState[],
  moduleId: string,
  enabled: boolean,
) {
  const canonicalIds = new Set<string>(CORE_INTELLIGENCE_MODULE_ORDER)
  const version =
    CORE_INTELLIGENCE_MODULE_METADATA[
      moduleId as keyof typeof CORE_INTELLIGENCE_MODULE_METADATA
    ]?.version ?? 'ci-v1'
  const next = states
    .filter((module) => canonicalIds.has(module.id))
    .map((module) =>
      module.id === moduleId ? { ...module, enabled, version } : module,
    )

  if (!next.some((module) => module.id === moduleId)) {
    next.push({ id: moduleId, enabled, version })
  }

  return next.sort(
    (left, right) =>
      CORE_INTELLIGENCE_MODULE_ORDER.indexOf(
        left.id as (typeof CORE_INTELLIGENCE_MODULE_ORDER)[number],
      ) -
      CORE_INTELLIGENCE_MODULE_ORDER.indexOf(
        right.id as (typeof CORE_INTELLIGENCE_MODULE_ORDER)[number],
      ),
  )
}

/**
 * Merges enrichment plugin preference into an existing collection without losing stable identifiers.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function upsertEnrichmentPluginPreference(
  preferences: EnrichmentPluginPreference[],
  pluginId: string,
  enabled: boolean,
) {
  const next = preferences.some((plugin) => plugin.pluginId === pluginId)
    ? preferences.map((plugin) =>
        plugin.pluginId === pluginId ? { ...plugin, enabled } : plugin,
      )
    : [...preferences, { pluginId, enabled }]

  return next.sort((left, right) => left.pluginId.localeCompare(right.pluginId))
}
