import { describe, expect, test } from 'vitest'
import {
  BUILT_IN_ENRICHMENT_VERSION,
  READABLE_CONTENT_REFETCH_PLUGIN_ID,
  defaultEnrichmentSettings,
  enrichmentPluginEnabled,
  enrichmentPluginState,
  resolveEnrichmentSettings,
} from './enrichment'

describe('enrichment helpers', () => {
  test('returns the built-in plugin defaults', () => {
    expect(defaultEnrichmentSettings()).toEqual({
      plugins: [
        {
          id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
          enabled: true,
          version: BUILT_IN_ENRICHMENT_VERSION,
        },
      ],
    })
  })

  test('merges saved settings onto the built-in registry', () => {
    expect(
      resolveEnrichmentSettings({
        plugins: [
          {
            id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
            enabled: false,
            version: 'custom-version',
          },
          {
            id: 'custom-plugin',
            enabled: true,
            version: '0.0.1',
          },
        ],
      }),
    ).toEqual({
      plugins: [
        {
          id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
          enabled: false,
          version: 'custom-version',
        },
        {
          id: 'custom-plugin',
          enabled: true,
          version: '0.0.1',
        },
      ],
    })
  })

  test('resolves individual plugin state and enabled flags', () => {
    const settings = {
      plugins: [
        {
          id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
          enabled: false,
          version: BUILT_IN_ENRICHMENT_VERSION,
        },
      ],
    }

    expect(
      enrichmentPluginState(settings, READABLE_CONTENT_REFETCH_PLUGIN_ID),
    ).toEqual({
      id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
      enabled: false,
      version: BUILT_IN_ENRICHMENT_VERSION,
    })
    expect(
      enrichmentPluginEnabled(settings, READABLE_CONTENT_REFETCH_PLUGIN_ID),
    ).toBe(false)
    expect(enrichmentPluginEnabled(undefined, 'missing-plugin')).toBe(false)
  })
})
