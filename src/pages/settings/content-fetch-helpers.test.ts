/**
 * Unit tests for the content-fetch consent helpers (W-ENRICH-1).
 *
 * These pure rules decide what egress the user has consented to — the master
 * switch, per-extractor toggles, and the per-domain blocklist round-trip — so
 * they get direct coverage of every branch, including the "default to enabled
 * when no stored preference" rule that mirrors the backend.
 */

import { describe, expect, test } from 'vitest'
import type { ContentFetchSettings } from '@/lib/types'
import {
  CONTENT_FETCH_EXTRACTOR_GENERIC_READABLE,
  CONTENT_FETCH_EXTRACTOR_GITHUB_REPO,
  applyContentFetchExtractorToggle,
  applyContentFetchMasterToggle,
  buildContentFetchDomainRules,
  domainRulesToText,
  extractorEnabled,
} from './content-fetch-helpers'

function settings(
  overrides: Partial<ContentFetchSettings> = {},
): ContentFetchSettings {
  return {
    enabled: false,
    extractors: [],
    domains: [],
    queuedJobs: 0,
    runningJobs: 0,
    failedJobs: 0,
    storedRecords: 0,
    ...overrides,
  }
}

describe('extractorEnabled', () => {
  test('returns false when settings have not loaded', () => {
    expect(extractorEnabled(null, CONTENT_FETCH_EXTRACTOR_GITHUB_REPO)).toBe(
      false,
    )
  })

  test('defaults to enabled when there is no stored preference', () => {
    expect(
      extractorEnabled(settings(), CONTENT_FETCH_EXTRACTOR_GITHUB_REPO),
    ).toBe(true)
  })

  test('honours an explicit stored preference', () => {
    const loaded = settings({
      extractors: [
        { extractorId: CONTENT_FETCH_EXTRACTOR_GITHUB_REPO, enabled: false },
      ],
    })
    expect(extractorEnabled(loaded, CONTENT_FETCH_EXTRACTOR_GITHUB_REPO)).toBe(
      false,
    )
    expect(
      extractorEnabled(loaded, CONTENT_FETCH_EXTRACTOR_GENERIC_READABLE),
    ).toBe(true)
  })
})

describe('applyContentFetchMasterToggle', () => {
  test('flips only the master switch and preserves preferences', () => {
    const loaded = settings({
      extractors: [{ extractorId: 'github-repo', enabled: false }],
      domains: [{ domain: 'x.test', allowed: false }],
    })
    const next = applyContentFetchMasterToggle(loaded, true)
    expect(next.enabled).toBe(true)
    expect(next.extractors).toEqual(loaded.extractors)
    expect(next.domains).toEqual(loaded.domains)
    // Original is not mutated.
    expect(loaded.enabled).toBe(false)
  })
})

describe('applyContentFetchExtractorToggle', () => {
  test('updates an existing extractor preference in place', () => {
    const loaded = settings({
      extractors: [
        { extractorId: 'github-repo', enabled: true },
        { extractorId: 'generic-readable', enabled: true },
      ],
    })
    const next = applyContentFetchExtractorToggle(loaded, 'github-repo', false)
    expect(next.extractors).toEqual([
      { extractorId: 'github-repo', enabled: false },
      { extractorId: 'generic-readable', enabled: true },
    ])
  })

  test('appends a preference when the extractor was absent', () => {
    const next = applyContentFetchExtractorToggle(
      settings(),
      'github-repo',
      false,
    )
    expect(next.extractors).toEqual([
      { extractorId: 'github-repo', enabled: false },
    ])
  })
})

describe('domain blocklist round-trip', () => {
  test('serializes only blocked rules, one host per line', () => {
    const text = domainRulesToText([
      { domain: 'blocked-a.test', allowed: false },
      { domain: 'allowed.test', allowed: true },
      { domain: 'blocked-b.test', allowed: false },
    ])
    expect(text).toBe('blocked-a.test\nblocked-b.test')
  })

  test('parses, trims, lowercases, and de-dupes hosts into block rules', () => {
    const rules = buildContentFetchDomainRules(
      '  Blocked.TEST \n\n blocked.test \nOther.test\n   ',
    )
    expect(rules).toEqual([
      { domain: 'blocked.test', allowed: false },
      { domain: 'other.test', allowed: false },
    ])
  })

  test('an empty textarea parses to no rules', () => {
    expect(buildContentFetchDomainRules('   \n  \n')).toEqual([])
  })
})
