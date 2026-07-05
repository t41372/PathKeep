/**
 * This test file protects the front-end helper and contract logic in Enrichment.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { describe, expect, test } from 'vitest'
import {
  READABLE_CONTENT_REFETCH_VERSION,
  READABLE_CONTENT_REFETCH_PLUGIN_ID,
  TITLE_NORMALIZATION_PLUGIN_ID,
  TITLE_NORMALIZATION_VERSION,
  defaultEnrichmentSettings,
  enrichmentPluginEnabled,
  enrichmentPluginRegistry,
  enrichmentPluginState,
  resolveEnrichmentSettings,
} from './enrichment'
import { readableContentFetchAvailable } from './release-capabilities'

describe('enrichment helpers', () => {
  test('returns the built-in plugin defaults', () => {
    expect(defaultEnrichmentSettings()).toEqual({
      plugins: [
        {
          id: TITLE_NORMALIZATION_PLUGIN_ID,
          enabled: true,
          version: TITLE_NORMALIZATION_VERSION,
        },
        {
          id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
          enabled: false,
          version: READABLE_CONTENT_REFETCH_VERSION,
        },
      ],
    })
  })

  test('keeps the network refetch plugin default OFF and decoupled from the release-availability flag (ENR-2)', () => {
    // The release flag is live in this build, yet the network-egress enrichment
    // plugin must still default OFF. Availability (may surfaces SHOW stats) is
    // not consent (is fetching ON). A fresh user keeps network enrichment off
    // regardless of this flag.
    expect(readableContentFetchAvailable).toBe(true)
    const registryEntry = enrichmentPluginRegistry.find(
      (plugin) => plugin.id === READABLE_CONTENT_REFETCH_PLUGIN_ID,
    )
    expect(registryEntry?.defaultEnabled).toBe(false)
    expect(
      enrichmentPluginEnabled(
        defaultEnrichmentSettings(),
        READABLE_CONTENT_REFETCH_PLUGIN_ID,
      ),
    ).toBe(false)
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
          id: TITLE_NORMALIZATION_PLUGIN_ID,
          enabled: true,
          version: TITLE_NORMALIZATION_VERSION,
        },
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
          version: READABLE_CONTENT_REFETCH_VERSION,
        },
      ],
    }

    expect(
      enrichmentPluginState(settings, READABLE_CONTENT_REFETCH_PLUGIN_ID),
    ).toEqual({
      id: READABLE_CONTENT_REFETCH_PLUGIN_ID,
      enabled: false,
      version: READABLE_CONTENT_REFETCH_VERSION,
    })
    expect(
      enrichmentPluginEnabled(settings, READABLE_CONTENT_REFETCH_PLUGIN_ID),
    ).toBe(false)
    expect(enrichmentPluginEnabled(undefined, 'missing-plugin')).toBe(false)
  })
})
