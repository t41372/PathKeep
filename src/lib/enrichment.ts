import type {
  AppConfig,
  EnrichmentPluginState,
  EnrichmentSettings,
} from './types'

export const READABLE_CONTENT_REFETCH_PLUGIN_ID = 'readable-content-refetch'
export const BUILT_IN_ENRICHMENT_VERSION = 'm4-v1'

export interface EnrichmentPluginDefinition {
  id: string
  version: string
  defaultEnabled: boolean
  queue: 'insights'
  derivedTables: string[]
  freshnessDays: number
}

export const enrichmentPluginRegistry: EnrichmentPluginDefinition[] = [
  {
    id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
    version: BUILT_IN_ENRICHMENT_VERSION,
    defaultEnabled: true,
    queue: 'insights',
    derivedTables: [
      'visit_content_enrichments',
      'visit_insight_features',
      'insight_topics',
      'insight_threads',
      'insight_cards',
      'insight_runs',
    ],
    freshnessDays: 7,
  },
]

export function defaultEnrichmentSettings(): EnrichmentSettings {
  return {
    plugins: enrichmentPluginRegistry.map((plugin) => ({
      id: plugin.id,
      enabled: plugin.defaultEnabled,
      version: plugin.version,
    })),
  }
}

export function resolveEnrichmentSettings(
  settings?: EnrichmentSettings | null,
): EnrichmentSettings {
  const defaults = defaultEnrichmentSettings()
  const existing = new Map(
    (settings?.plugins ?? []).map((plugin) => [plugin.id, plugin]),
  )
  const plugins = defaults.plugins.map((plugin) => {
    const saved = existing.get(plugin.id)
    return saved ? { ...plugin, ...saved } : plugin
  })

  for (const plugin of settings?.plugins ?? []) {
    if (!plugins.some((entry) => entry.id === plugin.id)) {
      plugins.push(plugin)
    }
  }

  return { plugins }
}

export function resolveAppConfigEnrichment(
  config: Pick<AppConfig, 'enrichment'>,
): EnrichmentSettings {
  return resolveEnrichmentSettings(config.enrichment)
}

export function enrichmentPluginState(
  settings: EnrichmentSettings | null | undefined,
  pluginId: string,
): EnrichmentPluginState {
  return (
    resolveEnrichmentSettings(settings).plugins.find(
      (plugin) => plugin.id === pluginId,
    ) ?? {
      id: pluginId,
      enabled: false,
      version: BUILT_IN_ENRICHMENT_VERSION,
    }
  )
}

export function enrichmentPluginEnabled(
  settings: EnrichmentSettings | null | undefined,
  pluginId: string,
): boolean {
  return enrichmentPluginState(settings, pluginId).enabled
}
