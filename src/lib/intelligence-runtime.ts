import type { TranslationKey } from './i18n'
import type { EnrichmentPluginPreference } from './types'

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
