import { describe, expect, test } from 'vitest'
import type {
  FrictionSignal,
  PathFlow,
  ReopenedInvestigation,
  StableSource,
} from '../../../../lib/core-intelligence'
import {
  hasMeaningfulStableSources,
  humanizeDiscoveryWeekLabel,
  isMeaningfulFrictionSignal,
  isMeaningfulPathFlow,
  isSearchBackedReopenedInvestigation,
} from './heuristics'

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('secondary-section heuristics', () => {
  test('requires both entry and landing evidence for stable sources', () => {
    expect(hasMeaningfulStableSources([stableSource()], [])).toBe(false)
    expect(hasMeaningfulStableSources([], [stableSource()])).toBe(false)
    expect(hasMeaningfulStableSources([stableSource()], [stableSource()])).toBe(
      true,
    )
  })

  test('keeps only actionable friction signals', () => {
    expect(
      isMeaningfulFrictionSignal(
        frictionSignal({
          description: '   ',
          evidenceType: 'strong',
          occurrenceCount: 99,
          signalKind: 'bounce_pattern',
        }),
      ),
    ).toBe(false)
    expect(isMeaningfulFrictionSignal(frictionSignal())).toBe(true)
    expect(
      isMeaningfulFrictionSignal(
        frictionSignal({
          evidenceType: 'weak',
          occurrenceCount: 2,
          signalKind: 'redirect_chain',
        }),
      ),
    ).toBe(true)
    expect(
      isMeaningfulFrictionSignal(
        frictionSignal({
          evidenceType: 'weak',
          occurrenceCount: 1,
          signalKind: 'http_error',
        }),
      ),
    ).toBe(false)
  })

  test('filters reopened investigations to recurring search-backed questions', () => {
    expect(
      isSearchBackedReopenedInvestigation(
        reopened({
          anchorType: 'reference_page',
        }),
      ),
    ).toBe(false)
    expect(
      isSearchBackedReopenedInvestigation(
        reopened({
          occurrenceCount: 1,
          distinctDays: 2,
        }),
      ),
    ).toBe(false)
    expect(
      isSearchBackedReopenedInvestigation(
        reopened({
          anchorLabel: 'https://example.com/auth/callback',
        }),
      ),
    ).toBe(false)
    expect(
      isSearchBackedReopenedInvestigation(
        reopened({
          anchorLabel: '',
        }),
      ),
    ).toBe(false)
    expect(
      isSearchBackedReopenedInvestigation(
        reopened({
          anchorLabel: 'sqlite wal checkpoint',
        }),
      ),
    ).toBe(true)
    expect(
      isSearchBackedReopenedInvestigation(
        reopened({
          anchorLabel: 'longrecurring',
        }),
      ),
    ).toBe(true)
  })

  test('keeps meaningful cross-domain path flows and rejects utility loops', () => {
    expect(
      isMeaningfulPathFlow(
        pathFlow({
          occurrenceCount: 1,
          flowPattern: 'Search -> Example',
        }),
      ),
    ).toBe(false)
    expect(
      isMeaningfulPathFlow(
        pathFlow({
          flowPattern: 'Search -> login.example.com',
        }),
      ),
    ).toBe(false)
    expect(
      isMeaningfulPathFlow(
        pathFlow({
          flowPattern: 'https://chat.openai.com/c -> https://chatgpt.com/d',
        }),
      ),
    ).toBe(false)
    expect(
      isMeaningfulPathFlow(
        pathFlow({
          flowPattern: 'https://twitter.com/a -> https://x.com/b',
        }),
      ),
    ).toBe(false)
    expect(
      isMeaningfulPathFlow(
        pathFlow({
          flowPattern: 'search.example -> docs.example -> docs.example',
        }),
      ),
    ).toBe(false)
    expect(
      isMeaningfulPathFlow(
        pathFlow({
          flowPattern: 'Search → Example Docs',
        }),
      ),
    ).toBe(true)
  })

  test('humanizes valid ISO week labels and preserves malformed keys', () => {
    expect(humanizeDiscoveryWeekLabel('2026-W04', t)).toBe(
      'discoveryTrendWeekLabel:{"year":2026,"week":4}',
    )
    expect(humanizeDiscoveryWeekLabel('2026-04', t)).toBe('2026-04')
  })
})

function stableSource(): StableSource {
  return {
    registrableDomain: 'example.com',
    displayName: 'Example',
    sourceRole: 'entry',
    trailCount: 2,
    stableLandingCount: 2,
    effectivenessScore: 0.8,
  }
}

function frictionSignal(
  overrides: Partial<FrictionSignal> = {},
): FrictionSignal {
  return {
    registrableDomain: 'example.com',
    url: null,
    evidenceType: 'strong',
    signalKind: 'bounce_pattern',
    occurrenceCount: 3,
    description: 'Repeated bounces',
    ...overrides,
  }
}

function reopened(
  overrides: Partial<ReopenedInvestigation> = {},
): ReopenedInvestigation {
  return {
    investigationId: 'investigation-1',
    anchorType: 'query_family',
    anchorId: 'family-1',
    anchorLabel: 'why sqlite wal',
    occurrenceCount: 2,
    distinctDays: 2,
    firstSeenAt: '2026-04-01T00:00:00Z',
    lastSeenAt: '2026-04-20T00:00:00Z',
    ...overrides,
  }
}

function pathFlow(overrides: Partial<PathFlow> = {}): PathFlow {
  return {
    flowId: 'flow-1',
    flowPattern: 'Search -> Example',
    stepCount: 2,
    occurrenceCount: 2,
    lastSeenAt: '2026-04-20T00:00:00Z',
    steps: [],
    ...overrides,
  }
}
