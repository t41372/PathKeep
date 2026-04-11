/**
 * This test file protects the front-end helper and contract logic in Insight Canonical.
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
import { createNamespaceTranslator } from './i18n/catalog'
import {
  resolveInsightOnThisDay,
  resolveInsightPeriodicSummary,
  resolveInsightTopDomains,
} from './insight-canonical'
import type { InsightSnapshot } from './types'

const t = createNamespaceTranslator('en', 'insights')

const baseSnapshot: InsightSnapshot = {
  generatedAt: '2026-04-09T12:00:00.000Z',
  windowDays: 30,
  profileId: 'chrome:Default',
  status: {
    ready: true,
    lastRunAt: '2026-04-09T12:00:00.000Z',
    runs: 1,
    cards: 2,
    topics: 1,
    threads: 1,
    queryGroups: 1,
    referencePages: 1,
    contentCoverage: 0.8,
    warning: null,
  },
  cards: [
    {
      cardId: 'card-1',
      kind: 'revisit',
      title: 'Revisited topic',
      summary: 'You revisited semantic search material several times.',
      windowDays: 30,
      profileId: 'chrome:Default',
      score: 0.81,
      chromiumEnhanced: true,
      evidence: [],
    },
    {
      cardId: 'card-2',
      kind: 'focus',
      title: 'Focus cluster',
      summary: 'SQLite and search indexing dominated this window.',
      windowDays: 30,
      profileId: 'chrome:Default',
      score: 0.77,
      chromiumEnhanced: false,
      evidence: [],
    },
  ],
  templateSummaries: [
    {
      summaryId: 'summary-1',
      kind: 'query-groups',
      title: 'Recent query refinement',
      body: 'Template summaries should be preferred before card summaries.',
      confidence: 0.81,
      profileId: 'chrome:Default',
      evidence: [],
    },
  ],
  queryGroups: [],
  topics: [],
  threads: [],
  queryLadders: [],
  referencePages: [],
  sourceEffectiveness: [],
  workflowMap: {
    profileId: 'chrome:Default',
    roles: [],
    edges: [],
    chromiumEnhanced: false,
  },
  profileFacets: [],
  canonical: {
    windowVisitCount: 12,
    windowUniqueDomains: 4,
    onThisDay: [
      {
        historyId: 1,
        profileId: 'chrome:Default',
        url: 'https://example.com/newer',
        title: 'Newer evidence',
        visitedAt: '2025-04-09T18:30:00.000Z',
        note: 'Newest matching result.',
      },
      {
        historyId: 2,
        profileId: 'chrome:Default',
        url: 'https://example.com/older',
        title: 'Older evidence',
        visitedAt: '2024-04-09T08:00:00.000Z',
        note: 'Older matching result.',
      },
      {
        historyId: 3,
        profileId: 'chrome:Default',
        url: 'https://example.com/other-day',
        title: 'Different day',
        visitedAt: '2025-04-08T08:00:00.000Z',
        note: 'Should be filtered out.',
      },
    ],
    topDomains: [
      { domain: 'sqlite.org', visitCount: 3 },
      { domain: 'example.com', visitCount: 6 },
      { domain: 'rust-lang.org', visitCount: 1 },
    ],
  },
  notes: [],
}

describe('insight canonical helpers', () => {
  test('filters on-this-day evidence by calendar date and newest-first order', () => {
    expect(resolveInsightOnThisDay(baseSnapshot, '04-09', 2)).toEqual([
      baseSnapshot.canonical.onThisDay[0],
      baseSnapshot.canonical.onThisDay[1],
    ])
    expect(resolveInsightOnThisDay(baseSnapshot, null, 2)).toEqual([])
  })

  test('normalizes top-domain counts against the heaviest domain', () => {
    expect(resolveInsightTopDomains(baseSnapshot)).toEqual([
      { domain: 'example.com', count: 6, pct: 100 },
      { domain: 'sqlite.org', count: 3, pct: 50 },
      { domain: 'rust-lang.org', count: 1, pct: 17 },
    ])
    expect(resolveInsightTopDomains(baseSnapshot, 0)).toEqual([])
  })

  test('prefers card summaries and falls back to canonical rollups when needed', () => {
    expect(resolveInsightPeriodicSummary(baseSnapshot, t)).toEqual([
      'Template summaries should be preferred before card summaries.',
      'You revisited semantic search material several times.',
    ])

    expect(
      resolveInsightPeriodicSummary(
        {
          ...baseSnapshot,
          templateSummaries: [],
          cards: [
            { ...baseSnapshot.cards[0], summary: '  ' },
            { ...baseSnapshot.cards[1], summary: 'Repeated summary' },
          ],
        },
        t,
        3,
      ),
    ).toEqual([
      'Repeated summary',
      'Captured 12 visits across 4 domains in the current window.',
      'Most activity clustered around example.com, sqlite.org, rust-lang.org.',
    ])

    expect(
      resolveInsightPeriodicSummary(
        {
          ...baseSnapshot,
          cards: [],
          templateSummaries: [],
          canonical: {
            ...baseSnapshot.canonical,
            topDomains: [],
          },
        },
        t,
        2,
      ),
    ).toEqual(['Captured 12 visits across 4 domains in the current window.'])
  })
})
