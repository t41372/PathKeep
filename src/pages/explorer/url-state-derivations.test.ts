/**
 * @file url-state-derivations.test.ts
 * @description Locks the Explorer URL-state derivation owner after splitting pure parsing and summary logic away from the Router hook.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Verify typed param normalization for Explorer URL state.
 * - Verify deterministic query/signature builders and active-filter summaries.
 * - Verify grouped date-window and active-shortcut derivation without mounting the route.
 *
 * ## Not responsible for
 * - Re-testing React Router integration or debounced query commits.
 * - Rendering Explorer panels or pagination chrome.
 *
 * ## Dependencies
 * - Depends on the Explorer i18n namespace for human-readable label assertions.
 *
 * ## Performance notes
 * - Pure unit tests keep URL-state coverage cheap while protecting hot-path derivations used during typing and pagination.
 */

import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import {
  buildExplorerActiveFilters,
  buildExplorerBrowserKinds,
  buildExplorerHistoryQuery,
  buildExplorerHistoryQuerySignature,
  buildExplorerRecentSearchLabel,
  buildExplorerSemanticQuery,
  buildExplorerSemanticQuerySignature,
  deriveExplorerUrlParamState,
  resolveExplorerActiveDateShortcut,
  resolveExplorerGroupedDateRange,
} from './url-state-derivations'

const explorerT = createNamespaceTranslator('en', 'explorer')

