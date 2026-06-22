/**
 * @file search-tuning-helpers.test.ts
 * @description Unit coverage for the pure hybrid-search tuning clamp/reset helpers.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Prove the client-side clamp mirrors the backend bounds (k ≥ 1 integer;
 *   weights [0, 100]; starred boost [0, 0.5]) and resets NaN to defaults.
 * - Prove apply/reset/diff helpers mutate only the targeted knob and never the
 *   provider lists.
 */

import { describe, expect, test } from 'vitest'
import type { AiSettings } from '../../lib/types'
import {
  SEARCH_TUNING_BOUNDS,
  SEARCH_TUNING_DEFAULTS,
  applySearchTuningKnob,
  clampSearchTuningValue,
  resetSearchTuningKnobs,
  resolveSearchTuningValue,
  searchTuningDiffersFromDefaults,
} from './search-tuning-helpers'

function settingsFixture(overrides: Partial<AiSettings> = {}): AiSettings {
  return {
    enabled: true,
    assistantEnabled: false,
    semanticIndexEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: true,
    enrichmentPlugins: [],
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt: '',
    llmProviders: [
      {
        id: 'llm-1',
        name: 'Local',
        purpose: 'llm',
        requestFormat: 'openai',
        enabled: true,
        baseUrl: 'http://localhost:1234/v1',
        apiKeySaved: false,
        defaultModel: 'm',
        modelCatalog: [],
        temperature: 0.7,
        maxTokens: 1200,
        dimensions: null,
        notes: null,
      },
    ],
    embeddingProviders: [],
    ...overrides,
  }
}

describe('clampSearchTuningValue', () => {
  test('floors hybridRrfK to an integer ≥ 1 and caps it at the slider max', () => {
    expect(clampSearchTuningValue('hybridRrfK', 0)).toBe(1)
    expect(clampSearchTuningValue('hybridRrfK', -50)).toBe(1)
    expect(clampSearchTuningValue('hybridRrfK', 60.9)).toBe(60)
    expect(clampSearchTuningValue('hybridRrfK', 9999)).toBe(
      SEARCH_TUNING_BOUNDS.hybridRrfK.max,
    )
  })

  test('clamps weights into [0, 100] without flooring fractions', () => {
    expect(clampSearchTuningValue('lexicalWeight', -1)).toBe(0)
    expect(clampSearchTuningValue('lexicalWeight', 2.5)).toBe(2.5)
    expect(clampSearchTuningValue('semanticWeight', 250)).toBe(100)
  })

  test('clamps starredBoost into [0, 0.5]', () => {
    expect(clampSearchTuningValue('starredBoost', -0.2)).toBe(0)
    expect(clampSearchTuningValue('starredBoost', 0.3)).toBe(0.3)
    expect(clampSearchTuningValue('starredBoost', 1)).toBe(0.5)
  })

  test('resets NaN (an emptied number field) to that knob default', () => {
    expect(clampSearchTuningValue('hybridRrfK', Number.NaN)).toBe(
      SEARCH_TUNING_DEFAULTS.hybridRrfK,
    )
    expect(clampSearchTuningValue('starredBoost', Number.NaN)).toBe(
      SEARCH_TUNING_DEFAULTS.starredBoost,
    )
  })
})

describe('resolveSearchTuningValue', () => {
  test('falls back to the default when the knob is absent or null', () => {
    expect(resolveSearchTuningValue(null, 'hybridRrfK')).toBe(60)
    expect(resolveSearchTuningValue(undefined, 'lexicalWeight')).toBe(1)
    expect(
      resolveSearchTuningValue(
        settingsFixture({ starredBoost: undefined }),
        'starredBoost',
      ),
    ).toBe(0.15)
    expect(
      resolveSearchTuningValue(
        settingsFixture({ semanticWeight: null as unknown as number }),
        'semanticWeight',
      ),
    ).toBe(1)
  })

  test('clamps a stored value that drifted out of range', () => {
    expect(
      resolveSearchTuningValue(
        settingsFixture({ starredBoost: 9 }),
        'starredBoost',
      ),
    ).toBe(0.5)
    expect(
      resolveSearchTuningValue(
        settingsFixture({ hybridRrfK: 0 }),
        'hybridRrfK',
      ),
    ).toBe(1)
  })
})

describe('applySearchTuningKnob', () => {
  test('sets one clamped knob and leaves the provider lists untouched', () => {
    const base = settingsFixture()
    const next = applySearchTuningKnob(base, 'lexicalWeight', 2.5)
    expect(next.lexicalWeight).toBe(2.5)
    expect(next.semanticWeight).toBe(base.semanticWeight)
    expect(next.llmProviders).toBe(base.llmProviders)
    expect(base.lexicalWeight).not.toBe(2.5)
  })

  test('clamps the applied value', () => {
    const next = applySearchTuningKnob(settingsFixture(), 'starredBoost', 5)
    expect(next.starredBoost).toBe(0.5)
  })
})

describe('resetSearchTuningKnobs', () => {
  test('restores all four knobs to defaults', () => {
    const base = settingsFixture({
      hybridRrfK: 12,
      lexicalWeight: 4,
      semanticWeight: 0,
      starredBoost: 0.4,
    })
    const next = resetSearchTuningKnobs(base)
    expect(next.hybridRrfK).toBe(60)
    expect(next.lexicalWeight).toBe(1)
    expect(next.semanticWeight).toBe(1)
    expect(next.starredBoost).toBe(0.15)
    expect(next.llmProviders).toBe(base.llmProviders)
  })
})

describe('searchTuningDiffersFromDefaults', () => {
  test('is false on defaults (or absent knobs) and true on any drift', () => {
    expect(searchTuningDiffersFromDefaults(null)).toBe(false)
    expect(searchTuningDiffersFromDefaults(settingsFixture())).toBe(false)
    expect(
      searchTuningDiffersFromDefaults(settingsFixture({ hybridRrfK: 61 })),
    ).toBe(true)
    expect(
      searchTuningDiffersFromDefaults(settingsFixture({ lexicalWeight: 2 })),
    ).toBe(true)
    expect(
      searchTuningDiffersFromDefaults(settingsFixture({ semanticWeight: 0 })),
    ).toBe(true)
    expect(
      searchTuningDiffersFromDefaults(settingsFixture({ starredBoost: 0.5 })),
    ).toBe(true)
  })
})
