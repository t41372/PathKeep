/**
 * @file optional-ai-availability.test.ts
 * @description Unit coverage for the multi-condition optional-AI gate.
 * @module lib/optional-ai-availability
 *
 * ## Responsibilities
 * - Verify each gate condition is checked in priority order.
 *
 * ## Not responsible for
 * - Re-testing surfaces that consume the gate.
 *
 * ## Performance notes
 * - Pure compute, runs in milliseconds.
 */

import { describe, expect, test } from 'vitest'
import { evaluateOptionalAiAvailability } from './optional-ai-availability'

describe('evaluateOptionalAiAvailability', () => {
  test('flags release-deferred first when the release flag is off', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: false,
        embeddingProviderId: 'provider-1',
        aiStatusState: 'ready',
      }),
    ).toEqual({ available: false, reason: 'release-deferred' })
  })

  test('flags release-deferred even when no provider is selected', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: false,
        embeddingProviderId: null,
        aiStatusState: 'failed',
      }),
    ).toEqual({ available: false, reason: 'release-deferred' })
  })

  test('flags no-embedding-provider when release is enabled but provider is missing', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: true,
        embeddingProviderId: null,
        aiStatusState: 'ready',
      }),
    ).toEqual({ available: false, reason: 'no-embedding-provider' })
  })

  test('flags ai-disabled when release is enabled but AI is turned off', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: true,
        aiEnabled: false,
        embeddingProviderId: 'provider-1',
        aiStatusState: 'ready',
      }),
    ).toEqual({ available: false, reason: 'ai-disabled' })
  })

  test('flags ai-disabled when runtime status says the index is disabled', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: true,
        embeddingProviderId: 'provider-1',
        aiStatusState: 'disabled',
      }),
    ).toEqual({ available: false, reason: 'ai-disabled' })
  })

  test.each(['failed', 'blocked', 'degraded'])(
    'flags embedding-provider-error when ai status is %s',
    (state) => {
      expect(
        evaluateOptionalAiAvailability({
          releaseEnabled: true,
          embeddingProviderId: 'provider-1',
          aiStatusState: state,
        }),
      ).toEqual({ available: false, reason: 'embedding-provider-error' })
    },
  )

  test('returns available when every condition is satisfied', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: true,
        embeddingProviderId: 'provider-1',
        aiStatusState: 'ready',
      }),
    ).toEqual({ available: true, reason: null })
  })

  test('treats missing or null AI state as available', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: true,
        embeddingProviderId: 'provider-1',
        aiStatusState: null,
      }),
    ).toEqual({ available: true, reason: null })
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: true,
        embeddingProviderId: 'provider-1',
      }),
    ).toEqual({ available: true, reason: null })
  })

  test('treats unknown non-error AI states as available', () => {
    expect(
      evaluateOptionalAiAvailability({
        releaseEnabled: true,
        embeddingProviderId: 'provider-1',
        aiStatusState: 'rebuilding',
      }),
    ).toEqual({ available: true, reason: null })
  })
})
