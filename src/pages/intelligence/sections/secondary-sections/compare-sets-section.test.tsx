/**
 * @file compare-sets-section.test.tsx
 * @description Render-level coverage for the secondary compare-sets card.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify compare-set loading, empty, and populated card states.
 * - Keep compare-set, trail, and focused-domain route links covered without mounting the full Intelligence page.
 *
 * ## Not responsible for
 * - Re-testing the backend compare-set query.
 * - Re-testing shared compare-set detail routes.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` so the section can be exercised with deterministic payloads.
 * - Uses `MemoryRouter` because card rows render route links.
 *
 * ## Performance notes
 * - Pure render tests avoid asynchronous overview reads while preserving bounded-list behavior.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../../../lib/core-intelligence'
import { I18nProvider } from '../../../../lib/i18n'
import type {
  CompareSet,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
} from '../../../../lib/core-intelligence'
import { CompareSetsSection } from './compare-sets-section'

const { useAsyncDataMock } = vi.hoisted(() => ({
  useAsyncDataMock: vi.fn(),
}))

vi.mock('../../../../lib/core-intelligence', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceModule>()
  return {
    ...actual,
    useAsyncData: useAsyncDataMock,
  }
})

const dateRange: DateRange = { start: '2026-04-01', end: '2026-04-30' }

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('CompareSetsSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading and empty states', () => {
    useAsyncDataMock.mockReturnValue({ data: null, loading: true })
    const { rerender } = renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()

    useAsyncDataMock.mockReturnValue({
      data: compareSetResult([]),
      loading: false,
    })
    rerender(sectionNode())
    expect(screen.getByText('compareSetsEmpty')).toBeVisible()
  })

  test('renders compare-set cards with shared route links', () => {
    useAsyncDataMock.mockReturnValue({
      data: compareSetResult([compareSetFixture()]),
      loading: false,
    })

    renderSection()

    expect(screen.getByText('compareSetsTitle')).toBeVisible()
    expect(screen.getByRole('link', { name: 'rust sqlite' })).toHaveAttribute(
      'href',
      '/compare/cmp-1',
    )
    expect(
      screen.getByRole('link', { name: 'trailRouteTitle' }),
    ).toHaveAttribute('href', '/trail/trail-1')
    expect(screen.getByText('compareSetsPages:{"count":5}')).toBeVisible()
    expect(screen.getByText('compareSetsLanding')).toBeVisible()
    expect(screen.getByRole('link', { name: 'docs.example' })).toHaveAttribute(
      'href',
      '/domain/docs.example?focus=compare-set:cmp-1',
    )
    expect(
      screen.queryByRole('link', { name: 'hidden.example' }),
    ).not.toBeInTheDocument()
  })
})

function renderSection() {
  return render(sectionNode())
}

function sectionNode() {
  return (
    <MemoryRouter>
      <I18nProvider>
        <CompareSetsSection
          compareSetHref={(compareSetId) => `/compare/${compareSetId}`}
          dateRange={dateRange}
          focusedDomainHref={(domain, focus) =>
            `/domain/${domain}?focus=${focus.focusType}:${focus.focusId}`
          }
          profileId={null}
          scopeLabel="All profiles"
          trailHref={(trailId) => `/trail/${trailId}`}
          t={t}
        />
      </I18nProvider>
    </MemoryRouter>
  )
}

function compareSetResult(
  data: CompareSet[],
  state: CoreIntelligenceSectionMeta['state'] = 'ready',
): CoreIntelligenceSectionResult<CompareSet[]> {
  return {
    data,
    meta: {
      sectionId: 'compare-sets',
      generatedAt: '2026-04-25T12:00:00.000Z',
      window: { kind: 'date-range', dateRange },
      moduleIds: ['compare_sets'],
      sourceTables: ['visits'],
      includesEnrichment: false,
      state,
      stateReason: null,
      notes: [],
    },
  }
}

function compareSetFixture(): CompareSet {
  return {
    compareSetId: 'cmp-1',
    trailId: 'trail-1',
    searchQuery: 'rust sqlite',
    pageCategory: 'learning',
    pages: [
      pageFixture('docs.example', true),
      pageFixture('blog.example'),
      pageFixture('sqlite.example'),
      pageFixture('tauri.example'),
      pageFixture('hidden.example'),
    ],
  }
}

function pageFixture(
  registrableDomain: string,
  isLanding = false,
): CompareSet['pages'][number] {
  return {
    canonicalUrl: `https://${registrableDomain}/canonical`,
    url: `https://${registrableDomain}/`,
    title: `${registrableDomain} title`,
    registrableDomain,
    visitCount: 3,
    isLanding,
  }
}
