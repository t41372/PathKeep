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
      { id: 'mode', label: 'MODE', value: 'Hybrid' },
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
      'Semantic · Session · Enabled · sqlite · example.com · Default Profile · Chrome · Apr 1 - Apr 20',
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
    expect(fallbackDateLabel).toBe(
      'Hybrid · Search Trail · All time - All time',
    )
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
