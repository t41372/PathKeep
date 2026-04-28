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
  deterministicModuleDescription,
  deterministicModuleLabel,
  deterministicModuleStatusLabel,
  enrichmentPluginBoundaryLabel,
  enrichmentPluginDescription,
  enrichmentPluginLabel,
  intelligenceRuntimeJobStateLabel,
  upsertDeterministicModuleState,
  upsertEnrichmentPluginPreference,
} from './intelligence-runtime'
import { createTranslator } from './i18n'

describe('intelligence runtime helpers', () => {
  const t = createTranslator('en')

  test('maps known plugin ids to labels and descriptions', () => {
    expect(enrichmentPluginLabel('title-normalization', t)).toBe(
      'Title normalization',
    )
    expect(enrichmentPluginLabel('readable-content-refetch', t)).toBe(
      'Page content fetcher',
    )
    expect(enrichmentPluginLabel('deterministic-rebuild', t)).toBe(
      'Deterministic rebuild',
    )
    expect(enrichmentPluginDescription('title-normalization', t)).toContain(
      'Cleans up page titles locally',
    )
    expect(
      enrichmentPluginDescription('readable-content-refetch', t),
    ).toContain('Reads page text from visited pages')
  })

  test('falls back for unknown plugin ids and source kinds', () => {
    expect(enrichmentPluginLabel('custom-plugin', t)).toBe('custom-plugin')
    expect(enrichmentPluginDescription('custom-plugin', t)).toBe(
      'Check the plugin boundary before you turn it on for everyday use.',
    )
    expect(enrichmentPluginBoundaryLabel('network', t)).toBe('Network')
    expect(enrichmentPluginBoundaryLabel('local', t)).toBe('Local only')
  })

  test('maps intelligence runtime job states to labels', () => {
    expect(intelligenceRuntimeJobStateLabel('queued', t)).toBe('Queued')
    expect(intelligenceRuntimeJobStateLabel('running', t)).toBe('Running')
    expect(intelligenceRuntimeJobStateLabel('succeeded', t)).toBe('Succeeded')
    expect(intelligenceRuntimeJobStateLabel('failed', t)).toBe('Failed')
    expect(intelligenceRuntimeJobStateLabel('cancelled', t)).toBe('Cancelled')
    expect(intelligenceRuntimeJobStateLabel('unknown', t)).toBe('unknown')
  })

  test('maps deterministic module metadata and status fallbacks', () => {
    const expectedModules = [
      [
        'visit-derived-facts',
        'Visit-derived facts',
        'Normalizes visit-level evidence, site dictionary fields, and search metadata before downstream rebuild stages run.',
      ],
      [
        'daily-rollups',
        'Daily rollups',
        'Composes day-level rollups for domains, categories, engines, and digest summaries.',
      ],
      [
        'sessions',
        'Sessions',
        'Groups nearby visits into browsing sessions without guessing hidden dwell time.',
      ],
      [
        'search-trails',
        'Search trails',
        'Builds search trails, trail members, events, and query families from normalized visits.',
      ],
      [
        'refind-pages',
        'Refind pages',
        'Tracks pages and sources that repeatedly help you return to the same work.',
      ],
      [
        'activity-mix',
        'Activity mix',
        'Keeps digest metrics and period-over-period activity summaries in sync with daily rollups.',
      ],
      [
        'search-effectiveness',
        'Search effectiveness',
        'Explains which search trails reopen, converge, or lead to useful follow-up results.',
      ],
      [
        'domain-deep-dive',
        'Domain deep dive',
        'Maintains domain rhythm, habit, and path-flow surfaces for deeper deterministic review.',
      ],
    ] as const

    for (const [moduleId, label, description] of expectedModules) {
      expect(deterministicModuleLabel(moduleId, t)).toBe(label)
      expect(deterministicModuleDescription(moduleId, t)).toBe(description)
      expect(upsertDeterministicModuleState([], moduleId, true)).toEqual([
        { id: moduleId, enabled: true, version: 'ci-v1' },
      ])
    }
    expect(deterministicModuleDescription('activity-mix', t)).toContain(
      'digest metrics',
    )
    expect(deterministicModuleLabel('custom-module', t)).toBe('custom-module')
    expect(deterministicModuleDescription('custom-module', t)).toBe(
      'Check the saved module trace before you rely on this result.',
    )
    expect(deterministicModuleStatusLabel('ready', t)).toBe('Ready')
    expect(deterministicModuleStatusLabel('stale', t)).toBe('Stale')
    expect(deterministicModuleStatusLabel('disabled', t)).toBe('Disabled')
    expect(deterministicModuleStatusLabel('idle', t)).toBe('Idle')
    expect(deterministicModuleStatusLabel('custom', t)).toBe('custom')
  })

  test('upserts deterministic module state and drops stale unknown entries', () => {
    expect(
      upsertDeterministicModuleState(
        [
          { id: 'custom-module', enabled: true, version: 'legacy' },
          { id: 'daily-rollups', enabled: true, version: 'old' },
        ],
        'activity-mix',
        false,
      ),
    ).toEqual([
      { id: 'daily-rollups', enabled: true, version: 'old' },
      { id: 'activity-mix', enabled: false, version: 'ci-v1' },
    ])

    expect(
      upsertDeterministicModuleState(
        [{ id: 'activity-mix', enabled: false, version: 'old' }],
        'activity-mix',
        true,
      ),
    ).toEqual([{ id: 'activity-mix', enabled: true, version: 'ci-v1' }])

    expect(
      upsertDeterministicModuleState(
        [
          { id: 'search-trails', enabled: true, version: 'old' },
          { id: 'visit-derived-facts', enabled: false, version: 'old' },
        ],
        'sessions',
        true,
      ),
    ).toEqual([
      { id: 'visit-derived-facts', enabled: false, version: 'old' },
      { id: 'sessions', enabled: true, version: 'ci-v1' },
      { id: 'search-trails', enabled: true, version: 'old' },
    ])
    expect(
      upsertDeterministicModuleState(
        [
          { id: 'daily-rollups', enabled: true, version: 'old' },
          { id: 'sessions', enabled: true, version: 'old' },
        ],
        'sessions',
        false,
      ),
    ).toEqual([
      { id: 'daily-rollups', enabled: true, version: 'old' },
      { id: 'sessions', enabled: false, version: 'ci-v1' },
    ])
    expect(upsertDeterministicModuleState([], 'custom-module', true)).toEqual([
      { id: 'custom-module', enabled: true, version: 'ci-v1' },
    ])
  })

  test('upserts plugin preferences and keeps ordering stable', () => {
    expect(
      upsertEnrichmentPluginPreference([], 'readable-content-refetch', true),
    ).toEqual([{ pluginId: 'readable-content-refetch', enabled: true }])
    expect(
      upsertEnrichmentPluginPreference(
        [{ pluginId: 'title-normalization', enabled: true }],
        'readable-content-refetch',
        true,
      ),
    ).toEqual([
      { pluginId: 'readable-content-refetch', enabled: true },
      { pluginId: 'title-normalization', enabled: true },
    ])

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
    expect(
      upsertEnrichmentPluginPreference(
        [
          { pluginId: 'readable-content-refetch', enabled: true },
          { pluginId: 'title-normalization', enabled: false },
        ],
        'readable-content-refetch',
        false,
      ),
    ).toEqual([
      { pluginId: 'readable-content-refetch', enabled: false },
      { pluginId: 'title-normalization', enabled: false },
    ])
  })
})
