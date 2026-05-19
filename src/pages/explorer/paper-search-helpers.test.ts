import { describe, expect, test } from 'vitest'
import type { HistoryEntry } from '@/lib/types/archive'
import {
  buildPaperSearchDayGroups,
  explorerStateFromPaperSearchMode,
  paperSearchEntryFromHistoryEntry,
  paperSearchModeFromExplorerState,
} from './paper-search-helpers'

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
  test('regexMode wins over semantic', () => {
    expect(paperSearchModeFromExplorerState('semantic', true)).toBe('regex')
  })

  test('semantic without regex maps to semantic', () => {
    expect(paperSearchModeFromExplorerState('semantic', false)).toBe('semantic')
  })

  test('hybrid and keyword without regex both map to keyword', () => {
    expect(paperSearchModeFromExplorerState('keyword', false)).toBe('keyword')
    expect(paperSearchModeFromExplorerState('hybrid', false)).toBe('keyword')
  })
})

describe('explorerStateFromPaperSearchMode', () => {
  test('regex pins mode to keyword and toggles regex on', () => {
    expect(explorerStateFromPaperSearchMode('regex')).toEqual({
      mode: 'keyword',
      regexMode: true,
    })
  })

  test('semantic clears regex flag', () => {
    expect(explorerStateFromPaperSearchMode('semantic')).toEqual({
      mode: 'semantic',
      regexMode: false,
    })
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
})
