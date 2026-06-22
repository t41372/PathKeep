import { describe, expect, test } from 'vitest'
import type { HistoryEntry } from '@/lib/types/archive'
import type {
  AiIndexStatus,
  AiQueueStatus,
  AiSearchResultItem,
} from '@/lib/types/intelligence'
import { createNamespaceTranslator } from '@/lib/i18n'
import {
  buildPaperSearchDayGroups,
  buildPaperSearchRelevanceList,
  buildSmartScopeLine,
  deriveSmartIndexProgress,
  explorerStateFromPaperSearchMode,
  paperSearchEntryFromAiSearchItem,
  paperSearchEntryFromHistoryEntry,
  paperSearchModeFromExplorerState,
} from './paper-search-helpers'

// Mirrors the `scoreBand` thresholds in lib/intelligence-ai-presentation so
// the adapter tests assert the real band labels/tones, not a stub.
function intelligenceT(key: string) {
  return key
}

function makeAiItem(
  over: Partial<AiSearchResultItem> = {},
): AiSearchResultItem {
  return {
    historyId: 42,
    profileId: 'chrome:Default',
    url: 'https://example.com/docs',
    title: 'SQLite WAL guide',
    domain: 'example.com',
    visitedAt: '2026-05-17T10:30:00',
    score: 0.91,
    matchReason: 'Lexical + semantic match',
    ...over,
  }
}

function makeEntry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: 1,
    profileId: 'chrome:Default',
    url: 'https://example.com/',
    title: 'Example',
    domain: 'example.com',
    favicon: null,
    visitedAt: '2026-05-17T10:30:00',
    visitTime: 1747488600,
    durationMs: null,
    transition: 0,
    sourceVisitId: 0,
    appId: null,
    ...over,
  }
}

describe('paperSearchModeFromExplorerState', () => {
  test('regexMode wins over the AI mode', () => {
    expect(paperSearchModeFromExplorerState('hybrid', true)).toBe('regex')
    expect(paperSearchModeFromExplorerState('semantic', true)).toBe('regex')
  })

  test('hybrid without regex maps to the unified Smart mode', () => {
    expect(paperSearchModeFromExplorerState('hybrid', false)).toBe('smart')
  })

  test('legacy semantic without regex aliases onto the Smart mode', () => {
    // REACH-B: `?mode=semantic` is a read-only alias for Smart so old deep
    // links keep working even though the UI only writes `hybrid`.
    expect(paperSearchModeFromExplorerState('semantic', false)).toBe('smart')
  })

  test('keyword without regex maps to keyword', () => {
    expect(paperSearchModeFromExplorerState('keyword', false)).toBe('keyword')
  })
})

describe('explorerStateFromPaperSearchMode', () => {
  test('regex pins mode to keyword and toggles regex on', () => {
    expect(explorerStateFromPaperSearchMode('regex')).toEqual({
      mode: 'keyword',
      regexMode: true,
    })
  })

  test('smart writes the honest hybrid URL mode and clears regex', () => {
    // We always WRITE `hybrid` (the real backend behavior), never `semantic`.
    expect(explorerStateFromPaperSearchMode('smart')).toEqual({
      mode: 'hybrid',
      regexMode: false,
    })
  })

  test('smart round-trips through ?mode=hybrid', () => {
    const url = explorerStateFromPaperSearchMode('smart')
    expect(paperSearchModeFromExplorerState(url.mode, url.regexMode)).toBe(
      'smart',
    )
  })

  test('keyword clears everything', () => {
    expect(explorerStateFromPaperSearchMode('keyword')).toEqual({
      mode: 'keyword',
      regexMode: false,
    })
  })
})

