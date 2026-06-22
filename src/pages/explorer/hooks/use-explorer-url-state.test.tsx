/**
 * @file use-explorer-url-state.test.tsx
 * @description Router-integrated coverage for Explorer URL state, pagination, and recent-search persistence.
 * @module pages/explorer/hooks
 *
 * ## Responsibilities
 * - Verify Explorer route params stay synchronized with hook handlers.
 * - Protect regex validation, history page-size persistence, semantic cursor trails, and date shortcuts.
 * - Cover debounced query commits that pure URL derivation tests cannot exercise.
 *
 * ## Not responsible for
 * - Re-testing backend history/semantic query execution.
 * - Rendering Explorer panels.
 *
 * ## Dependencies
 * - Uses MemoryRouter because the hook owns `useSearchParams` integration.
 *
 * ## Performance notes
 * - Keeps the hook under test isolated from backend calls and route rendering.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../../lib/i18n'
import {
  defaultKeywordPageSize,
  explorerPageSizeStorageKey,
  recentSearchesStorageKey,
  toLocalDateString,
} from '../helpers'
import { useExplorerUrlState } from './use-explorer-url-state'

const explorerT = createNamespaceTranslator('en', 'explorer')

function createWrapper(initialEntry: string) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <MemoryRouter initialEntries={[initialEntry]}>{children}</MemoryRouter>
    )
  }
}

function renderExplorerUrlState(initialEntry: string) {
  return renderHook(
    () =>
      useExplorerUrlState({
        activeProfileId: 'safari:Work',
        explorerT,
        language: 'en',
        selectedProfileIds: ['chrome:Default', 'safari:Work'],
      }),
    { wrapper: createWrapper(initialEntry) },
  )
}

describe('useExplorerUrlState', () => {
  beforeEach(() => {
    window.localStorage.clear()
  })

  test('treats JavaScript-only lookaround regex as invalid before backend search', () => {
    const { result } = renderExplorerUrlState(
      '/explorer?q=%5E((%3F!pathkeep).)*%24&regex=1',
    )

    expect(result.current.regexValid).toBe(false)
    expect(result.current.currentQuery).toEqual(
      expect.objectContaining({
        q: '^((?!pathkeep).)*$',
        regexMode: true,
      }),
    )
  })

  test('derives route params and updates history view pagination controls', async () => {
    const { result } = renderExplorerUrlState(
      '/explorer?q=%5B&regex=1&mode=keyword&view=session&profileId=chrome%3ADefault&browserKind=chrome&start=2026-04-01&end=2026-04-20&page=3&pageSize=100&cursor=c1&semanticCursor=s1',
    )

    expect(result.current.regexValid).toBe(false)
    expect(result.current.profileId).toBe('chrome:Default')
    expect(result.current.browserKinds).toEqual(['chrome', 'safari'])
    expect(result.current.currentQuery).toEqual(
      expect.objectContaining({
        browserKind: 'chrome',
        cursor: null,
        limit: 100,
        page: 3,
        profileId: 'chrome:Default',
        q: '[',
        regexMode: true,
      }),
    )
    expect(result.current.activeFilters.map((filter) => filter.id)).toEqual([
      'q',
      'view',
      'regex',
      'profileId',
      'browserKind',
      'start',
      'end',
    ])

    act(() => {
      result.current.handleNextHistoryPage(3)
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('page')).toBe('4'),
    )

    act(() => {
      result.current.handlePreviousHistoryPage(4)
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('page')).toBe('3'),
    )

    act(() => {
      result.current.handleFirstHistoryPage(3)
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('page')).toBeNull(),
    )

    act(() => {
      result.current.handleLastHistoryPage(1, 9)
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('page')).toBe('9'),
    )

    act(() => {
      result.current.setHistoryPageSize(25)
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('pageSize')).toBe('25'),
    )
    expect(window.localStorage.getItem(explorerPageSizeStorageKey)).toBe('25')
    expect(result.current.searchParams.get('page')).toBeNull()

    act(() => {
      result.current.setView('trail')
    })
    await waitFor(() => expect(result.current.view).toBe('trail'))
    expect(result.current.searchParams.get('semanticCursor')).toBeNull()

    act(() => {
      result.current.clearDateRange()
    })
    await waitFor(() => expect(result.current.start).toBeNull())
    expect(result.current.end).toBeNull()

    act(() => {
      result.current.clearAllFilters()
    })
    await waitFor(() => expect(result.current.searchParams.toString()).toBe(''))
  })

  test('debounces query commits, persists recent searches, and applies date shortcuts', async () => {
    const { result } = renderExplorerUrlState(
      '/explorer?q=old&page=2&cursor=c1&semanticCursor=s1',
    )

    vi.useFakeTimers()
    act(() => {
      result.current.setQueryInput('new recall')
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(181)
    })
    vi.useRealTimers()

    expect(result.current.searchParams.get('q')).toBe('new recall')
    expect(result.current.searchParams.get('page')).toBeNull()
    expect(result.current.searchParams.get('cursor')).toBeNull()
    expect(result.current.searchParams.get('semanticCursor')).toBeNull()

    act(() => {
      result.current.persistRecentSearch({
        q: 'sqlite',
        mode: 'semantic',
        view: 'time',
        domain: 'example.com',
        profileId: 'chrome:Default',
        browserKind: 'chrome',
        start: '2026-04-01',
        end: '2026-04-20',
        regex: '1',
      })
    })
    expect(
      JSON.parse(window.localStorage.getItem(recentSearchesStorageKey) ?? '[]'),
    ).toEqual([
      expect.objectContaining({
        // L-2: the unified "Smart" vocabulary covers the legacy `semantic` alias.
        label:
          'Smart · Enabled · sqlite · example.com · Chrome · Default · Chrome · Apr 1 - Apr 20',
        params: expect.objectContaining({ q: 'sqlite', sort: 'newest' }),
      }),
    ])

    act(() => {
      result.current.persistRecentSearch({
        q: '',
        mode: 'keyword',
        view: 'time',
        domain: null,
        profileId: null,
        browserKind: null,
        start: null,
        end: '2026-01-02',
        regex: null,
      })
      result.current.persistRecentSearch({
        q: '',
        mode: 'keyword',
        view: 'time',
        domain: null,
        profileId: null,
        browserKind: null,
        start: 'not-a-date',
        end: '2026-01-02',
        regex: null,
      })
    })
    const persistedSearches = JSON.parse(
      window.localStorage.getItem(recentSearchesStorageKey) ?? '[]',
    ) as Array<{ label: string }>
    expect(persistedSearches.map((entry) => entry.label)).toEqual(
      expect.arrayContaining(['All time - Jan 2', 'not-a-date - Jan 2']),
    )

    const endDate = new Date()
    const startDate = new Date(endDate)
    startDate.setDate(endDate.getDate() - 6)

    act(() => {
      result.current.applyDateShortcut(7)
    })
    await waitFor(() =>
      expect(result.current.start).toBe(toLocalDateString(startDate)),
    )
    expect(result.current.end).toBe(toLocalDateString(endDate))
    expect(result.current.activeDateShortcut()).toBe('week')
  })

  test('tracks semantic cursor history and clamps manual history jumps', async () => {
    const { result } = renderExplorerUrlState(
      '/explorer?mode=semantic&q=vector&semanticCursor=cursor-2',
    )

    act(() => {
      result.current.handleNextSemanticPage(null)
    })
    expect(result.current.searchParams.get('semanticCursor')).toBe('cursor-2')

    act(() => {
      result.current.handleNextSemanticPage('cursor-3')
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('semanticCursor')).toBe(
        'cursor-3',
      ),
    )
    expect(result.current.semanticTrail).toEqual(['cursor-2'])

    act(() => {
      result.current.handlePreviousSemanticPage()
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('semanticCursor')).toBe(
        'cursor-2',
      ),
    )

    act(() => {
      result.current.setHistoryPageInput('not-a-number')
    })
    await waitFor(() =>
      expect(result.current.historyPageInput).toBe('not-a-number'),
    )
    act(() => {
      result.current.handleHistoryPageJump(4, 10)
    })
    expect(result.current.historyPageInput).toBe('4')

    act(() => {
      result.current.setHistoryPageInput('99')
    })
    await waitFor(() => expect(result.current.historyPageInput).toBe('99'))
    act(() => {
      result.current.handleHistoryPageJump(4, 10)
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('page')).toBe('10'),
    )
    expect(result.current.historyPageInput).toBe('10')
  })

  test('tracks semantic cursor history from an empty cursor trail', async () => {
    const { result } = renderExplorerUrlState(
      '/explorer?mode=semantic&q=vector',
    )

    act(() => {
      result.current.handlePreviousSemanticPage()
    })
    expect(result.current.searchParams.get('semanticCursor')).toBeNull()

    act(() => {
      result.current.handleNextSemanticPage('cursor-1')
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('semanticCursor')).toBe(
        'cursor-1',
      ),
    )
    expect(result.current.semanticTrail).toEqual([''])

    act(() => {
      result.current.handlePreviousSemanticPage()
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('semanticCursor')).toBeNull(),
    )
    expect(result.current.semanticTrail).toEqual([])
  })

  test('covers clearing, defaulting, and no-op pagination branches', async () => {
    const { result } = renderExplorerUrlState(
      '/explorer?q=remove&page=2&cursor=c1&semanticCursor=s1&pageSize=100&view=domain',
    )

    vi.useFakeTimers()
    act(() => {
      result.current.setQueryInput('')
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(181)
    })
    vi.useRealTimers()

    expect(result.current.searchParams.get('q')).toBeNull()
    expect(result.current.searchParams.get('page')).toBeNull()
    expect(result.current.searchParams.get('cursor')).toBeNull()
    expect(result.current.searchParams.get('semanticCursor')).toBeNull()

    act(() => {
      result.current.updateParam('domain', 'example.com')
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('domain')).toBe('example.com'),
    )

    act(() => {
      result.current.updateParam('domain', null, { resetPagination: false })
    })
    await waitFor(() =>
      expect(result.current.searchParams.get('domain')).toBeNull(),
    )

    act(() => {
      result.current.setView('time')
    })
    await waitFor(() => expect(result.current.view).toBe('time'))
    expect(result.current.searchParams.get('view')).toBeNull()

    act(() => {
      result.current.setView('trail')
    })
    await waitFor(() => expect(result.current.view).toBe('trail'))
    expect(result.current.start).not.toBeNull()
    expect(result.current.end).not.toBeNull()

    act(() => {
      result.current.setHistoryPageSize(999)
    })
    await waitFor(() =>
      expect(result.current.pageSize).toBe(defaultKeywordPageSize),
    )
    expect(result.current.searchParams.get('pageSize')).toBeNull()

    act(() => {
      result.current.handleFirstHistoryPage(1)
      result.current.handleLastHistoryPage(10, 10)
    })
    expect(result.current.searchParams.get('page')).toBeNull()

    act(() => {
      result.current.setHistoryPageInput('4')
    })
    await waitFor(() => expect(result.current.historyPageInput).toBe('4'))
    act(() => {
      result.current.handleHistoryPageJump(4, 10)
    })
    expect(result.current.searchParams.get('page')).toBeNull()
    expect(result.current.historyPageInput).toBe('4')

    const recentParams = {
      q: 'sqlite',
      mode: 'keyword' as const,
      view: 'time' as const,
      domain: null,
      profileId: null,
      browserKind: null,
      start: null,
      end: null,
      regex: null,
    }
    act(() => {
      result.current.persistRecentSearch(recentParams)
      result.current.persistRecentSearch(recentParams)
    })
    expect(
      JSON.parse(window.localStorage.getItem(recentSearchesStorageKey) ?? '[]'),
    ).toHaveLength(1)
  })
})
