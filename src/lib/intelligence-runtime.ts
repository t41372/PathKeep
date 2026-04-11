import type { TranslationKey } from './i18n'
import type {
  DeterministicModuleState,
  EnrichmentPluginPreference,
} from './types'

type Translator = (
  key: TranslationKey,
  vars?: Record<string, string | number>,
) => string

export function enrichmentPluginLabel(pluginId: string, t: Translator) {
  switch (pluginId) {
    case 'title-normalization':
      return t('titleNormalizationPlugin')
    case 'readable-content-refetch':
      return t('readableContentPlugin')
    default:
      return pluginId
  }
}

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

export function enrichmentPluginBoundaryLabel(
  sourceKind: string,
  t: Translator,
) {
  return sourceKind === 'network' ? t('networkAccess') : t('localOnly')
}

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

export function deterministicModuleLabel(moduleId: string, t: Translator) {
  switch (moduleId) {
    case 'query-groups':
      return t('queryGroupsModule')
    case 'threads':
      return t('threadsModule')
    case 'reference-pages':
      return t('referencePagesModule')
    case 'source-effectiveness':
      return t('sourceEffectivenessModule')
    case 'template-summaries':
      return t('templateSummariesModule')
    default:
      return moduleId
  }
}

export function deterministicModuleDescription(
  moduleId: string,
  t: Translator,
) {
  switch (moduleId) {
    case 'query-groups':
      return t('queryGroupsModuleDescription')
    case 'threads':
      return t('threadsModuleDescription')
    case 'reference-pages':
      return t('referencePagesModuleDescription')
    case 'source-effectiveness':
      return t('sourceEffectivenessModuleDescription')
    case 'template-summaries':
      return t('templateSummariesModuleDescription')
    default:
      return t('deterministicModuleFallbackDescription')
  }
}

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

export function upsertDeterministicModuleState(
  states: DeterministicModuleState[],
  moduleId: string,
  enabled: boolean,
) {
  const next = states.some((module) => module.id === moduleId)
    ? states.map((module) =>
        module.id === moduleId ? { ...module, enabled } : module,
      )
    : [...states, { id: moduleId, enabled, version: 'm5b-v1' }]

  return next.sort((left, right) => left.id.localeCompare(right.id))
}

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