describe('paperSearchEntryFromHistoryEntry', () => {
  test('formats local time as HH:mm with zero padding', () => {
    const entry = paperSearchEntryFromHistoryEntry(
      makeEntry({ visitedAt: '2026-05-17T07:05:00' }),
    )
    expect(entry.time).toBe('07:05')
  })

  test('falls back to URL when title is missing or whitespace', () => {
    const fromBlankTitle = paperSearchEntryFromHistoryEntry(
      makeEntry({ title: '   ', url: 'https://example.com/x' }),
    )
    expect(fromBlankTitle.title).toBe('https://example.com/x')
    const fromNullTitle = paperSearchEntryFromHistoryEntry(
      makeEntry({ title: null, url: 'https://example.com/y' }),
    )
    expect(fromNullTitle.title).toBe('https://example.com/y')
  })

  test('maps every known transition code to its label', () => {
    const cases: ReadonlyArray<[number, string]> = [
      [0, 'link'],
      [1, 'typed'],
      [2, 'auto-bookmark'],
      [3, 'auto-subframe'],
      [4, 'manual-subframe'],
      [5, 'generated'],
      [6, 'start-page'],
      [7, 'form-submit'],
      [8, 'reload'],
      [9, 'keyword'],
      [10, 'keyword-generated'],
    ]
    for (const [code, label] of cases) {
      expect(
        paperSearchEntryFromHistoryEntry(makeEntry({ transition: code }))
          .transitionType,
      ).toBe(label)
    }
  })

  test('returns undefined transitionType for unknown codes', () => {
    expect(
      paperSearchEntryFromHistoryEntry(makeEntry({ transition: 999 }))
        .transitionType,
    ).toBeUndefined()
  })

  test('returns undefined transitionType when the entry has no transition', () => {
    expect(
      paperSearchEntryFromHistoryEntry(makeEntry({ transition: null }))
        .transitionType,
    ).toBeUndefined()
  })

  test('time is empty when visitedAt is not parseable', () => {
    expect(
      paperSearchEntryFromHistoryEntry(makeEntry({ visitedAt: 'nonsense' }))
        .time,
    ).toBe('')
  })

  test('threads a non-empty enrichment excerpt through to the result entry', () => {
    const entry = paperSearchEntryFromHistoryEntry(
      makeEntry({ enrichmentExcerpt: 'Reusable workflow runner • CI' }),
    )
    expect(entry.enrichmentExcerpt).toBe('Reusable workflow runner • CI')
  })

  test('drops a blank or whitespace-only enrichment excerpt to undefined', () => {
    expect(
      paperSearchEntryFromHistoryEntry(makeEntry({ enrichmentExcerpt: '   ' }))
        .enrichmentExcerpt,
    ).toBeUndefined()
    expect(
      paperSearchEntryFromHistoryEntry(makeEntry({ enrichmentExcerpt: null }))
        .enrichmentExcerpt,
    ).toBeUndefined()
  })

  test('leaves the excerpt undefined for preview-fixture rows that omit it', () => {
    // The preview/browser fixtures build HistoryEntry without the field; the
    // mapping must not crash and must yield undefined (suppressing the affordance).
    const entry = paperSearchEntryFromHistoryEntry(makeEntry())
    expect(entry.enrichmentExcerpt).toBeUndefined()
  })
})

describe('buildPaperSearchDayGroups', () => {
  test('returns empty array for empty input', () => {
    expect(buildPaperSearchDayGroups([], { language: 'en' })).toEqual([])
  })

  test('groups by local day, sorts days newest-first, sorts entries within a day newest-first', () => {
    const entries: HistoryEntry[] = [
      makeEntry({ id: 1, visitedAt: '2026-05-15T08:00:00' }),
      makeEntry({ id: 2, visitedAt: '2026-05-17T09:00:00' }),
      makeEntry({ id: 3, visitedAt: '2026-05-17T10:30:00' }),
      makeEntry({ id: 4, visitedAt: '2026-05-16T22:00:00' }),
    ]
    const groups = buildPaperSearchDayGroups(entries, { language: 'en' })
    expect(groups.map((g) => g.date)).toEqual([
      '2026-05-17',
      '2026-05-16',
      '2026-05-15',
    ])
    expect(groups[0].entries.map((entry) => entry.id)).toEqual([3, 2])
    expect(groups[0].label).toContain('17')
  })

  test('a single-entry day still gets a label', () => {
    const groups = buildPaperSearchDayGroups(
      [makeEntry({ id: 9, visitedAt: '2026-05-17T10:30:00' })],
      { language: 'en' },
    )
    expect(groups).toHaveLength(1)
    expect(groups[0].entries).toHaveLength(1)
    expect(groups[0].label).toMatch(/2026/)
  })

  test('within-day sort treats NaN timestamps as equal without throwing', () => {
    // Both entries get the same day key (the raw 10-char prefix) so they
    // land in the same bucket; the sort comparator then has to fall back
    // to the NaN-safe `return 0` branch.
    const groups = buildPaperSearchDayGroups(
      [
        makeEntry({ id: 1, visitedAt: 'bogus-date' }),
        makeEntry({ id: 2, visitedAt: 'bogus-date' }),
      ],
      { language: 'en' },
    )
    expect(groups).toHaveLength(1)
    expect(groups[0].entries).toHaveLength(2)
  })
})

