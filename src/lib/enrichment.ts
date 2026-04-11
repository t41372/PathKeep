/**
 * This module describes the built-in enrichment plugin registry and default front-end-facing settings.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `TITLE_NORMALIZATION_PLUGIN_ID`
 * - `READABLE_CONTENT_REFETCH_PLUGIN_ID`
 * - `TITLE_NORMALIZATION_VERSION`
 * - `READABLE_CONTENT_REFETCH_VERSION`
 * - `EnrichmentPluginDefinition`
 * - `enrichmentPluginRegistry`
 * - `defaultEnrichmentSettings`
 * - `resolveEnrichmentSettings`
 * - `enrichmentPluginState`
 * - `enrichmentPluginEnabled`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { EnrichmentPluginState, EnrichmentSettings } from './types'

export const TITLE_NORMALIZATION_PLUGIN_ID = 'title-normalization'
export const READABLE_CONTENT_REFETCH_PLUGIN_ID = 'readable-content-refetch'
export const TITLE_NORMALIZATION_VERSION = 'm5-v1'
export const READABLE_CONTENT_REFETCH_VERSION = 'm4-v1'

/**
 * Defines the typed shape for enrichment plugin definition.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export interface EnrichmentPluginDefinition {
  id: string
  version: string
  defaultEnabled: boolean
  queue: 'intelligence-runtime'
  derivedTables: string[]
  freshnessDays: number | null
}

/**
 * Collects the registry entries for enrichment plugin.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export const enrichmentPluginRegistry: EnrichmentPluginDefinition[] = [
  {
    id: TITLE_NORMALIZATION_PLUGIN_ID,
    version: TITLE_NORMALIZATION_VERSION,
    defaultEnabled: true,
    queue: 'intelligence-runtime',
    derivedTables: ['visit_content_enrichments', 'visit_insight_features'],
    freshnessDays: null,
  },
  {
    id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
    version: READABLE_CONTENT_REFETCH_VERSION,
    defaultEnabled: true,
    queue: 'intelligence-runtime',
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

/**
 * Returns the default enrichment settings.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function defaultEnrichmentSettings(): EnrichmentSettings {
  return {
    plugins: enrichmentPluginRegistry.map((plugin) => ({
      id: plugin.id,
      enabled: plugin.defaultEnabled,
      version: plugin.version,
    })),
  }
}

/**
 * Resolves enrichment settings from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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

/**
 * Explains how enrichment plugin state works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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
      version: 'unknown',
    }
  )
}

/**
 * Explains how enrichment plugin enabled works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function enrichmentPluginEnabled(
  settings: EnrichmentSettings | null | undefined,
  pluginId: string,
): boolean {
  return enrichmentPluginState(settings, pluginId).enabled
}
