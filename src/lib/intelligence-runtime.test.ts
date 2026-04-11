/**
 * This test file protects the front-end helper and contract logic in Intelligence Runtime.
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
  enrichmentPluginBoundaryLabel,
  enrichmentPluginDescription,
  enrichmentPluginLabel,
  intelligenceRuntimeJobStateLabel,
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

  test('maps intelligence runtime job states to labels', () => {
    expect(intelligenceRuntimeJobStateLabel('queued', t)).toBe('Queued')
    expect(intelligenceRuntimeJobStateLabel('running', t)).toBe('Running')
    expect(intelligenceRuntimeJobStateLabel('failed', t)).toBe('Failed')
    expect(intelligenceRuntimeJobStateLabel('cancelled', t)).toBe('Cancelled')
    expect(intelligenceRuntimeJobStateLabel('unknown', t)).toBe('unknown')
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
