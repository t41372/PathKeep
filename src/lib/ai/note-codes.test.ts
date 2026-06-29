/**
 * @file note-codes.test.ts
 * @description Verifies every backend AI note/warning CODE resolves to localized copy (review-fix M-6/M-7).
 * @module lib/ai
 *
 * The localizer is the contract that keeps raw English backend prose off the zh-CN/zh-TW surfaces.
 * These tests exhaustively round-trip EVERY variant of all three code families through a REAL
 * namespace translator in EVERY shipped locale, so a missing catalog key (which would fall back to
 * echoing the key) or an un-localized variant fails the build.
 */

import { describe, expect, it } from 'vitest'
import {
  localizeAiAgentNote,
  localizeAiIndexWarning,
  localizeAiSearchNote,
  localizeAiSearchNotes,
} from './note-codes'
import { createNamespaceTranslator } from '../i18n/catalog/catalog-runtime'
import type {
  AiAgentNote,
  AiIndexWarning,
  AiSearchNote,
} from '../types/intelligence'

const LOCALES = ['en', 'zh-CN', 'zh-TW'] as const

// The CLOSED set of every backend variant. If a new variant is added to the Rust enum + the TS
// mirror, the type checker forces it into the localizer's switch; listing every variant here proves
// each one resolves to a real, non-echoed catalog string in every locale.
const SEARCH_NOTES: AiSearchNote[] = [
  { code: 'lexicalFallbackNoProvider' },
  { code: 'emptySemanticIndex' },
  { code: 'semanticMatchesFilteredOut' },
  { code: 'configDriftDimension' },
  { code: 'configDriftFingerprint' },
  { code: 'stale', reason: 'watermark' },
  { code: 'stale', reason: 'enrichment' },
  { code: 'providerResolutionFailed', reason: 'connection refused' },
]

const INDEX_WARNINGS: AiIndexWarning[] = [
  { code: 'archiveNotInitialized' },
  { code: 'noEmbeddingProvider' },
  { code: 'embeddingProviderMissing', providerId: 'emb-9' },
  { code: 'embeddingProviderDisabled', providerName: 'My Embed' },
  { code: 'embeddingProviderNoApiKey', providerName: 'My Embed' },
  { code: 'embeddingProviderNoModel', providerName: 'My Embed' },
  { code: 'indexNotBuilt' },
  { code: 'indexStale', reason: 'watermark' },
  { code: 'indexStale', reason: 'enrichment' },
  { code: 'buildFailed', reason: 'boom from provider' },
  { code: 'indexVectorsMissing' },
]

const AGENT_NOTES: AiAgentNote[] = [
  { code: 'maxStepsReached' },
  { code: 'tokenBudgetReached' },
  { code: 'toolCallingUnavailable' },
]

/** A resolved string is "real" when it is non-empty and not an echoed (un)resolved i18n key. */
function assertLocalized(value: string, key: string) {
  expect(value.trim().length).toBeGreaterThan(0)
  // A localized hit never equals the bare label we passed in.
  expect(value).not.toBe(key)
  // On a catalog MISS the translator echoes the PREFIXED key (e.g.
  // "explorer.aiSearchNoteLexicalFallbackNoProvider") — non-empty and != the bare label, so the
  // checks above would not catch it. Assert the value does not look like an unresolved namespace
  // key so a missing/typo'd catalog entry is genuinely caught.
  expect(value).not.toMatch(/^(explorer|settings|assistant)\./)
}

describe('localizeAiSearchNote — every code, every locale', () => {
  for (const locale of LOCALES) {
    const t = createNamespaceTranslator(locale, 'explorer')
    for (const note of SEARCH_NOTES) {
      const label = note.code === 'stale' ? `stale:${note.reason}` : note.code
      it(`resolves ${label} in ${locale}`, () => {
        assertLocalized(localizeAiSearchNote(note, t), label)
      })
    }
  }

  it('weaves the opaque reason into the provider-resolution-failed note', () => {
    const t = createNamespaceTranslator('en', 'explorer')
    expect(
      localizeAiSearchNote(
        { code: 'providerResolutionFailed', reason: 'connection refused' },
        t,
      ),
    ).toContain('connection refused')
  })

  it('localizeAiSearchNotes maps a list in order and tolerates undefined', () => {
    const t = createNamespaceTranslator('en', 'explorer')
    expect(localizeAiSearchNotes(undefined, t)).toEqual([])
    const lines = localizeAiSearchNotes(
      [{ code: 'lexicalFallbackNoProvider' }, { code: 'emptySemanticIndex' }],
      t,
    )
    expect(lines).toHaveLength(2)
    expect(lines[0]).toBe(
      localizeAiSearchNote({ code: 'lexicalFallbackNoProvider' }, t),
    )
    expect(lines[1]).toBe(
      localizeAiSearchNote({ code: 'emptySemanticIndex' }, t),
    )
  })
})

describe('localizeAiIndexWarning — every code, every locale', () => {
  for (const locale of LOCALES) {
    const t = createNamespaceTranslator(locale, 'settings')
    for (const warning of INDEX_WARNINGS) {
      const label =
        warning.code === 'indexStale'
          ? `indexStale:${warning.reason}`
          : warning.code
      it(`resolves ${label} in ${locale}`, () => {
        assertLocalized(localizeAiIndexWarning(warning, t), label)
      })
    }
  }

  it('weaves structural interpolation params (provider id / name / failure reason)', () => {
    const t = createNamespaceTranslator('en', 'settings')
    expect(
      localizeAiIndexWarning(
        { code: 'embeddingProviderMissing', providerId: 'emb-9' },
        t,
      ),
    ).toContain('emb-9')
    expect(
      localizeAiIndexWarning(
        { code: 'embeddingProviderDisabled', providerName: 'My Embed' },
        t,
      ),
    ).toContain('My Embed')
    expect(
      localizeAiIndexWarning(
        { code: 'embeddingProviderNoApiKey', providerName: 'My Embed' },
        t,
      ),
    ).toContain('My Embed')
    expect(
      localizeAiIndexWarning(
        { code: 'embeddingProviderNoModel', providerName: 'My Embed' },
        t,
      ),
    ).toContain('My Embed')
    expect(
      localizeAiIndexWarning(
        { code: 'buildFailed', reason: 'boom from provider' },
        t,
      ),
    ).toContain('boom from provider')
  })
})

describe('localizeAiAgentNote — every code, every locale', () => {
  for (const locale of LOCALES) {
    const t = createNamespaceTranslator(locale, 'assistant')
    for (const note of AGENT_NOTES) {
      it(`resolves ${note.code} in ${locale}`, () => {
        assertLocalized(localizeAiAgentNote(note, t), note.code)
      })
    }
  }
})
