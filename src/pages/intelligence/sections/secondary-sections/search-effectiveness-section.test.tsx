/**
 * @file search-effectiveness-section.test.tsx
 * @description Render-level coverage for the search-effectiveness secondary card.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify loading, hidden-empty, empty-degraded, and populated search-effectiveness branches.
 * - Keep engine/source/topic route links covered without mounting the full Intelligence page.
 *
 * ## Not responsible for
 * - Re-testing backend search-effectiveness scoring.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` because this card owns only bounded section rendering.
 * - Uses MemoryRouter for domain and query-family links.
 *
 * ## Performance notes
 * - Fixtures are tiny and do not walk raw archive rows.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../../../lib/core-intelligence'
import { I18nProvider } from '../../../../lib/i18n'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  SearchEffectiveness,
} from '../../../../lib/core-intelligence'
import { SearchEffectivenessSection } from './search-effectiveness-section'

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

describe('SearchEffectivenessSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading, hidden-ready-empty, and degraded-empty states', () => {
    setSearchEffectiveness(null, { loading: true })
    const { container, rerender } = renderSection()

    expect(
      container.querySelector('.intelligence-skeleton'),
    ).toBeInTheDocument()

    setSearchEffectiveness(emptyEffectiveness(), { state: 'ready' })
    rerender(sectionNode())
    expect(container).toBeEmptyDOMElement()

    setSearchEffectiveness(null, { state: 'degraded' })
    rerender(sectionNode())
    expect(screen.getByText('searchEffectivenessEmpty')).toBeVisible()
  })

  test('renders engines, resolving sources, and hard topics with fallback labels', () => {
    setSearchEffectiveness({
      engineStats: [
        {
          searchEngine: 'plain',
          displayName: null,
          avgReformulations: 1.25,
          avgDepth: 2.5,
          totalTrails: 7,
        },
      ],
      topResolvingSources: [
        {
          registrableDomain: 'landing.example',
          displayName: null,
          sourceRole: 'landing',
          trailCount: 3,
          stableLandingCount: 2,
          effectivenessScore: 0.8,
        },
        {
          registrableDomain: 'entry.example',
          displayName: 'Entry Source',
          sourceRole: 'entry',
          trailCount: 4,
          stableLandingCount: 0,
          effectivenessScore: 0.7,
        },
      ],
      hardestTopics: [
        {
          familyId: 'family-hard',
          queryFamily: 'sqlite checkpoint',
          reformulationCount: 5,
          reSearchLagDays: 3.25,
        },
      ],
    })

    renderSection()

    expect(screen.getByText('plain')).toBeVisible()
    expect(
      screen.getByText('searchEffectivenessEngineRewrites:{"count":"1.3"}'),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: /landing\.example/ }),
    ).toHaveAttribute('href', '/domain/landing.example')
    expect(screen.getByRole('link', { name: /Entry Source/ })).toHaveAttribute(
      'href',
      '/domain/entry.example',
    )
    expect(
      screen.getByText('stableSourcesLandingCount:{"count":2}'),
    ).toBeVisible()
    expect(
      screen.getByText('stableSourcesEntryCount:{"count":4}'),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: '"sqlite checkpoint"' }),
    ).toHaveAttribute('href', '/query-family/family-hard')
    expect(
      screen.getByText('searchEffectivenessLag:{"days":"3.3"}'),
    ).toBeVisible()
  })
})

function renderSection() {
  return render(sectionNode())
}

function sectionNode() {
  return (
    <I18nProvider>
      <MemoryRouter>
        <SearchEffectivenessSection
          dateRange={dateRange}
          domainHref={(domain) => `/domain/${domain}`}
          profileId={null}
          queryFamilyHref={(familyId) => `/query-family/${familyId}`}
          scopeLabel="All profiles"
          t={t}
        />
      </MemoryRouter>
    </I18nProvider>
  )
}

function setSearchEffectiveness(
  data: SearchEffectiveness | null,
  {
    loading = false,
    state = 'ready',
  }: {
    loading?: boolean
    state?: CoreIntelligenceSectionMeta['state']
  } = {},
) {
  useAsyncDataMock.mockReturnValue({
    data: data === null ? null : sectionResult(data, state),
    loading,
  })
}

function sectionResult(
  data: SearchEffectiveness,
  state: CoreIntelligenceSectionMeta['state'] = 'ready',
): CoreIntelligenceSectionResult<SearchEffectiveness> {
  return {
    data,
    meta: {
      sectionId: 'search-effectiveness',
      generatedAt: '2026-04-25T12:00:00.000Z',
      window: { kind: 'date-range', dateRange },
      moduleIds: ['search-trails'],
      sourceTables: ['search_trails'],
      includesEnrichment: false,
      state,
      stateReason: null,
      notes: [],
    },
  }
}

function emptyEffectiveness(): SearchEffectiveness {
  return {
    engineStats: [],
    topResolvingSources: [],
    hardestTopics: [],
  }
}