describe('explorer url-state derivations', () => {
  test('normalizes raw query params into a typed explorer url state', () => {
    const params = new URLSearchParams({
      q: 'sqlite',
      regex: '1',
      mode: 'semantic',
      view: 'session',
      browserKind: 'chrome',
      domain: 'example.com',
      start: '2026-04-01',
      end: '2026-04-20',
      sort: 'oldest',
      page: '0',
      pageSize: '100',
      cursor: 'cursor-1',
      semanticCursor: 'semantic-2',
    })

    expect(deriveExplorerUrlParamState(params, 'chrome:Default')).toEqual({
      browserKind: 'chrome',
      cursor: 'cursor-1',
      domain: 'example.com',
      end: '2026-04-20',
      explicitPage: null,
      explicitProfileId: null,
      mode: 'semantic',
      pageSize: 100,
      profileId: 'chrome:Default',
      rawQuery: 'sqlite',
      regexMode: true,
      semanticCursor: 'semantic-2',
      sort: 'oldest',
      start: '2026-04-01',
      view: 'time',
    })
  })

  test('synthesizes start/end from `date` when no explicit range is set', () => {
    const state = deriveExplorerUrlParamState(
      new URLSearchParams('date=2026-05-17'),
      null,
    )
    expect(state.start).toBe('2026-05-17')
    expect(state.end).toBe('2026-05-17')
  })

  test('explicit start/end win over the `date` shorthand', () => {
    const state = deriveExplorerUrlParamState(
      new URLSearchParams('date=2026-05-17&start=2026-04-01&end=2026-04-30'),
      null,
    )
    expect(state.start).toBe('2026-04-01')
    expect(state.end).toBe('2026-04-30')
  })

  test('keeps keyword grouped views and positive explicit pages', () => {
    const state = deriveExplorerUrlParamState(
      new URLSearchParams('view=trail&page=2'),
      null,
    )
    expect(state.view).toBe('trail')
    expect(state.explicitPage).toBe(2)
  })

  test('defaults keyword searches to relevance when sort is not explicit', () => {
    expect(
      deriveExplorerUrlParamState(new URLSearchParams('q=github'), null).sort,
    ).toBe('relevance')
    expect(
      deriveExplorerUrlParamState(
        new URLSearchParams('q=%22release+notes%22'),
        null,
      ).sort,
    ).toBe('relevance')
    expect(deriveExplorerUrlParamState(new URLSearchParams(), null).sort).toBe(
      'newest',
    )
    expect(
      deriveExplorerUrlParamState(
        new URLSearchParams('q=site%3Agithub.com+-pathkeep'),
        null,
      ).sort,
    ).toBe('newest')
    expect(
      deriveExplorerUrlParamState(new URLSearchParams('q=OR'), null).sort,
    ).toBe('newest')
    expect(
      deriveExplorerUrlParamState(
        new URLSearchParams('q=site%3A%22github.com%22'),
        null,
      ).sort,
    ).toBe('newest')
    expect(
      deriveExplorerUrlParamState(new URLSearchParams('q=github&regex=1'), null)
        .sort,
    ).toBe('newest')
  })

  test('builds canonical history and semantic query payloads plus stable signatures', () => {
    expect(
      buildExplorerHistoryQuery({
        browserKind: 'chrome',
        cursor: 'ignored-cursor',
        deferredQuery: 'sqlite',
        domain: 'example.com',
        end: '2026-04-20',
        explicitPage: 3,
        pageSize: 50,
        profileId: 'chrome:Default',
        regexMode: true,
        sort: 'newest',
        start: '2026-04-01',
      }),
    ).toEqual(
      expect.objectContaining({
        q: 'sqlite',
        browserKind: 'chrome',
        cursor: null,
        domain: 'example.com',
        page: 3,
        limit: 50,
        profileId: 'chrome:Default',
        regexMode: true,
        sort: 'newest',
      }),
    )

    expect(
      buildExplorerHistoryQuery({
        browserKind: null,
        cursor: 'cursor-1',
        deferredQuery: '',
        domain: null,
        end: null,
        explicitPage: null,
        pageSize: 50,
        profileId: null,
        regexMode: false,
        sort: 'newest',
        start: null,
      }),
    ).toEqual({
      q: null,
      profileId: null,
      browserKind: null,
      domain: null,
      startTimeMs: null,
      endTimeMs: null,
      sort: 'newest',
      limit: 50,
      page: null,
      cursor: 'cursor-1',
      regexMode: false,
    })

    expect(
      buildExplorerSemanticQuery({
        deferredQuery: '  local recall  ',
        domain: 'example.com',
        profileId: 'chrome:Default',
        semanticCursor: 'semantic-1',
      }),
    ).toEqual({
      query: 'local recall',
      profileId: 'chrome:Default',
      domain: 'example.com',
      limit: 8,
      cursor: 'semantic-1',
    })

    expect(
      buildExplorerHistoryQuerySignature({
        browserKind: 'chrome',
        domain: 'example.com',
        end: '2026-04-20',
        mode: 'keyword',
        pageSize: 50,
        profileId: 'chrome:Default',
        rawQuery: 'sqlite',
        regexMode: true,
        sort: 'newest',
        start: '2026-04-01',
        view: 'time',
      }),
    ).toContain('"q":"sqlite"')

    expect(
      buildExplorerHistoryQuerySignature({
        browserKind: null,
        domain: null,
        end: null,
        mode: 'keyword',
        pageSize: 50,
        profileId: null,
        rawQuery: '',
        regexMode: false,
        sort: 'newest',
        start: null,
        view: 'time',
      }),
    ).toContain('"q":null')

    expect(
      buildExplorerSemanticQuerySignature({
        deferredQuery: '  local recall  ',
        domain: 'example.com',
        mode: 'hybrid',
        profileId: 'chrome:Default',
      }),
    ).toBe(
      JSON.stringify({
        query: 'local recall',
        profileId: 'chrome:Default',
        domain: 'example.com',
        mode: 'hybrid',
      }),
    )
  })

  test('builds browser kinds and active filter chips from query params', () => {
    const params = new URLSearchParams({
      q: 'sqlite',
      mode: 'hybrid',
      view: 'trail',
      regex: '1',
      domain: 'example.com',
      profileId: 'chrome:Default',
      browserKind: 'chrome',
      start: '2026-04-01',
      end: '2026-04-20',
    })

    expect(
      buildExplorerBrowserKinds([
        'chrome:Default',
        'chrome:Profile 2',
        'safari:History',
      ]),
    ).toEqual(['chrome', 'safari'])

    expect(
      buildExplorerActiveFilters({
        browserLabelForKind: (kind) => (kind === 'chrome' ? 'Chrome' : kind),
        end: '2026-04-20',
        explorerT,
        mode: 'hybrid',
        profileLabelForId: (profileId) =>
          profileId === 'chrome:Default' ? 'Default' : profileId,
        regexMode: true,
        searchParams: params,
        start: '2026-04-01',
        view: 'trail',
      }),
    ).toEqual([
      { id: 'q', label: 'KEYWORD', value: 'sqlite' },
      // L-2: the unified tab is "Smart search", so the mode chip says "Smart"
      // for both `hybrid` and the legacy `semantic` alias.
      { id: 'mode', label: 'MODE', value: 'Smart' },
      { id: 'view', label: 'View by', value: 'Search Trail' },
      { id: 'regex', label: 'REGEX', value: 'Enabled' },
      { id: 'domain', label: 'DOMAIN', value: 'example.com' },
      { id: 'profileId', label: 'PROFILE', value: 'Default' },
      { id: 'browserKind', label: 'BROWSER', value: 'Chrome' },
      { id: 'start', label: 'START', value: '2026-04-01' },
      { id: 'end', label: 'END', value: '2026-04-20' },
    ])

    expect(
      buildExplorerActiveFilters({
        end: null,
        explorerT,
        mode: 'keyword',
        regexMode: false,
        searchParams: new URLSearchParams({
          profileId: 'chrome:Default',
          browserKind: 'chrome',
        }),
        start: null,
        view: 'time',
      }),
    ).toEqual([
      { id: 'profileId', label: 'PROFILE', value: 'chrome:Default' },
      { id: 'browserKind', label: 'BROWSER', value: 'chrome' },
    ])

    expect(
      buildExplorerActiveFilters({
        end: null,
        explorerT,
        mode: 'semantic',
        regexMode: false,
        searchParams: new URLSearchParams(),
        start: null,
        view: 'session',
      }),
    ).toEqual([
      // L-2: the legacy `semantic` alias also surfaces as "Smart" now.
      { id: 'mode', label: 'MODE', value: 'Smart' },
      { id: 'view', label: 'View by', value: 'Session' },
    ])
  })

  test('builds recent-search labels from display-ready params', () => {
    const label = buildExplorerRecentSearchLabel({
      explorerT,
      formatRecentDate: (value) =>
        value === '2026-04-01'
          ? 'Apr 1'
          : value === '2026-04-20'
            ? 'Apr 20'
            : null,
      params: {
        mode: 'semantic',
        view: 'session',
        regex: '1',
        q: 'sqlite',
        domain: 'example.com',
        profileId: 'Default Profile',
        browserKind: 'Chrome',
        start: '2026-04-01',
        end: '2026-04-20',
      },
    })

    expect(label).toBe(
      // L-2: recent-search chips share the unified "Smart" vocabulary.
      'Smart · Session · Enabled · sqlite · example.com · Default Profile · Chrome · Apr 1 - Apr 20',
    )

    const fallbackDateLabel = buildExplorerRecentSearchLabel({
      explorerT,
      formatRecentDate: () => null,
      params: {
        mode: 'hybrid',
        view: 'trail',
        regex: null,
        q: '   ',
        domain: '   ',
        profileId: null,
        browserKind: null,
        start: '2026-04-01',
        end: '2026-04-20',
      },
    })
    expect(fallbackDateLabel).toBe('Smart · Search Trail · All time - All time')

    const bareKeywordLabel = buildExplorerRecentSearchLabel({
      explorerT,
      formatRecentDate: () => {
        throw new Error('date formatter should not run without a date range')
      },
      params: {
        mode: 'keyword',
        view: 'time',
        regex: null,
        q: '  sqlite  ',
        domain: '  example.org  ',
        profileId: null,
        browserKind: null,
        start: null,
        end: null,
      },
    })
    expect(bareKeywordLabel).toBe('sqlite · example.org')
  })

  test('resolves grouped date windows and active shortcuts from calendar dates', () => {
    expect(
      resolveExplorerGroupedDateRange(
        '2026-04-01',
        '2026-04-20',
        new Date('2026-04-20T12:00:00Z'),
      ),
    ).toEqual({
      start: '2026-04-01',
      end: '2026-04-20',
    })

    expect(
      resolveExplorerGroupedDateRange(
        null,
        null,
        new Date('2026-04-20T12:00:00Z'),
      ),
    ).toEqual({
      start: '2026-03-22',
      end: '2026-04-20',
    })

    expect(
      resolveExplorerActiveDateShortcut(
        '2026-04-20',
        '2026-04-20',
        new Date('2026-04-20T12:00:00Z'),
      ),
    ).toBe('day')

    expect(
      resolveExplorerActiveDateShortcut(
        null,
        '2026-04-20',
        new Date('2026-04-20T12:00:00Z'),
      ),
    ).toBeNull()

    expect(
      resolveExplorerActiveDateShortcut(
        '2026-04-20',
        null,
        new Date('2026-04-20T12:00:00Z'),
      ),
    ).toBeNull()

    expect(
      resolveExplorerActiveDateShortcut(
        '2026-04-01',
        '2026-04-19',
        new Date('2026-04-20T12:00:00Z'),
      ),
    ).toBeNull()

    expect(
      resolveExplorerActiveDateShortcut(
        '2026-04-18',
        '2026-04-20',
        new Date('2026-04-20T12:00:00Z'),
      ),
    ).toBeNull()
  })
})