describe('paperSearchEntryFromAiSearchItem', () => {
  test('keeps the real historyId so the row binds to the detail panel', () => {
    const entry = paperSearchEntryFromAiSearchItem(
      makeAiItem({ historyId: 1234 }),
      intelligenceT,
    )
    expect(entry.id).toBe(1234)
  })

  test('surfaces matchReason verbatim (NO faked snippet)', () => {
    const entry = paperSearchEntryFromAiSearchItem(
      makeAiItem({ matchReason: 'Semantic match (Starred)' }),
      intelligenceT,
    )
    expect(entry.matchReason).toBe('Semantic match (Starred)')
    // The AI item carries no snippet field, so the adapter must never invent one.
    expect(entry.snippet).toBeUndefined()
    expect(entry.enrichmentExcerpt).toBeUndefined()
  })

  test('derives the relevance band from score via scoreBand thresholds', () => {
    expect(
      paperSearchEntryFromAiSearchItem(
        makeAiItem({ score: 0.9 }),
        intelligenceT,
      ).relevanceBand,
    ).toEqual({ label: 'highConfidence', tone: 'success' })
    expect(
      paperSearchEntryFromAiSearchItem(
        makeAiItem({ score: 0.7 }),
        intelligenceT,
      ).relevanceBand,
    ).toEqual({ label: 'relevant', tone: 'warning' })
    expect(
      paperSearchEntryFromAiSearchItem(
        makeAiItem({ score: 0.2 }),
        intelligenceT,
      ).relevanceBand,
    ).toEqual({ label: 'weakMatch', tone: 'info' })
  })

  test('stamps the local day key for see-in-context', () => {
    const entry = paperSearchEntryFromAiSearchItem(
      makeAiItem({ visitedAt: '2026-05-17T10:30:00' }),
      intelligenceT,
    )
    expect(entry.dayKey).toBe('2026-05-17')
  })

  test('falls back to the URL when the title is missing or blank', () => {
    expect(
      paperSearchEntryFromAiSearchItem(
        makeAiItem({ title: null, url: 'https://example.com/y' }),
        intelligenceT,
      ).title,
    ).toBe('https://example.com/y')
    expect(
      paperSearchEntryFromAiSearchItem(
        makeAiItem({ title: '   ', url: 'https://example.com/z' }),
        intelligenceT,
      ).title,
    ).toBe('https://example.com/z')
  })
})

describe('buildPaperSearchRelevanceList', () => {
  test('preserves the backend ranking order (no re-sort, no day grouping)', () => {
    const list = buildPaperSearchRelevanceList(
      [
        makeAiItem({ historyId: 3, visitedAt: '2026-05-10T08:00:00' }),
        makeAiItem({ historyId: 1, visitedAt: '2026-05-17T08:00:00' }),
        makeAiItem({ historyId: 2, visitedAt: '2026-05-12T08:00:00' }),
      ],
      intelligenceT,
    )
    // Ranking order is kept exactly as the backend returned it (3, 1, 2) — NOT
    // re-sorted newest-first the way the day-grouped keyword path would.
    expect(list.map((entry) => entry.id)).toEqual([3, 1, 2])
  })

  test('returns an empty list for no items', () => {
    expect(buildPaperSearchRelevanceList([], intelligenceT)).toEqual([])
  })
})

function makeQueueStatus(over: Partial<AiQueueStatus> = {}): AiQueueStatus {
  return {
    paused: false,
    concurrency: 1,
    queued: 0,
    running: 0,
    failed: 0,
    recentJobs: [],
    ...over,
  }
}

function makeAiStatus(over: Partial<AiIndexStatus> = {}): AiIndexStatus {
  return {
    enabled: true,
    assistantEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    state: 'ready',
    ready: true,
    indexedItems: 0,
    lastIndexedAt: null,
    queuePaused: false,
    queueConcurrency: 1,
    queuedJobs: 0,
    runningJobs: 0,
    failedJobs: 0,
    recentJobs: [],
    semanticSidecarBytes: 0,
    semanticMetadataBytes: 0,
    estimatedEmbeddingTokens: 0,
    ...over,
  }
}

