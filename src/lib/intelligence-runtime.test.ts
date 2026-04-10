import { describe, expect, test } from 'vitest'
import {
  enrichmentPluginBoundaryLabel,
  enrichmentPluginDescription,
  enrichmentPluginLabel,
  upsertEnrichmentPluginPreference,
} from './intelligence-runtime'
import { createTranslator } from './i18n'

describe('intelligence runtime helpers', () => {
  const t = createTranslator('en')

  test('maps known plugin ids to labels and descriptions', () => {
    expect(enrichmentPluginLabel('title-normalization', t)).toBe(
      'Title normalization',
    )
    expect(
      enrichmentPluginDescription('readable-content-refetch', t),
    ).toContain('Fetches readable page content')
  })

  test('falls back for unknown plugin ids and source kinds', () => {
    expect(enrichmentPluginLabel('custom-plugin', t)).toBe('custom-plugin')
    expect(enrichmentPluginDescription('custom-plugin', t)).toBe(
      'Review the plugin boundary before enabling it for routine runs.',
    )
    expect(enrichmentPluginBoundaryLabel('network', t)).toBe('Network')
    expect(enrichmentPluginBoundaryLabel('local', t)).toBe('Local only')
  })

  test('upserts plugin preferences and keeps ordering stable', () => {
    expect(
      upsertEnrichmentPluginPreference([], 'readable-content-refetch', true),
    ).toEqual([{ pluginId: 'readable-content-refetch', enabled: true }])

    expect(
      upsertEnrichmentPluginPreference(
        [
          { pluginId: 'title-normalization', enabled: true },
          { pluginId: 'readable-content-refetch', enabled: true },
        ],
        'title-normalization',
        false,
      ),
    ).toEqual([
      { pluginId: 'readable-content-refetch', enabled: true },
      { pluginId: 'title-normalization', enabled: false },
    ])
  })
})
