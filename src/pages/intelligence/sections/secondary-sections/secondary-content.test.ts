/**
 * @file secondary-content.test.ts
 * @description Unit coverage for the cache-driven emptiness predicates that drop blank secondary-grid slots.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Prove each predicate stays conservative: kept on null/non-ready caches, kept on real content.
 * - Prove each predicate reports "no content" only when a ready payload is provably empty.
 *
 * ## Not responsible for
 * - Re-testing the underlying heuristics (covered by `heuristics.test.ts`).
 * - Asserting layout behavior, which is covered by the coordinator suite.
 */

import { describe, expect, test } from 'vitest'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  FrictionSignal,
  PathFlow,
  ReopenedInvestigation,
  StableSource,
} from '../../../../lib/core-intelligence'
import {
  hasDiscoveryTrendContent,
  hasFrictionContent,
  hasPathFlowsContent,
  hasReopenedInvestigationsContent,
  hasSearchEffectivenessContent,
} from './secondary-content'

function ready<T>(data: T): CoreIntelligenceSectionResult<T> {
  return { data, meta: meta('ready') }
}

function meta(
  state: CoreIntelligenceSectionMeta['state'],
): CoreIntelligenceSectionMeta {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    includesEnrichment: false,
    moduleIds: ['secondary'],
    notes: [],
    sectionId: 'secondary',
    sourceTables: ['core_intelligence'],
    state,
    stateReason: null,
    window: {
      dateRange: { start: '2026-04-01', end: '2026-04-30' },
      kind: 'date-range',
    },
  }
}

describe('secondary-content predicates', () => {
  test('keep slots when the cache is missing (emptiness unknown)', () => {
    expect(hasSearchEffectivenessContent(null)).toBe(true)
    expect(hasFrictionContent(null)).toBe(true)
    expect(hasReopenedInvestigationsContent(null)).toBe(true)
    expect(hasDiscoveryTrendContent(null)).toBe(true)
    expect(hasPathFlowsContent(null)).toBe(true)
  })

  test('keep slots when a cached snapshot is not ready yet', () => {
    const staleEmpty: CoreIntelligenceSectionResult<FrictionSignal[]> = {
      data: [],
      meta: meta('stale'),
    }
    expect(hasFrictionContent(staleEmpty)).toBe(true)
    expect(
      hasSearchEffectivenessContent({
        data: { engineStats: [], hardestTopics: [], topResolvingSources: [] },
        meta: meta('disabled'),
      }),
    ).toBe(true)
    expect(
      hasDiscoveryTrendContent({
        data: { availableYears: [], points: [] },
        meta: meta('degraded'),
      }),
    ).toBe(true)
  })

  test('drop search effectiveness only when every list is empty', () => {
    expect(
      hasSearchEffectivenessContent(
        ready({ engineStats: [], hardestTopics: [], topResolvingSources: [] }),
      ),
    ).toBe(false)
    expect(
      hasSearchEffectivenessContent(
        ready({
          engineStats: [],
          hardestTopics: [],
          topResolvingSources: [stableSource('landing')],
        }),
      ),
    ).toBe(true)
  })

  test('drop friction when no meaningful signal survives filtering', () => {
    expect(hasFrictionContent(ready([weakFrictionSignal()]))).toBe(false)
    expect(hasFrictionContent(ready([strongFrictionSignal()]))).toBe(true)
  })

  test('drop reopened investigations when no search-backed item survives', () => {
    expect(
      hasReopenedInvestigationsContent(ready([navigationalReopened()])),
    ).toBe(false)
    expect(
      hasReopenedInvestigationsContent(ready([searchBackedReopened()])),
    ).toBe(true)
  })

  test('drop discovery trend only when there are no weekly points', () => {
    expect(
      hasDiscoveryTrendContent(ready({ availableYears: [], points: [] })),
    ).toBe(false)
    expect(
      hasDiscoveryTrendContent(
        ready({
          availableYears: [2026],
          points: [
            {
              dateKey: '2026-W12',
              discoveryRate: 0.5,
              newDomainCount: 3,
              totalVisits: 10,
            },
          ],
        }),
      ),
    ).toBe(true)
  })

  test('drop path flows when no meaningful flow survives filtering', () => {
    expect(hasPathFlowsContent(ready([oneOffPathFlow()]))).toBe(false)
    expect(hasPathFlowsContent(ready([repeatedPathFlow()]))).toBe(true)
  })
})

function stableSource(role: StableSource['sourceRole']): StableSource {
  return {
    effectivenessScore: 0.9,
    registrableDomain: `${role}.example`,
    sourceRole: role,
    stableLandingCount: role === 'landing' ? 5 : 0,
    trailCount: role === 'entry' ? 6 : 0,
  }
}

function weakFrictionSignal(): FrictionSignal {
  return {
    description: 'one-off blip',
    evidenceType: 'weak',
    occurrenceCount: 1,
    registrableDomain: 'weak.example',
    signalKind: 'http_error',
  }
}

function strongFrictionSignal(): FrictionSignal {
  return {
    description: 'repeated bounce',
    evidenceType: 'strong',
    occurrenceCount: 3,
    registrableDomain: 'strong.example',
    signalKind: 'bounce_pattern',
  }
}

function navigationalReopened(): ReopenedInvestigation {
  return {
    anchorId: 'r-1',
    anchorLabel: 'login.example',
    anchorType: 'reference_page',
    distinctDays: 5,
    firstSeenAt: '2026-04-01',
    investigationId: 'inv-1',
    lastSeenAt: '2026-04-10',
    occurrenceCount: 4,
  }
}

function searchBackedReopened(): ReopenedInvestigation {
  return {
    anchorId: 'q-1',
    anchorLabel: 'how to test react components',
    anchorType: 'query_family',
    distinctDays: 3,
    firstSeenAt: '2026-04-01',
    investigationId: 'inv-2',
    lastSeenAt: '2026-04-10',
    occurrenceCount: 4,
  }
}

function oneOffPathFlow(): PathFlow {
  return {
    flowId: 'f-1',
    flowPattern: 'a.example -> b.example',
    lastSeenAt: '2026-04-10',
    occurrenceCount: 1,
    stepCount: 2,
    steps: [
      { index: 0, label: 'a.example', registrableDomain: 'a.example' },
      { index: 1, label: 'b.example', registrableDomain: 'b.example' },
    ],
  }
}

function repeatedPathFlow(): PathFlow {
  return {
    flowId: 'f-2',
    flowPattern: 'a.example -> b.example',
    lastSeenAt: '2026-04-10',
    occurrenceCount: 4,
    stepCount: 2,
    steps: [
      { index: 0, label: 'a.example', registrableDomain: 'a.example' },
      { index: 1, label: 'b.example', registrableDomain: 'b.example' },
    ],
  }
}