describe('deriveSmartIndexProgress', () => {
  test('idle when nothing is queued/running and no pending click', () => {
    const progress = deriveSmartIndexProgress({
      queueStatus: makeQueueStatus(),
      snapshotAiStatus: makeAiStatus({ indexedItems: 1200 }),
      pendingAction: false,
    })
    expect(progress).toEqual({
      phase: 'idle',
      active: false,
      queuedJobs: 0,
      runningJobs: 0,
      indexedItems: 1200,
    })
  })

  test('running when the live queue reports a running job', () => {
    const progress = deriveSmartIndexProgress({
      queueStatus: makeQueueStatus({ running: 1, queued: 3 }),
      snapshotAiStatus: makeAiStatus({ indexedItems: 50 }),
      pendingAction: false,
    })
    expect(progress.phase).toBe('running')
    expect(progress.active).toBe(true)
    expect(progress.runningJobs).toBe(1)
    expect(progress.queuedJobs).toBe(3)
  })

  test('queued when a build is enqueued (live or just-clicked) but not running', () => {
    // Live queue says queued > 0, running == 0.
    expect(
      deriveSmartIndexProgress({
        queueStatus: makeQueueStatus({ queued: 2 }),
        snapshotAiStatus: makeAiStatus(),
        pendingAction: false,
      }).phase,
    ).toBe('queued')
    // Just-clicked Build with no live counts yet — the local intent flag keeps
    // the CTA honest (queued, NOT "built") across the gap before the first poll.
    const pending = deriveSmartIndexProgress({
      queueStatus: makeQueueStatus(),
      snapshotAiStatus: makeAiStatus(),
      pendingAction: true,
    })
    expect(pending.phase).toBe('queued')
    expect(pending.active).toBe(true)
  })

  test('paused when a build is enqueued while the queue is paused', () => {
    const progress = deriveSmartIndexProgress({
      queueStatus: makeQueueStatus({ queued: 1, paused: true }),
      snapshotAiStatus: makeAiStatus(),
      pendingAction: false,
    })
    expect(progress.phase).toBe('paused')
    expect(progress.active).toBe(true)
  })

  test('a paused queue with NOTHING enqueued stays idle, not paused', () => {
    // Paused is only meaningful when there is work waiting on the queue.
    const progress = deriveSmartIndexProgress({
      queueStatus: makeQueueStatus({ paused: true }),
      snapshotAiStatus: makeAiStatus({ indexedItems: 10 }),
      pendingAction: false,
    })
    expect(progress.phase).toBe('idle')
  })

  test('running wins over paused when a job is actively running', () => {
    const progress = deriveSmartIndexProgress({
      queueStatus: makeQueueStatus({ running: 1, paused: true }),
      snapshotAiStatus: makeAiStatus(),
      pendingAction: false,
    })
    expect(progress.phase).toBe('running')
  })

  test('falls back to the snapshot queue counts when no live queue yet', () => {
    const progress = deriveSmartIndexProgress({
      queueStatus: null,
      snapshotAiStatus: makeAiStatus({
        queuedJobs: 4,
        runningJobs: 1,
        indexedItems: 7,
      }),
      pendingAction: false,
    })
    expect(progress.phase).toBe('running')
    expect(progress.queuedJobs).toBe(4)
    expect(progress.runningJobs).toBe(1)
    expect(progress.indexedItems).toBe(7)
  })

  test('tolerates a null snapshot (shell still loading)', () => {
    const progress = deriveSmartIndexProgress({
      queueStatus: null,
      snapshotAiStatus: null,
      pendingAction: false,
    })
    expect(progress).toEqual({
      phase: 'idle',
      active: false,
      queuedJobs: 0,
      runningJobs: 0,
      indexedItems: 0,
    })
  })
})

describe('buildSmartScopeLine', () => {
  const explorerT = createNamespaceTranslator('en', 'explorer')

  test('joins indexed coverage + freshness when both are available', () => {
    const line = buildSmartScopeLine({
      indexedItems: 1240,
      lastIndexedAt: '2026-05-17T10:30:00Z',
      language: 'en',
      explorerT,
    })
    expect(line).toContain('1,240')
    // Freshness piece formats the RFC3339 timestamp into a compact day label.
    expect(line).toContain('updated')
    expect(line).toContain('·')
  })

  test('omits the freshness piece when there is no lastIndexedAt', () => {
    const line = buildSmartScopeLine({
      indexedItems: 5,
      lastIndexedAt: null,
      language: 'en',
      explorerT,
    })
    expect(line).toContain('5')
    expect(line).not.toContain('·')
    expect(line).not.toContain('updated')
  })

  test('omits the coverage piece when nothing is indexed', () => {
    const line = buildSmartScopeLine({
      indexedItems: 0,
      lastIndexedAt: '2026-05-17T10:30:00Z',
      language: 'en',
      explorerT,
    })
    expect(line).not.toContain('indexed')
    expect(line).toContain('updated')
  })

  test('returns null when there is nothing honest to say', () => {
    expect(
      buildSmartScopeLine({
        indexedItems: 0,
        lastIndexedAt: null,
        language: 'en',
        explorerT,
      }),
    ).toBeNull()
  })

  test('omits freshness for an unparseable timestamp rather than printing Invalid Date', () => {
    const line = buildSmartScopeLine({
      indexedItems: 3,
      lastIndexedAt: 'not-a-date',
      language: 'en',
      explorerT,
    })
    expect(line).not.toContain('Invalid')
    expect(line).not.toContain('updated')
    expect(line).toContain('3')
  })
})
