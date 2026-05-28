/**
 * @file index.test.tsx
 * @description Route-shell coverage for the deterministic Intelligence page.
 * @module pages/intelligence
 *
 * ## Responsibilities
 * - Verify the route owns scope notes, staged skeleton switching, and access-strip navigation.
 * - Protect the href factories passed to section renderers without mounting every heavy section.
 * - Keep top-site suggestions wired to the primary overview cache.
 *
 * ## Not responsible for
 * - Re-testing each Intelligence section body; those have section-local suites.
 * - Re-testing route parser helpers from `lib/core-intelligence/routes`.
 *
 * ## Dependencies
 * - Uses React Router in-memory navigation and mocks shell/intelligence data providers at route boundaries.
 *
 * ## Performance notes
 * - Child sections are mocked so this suite exercises route glue without loading the full dashboard.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, useLocation } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../lib/core-intelligence'
import { IntelligencePage } from './index'

const {
  routeState,
  useIntelligenceRouteStateMock,
  useStagedIntelligenceOverviewMock,
  useShellDataMock,
  peekIntelligencePrimaryOverviewMock,
  sectionsRenderMock,
  setPresetMock,
  setCustomRangeMock,
} = vi.hoisted(() => {
  const routeState = {
    dateRange: { start: '2026-04-01', end: '2026-04-30' },
    effectiveProfileId: null as string | null,
    preset: 'month' as coreIntelligenceModule.TimeRangePreset,
    profileScopeLabel: null as string | null,
  }

  return {
    routeState,
    useIntelligenceRouteStateMock: vi.fn(),
    useStagedIntelligenceOverviewMock: vi.fn(),
    useShellDataMock: vi.fn(),
    peekIntelligencePrimaryOverviewMock: vi.fn(),
    sectionsRenderMock: vi.fn(),
    setPresetMock: vi.fn(),
    setCustomRangeMock: vi.fn(),
  }
})

vi.mock('../../app/shell-data-context', () => ({
  useShellData: useShellDataMock,
}))

vi.mock('../../components/intelligence/time-range-selector', () => ({
  TimeRangeSelector: ({
    onCustomRange,
    onPresetChange,
  }: {
    onCustomRange: (range: { start: string; end: string }) => void
    onPresetChange: (preset: string) => void
  }) => (
    <div data-testid="time-range-selector">
      <button type="button" onClick={() => onPresetChange('week')}>
        preset-week
      </button>
      <button type="button" onClick={() => onPresetChange('all')}>
        preset-all
      </button>
      <button
        type="button"
        onClick={() =>
          onCustomRange({ start: '2026-04-10', end: '2026-04-11' })
        }
      >
        custom-range
      </button>
    </div>
  ),
}))

vi.mock('../../lib/core-intelligence', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceModule>()

  return {
    ...actual,
    peekIntelligencePrimaryOverview: peekIntelligencePrimaryOverviewMock,
  }
})

vi.mock('../../lib/i18n/hooks', () => ({
  useI18n: () => ({
    language: 'en',
    t: (key: string, vars?: Record<string, string | number>) => {
      if (key === 'scopedViewBody') {
        return `Scoped to ${vars?.profile}`
      }
      if (key === 'archiveWideBadge') {
        return 'Archive-wide'
      }
      return key
    },
    ns: () => (key: string) => key,
  }),
}))

vi.mock('./route-state', () => ({
  useIntelligenceRouteState: useIntelligenceRouteStateMock,
}))

vi.mock('./runtime-digest', () => ({
  IntelligenceRuntimeDigest: ({
    initialized,
    unlocked,
  }: {
    initialized: boolean
    unlocked: boolean
  }) => (
    <div data-testid="runtime-digest">
      {initialized ? 'initialized' : 'not-initialized'} /
      {unlocked ? 'unlocked' : 'locked'}
    </div>
  ),
}))

vi.mock('./sections', () => ({
  IntelligenceSections: (props: {
    compareSetHref: (compareSetId: string) => string
    dayHref: (date: string) => string
    domainHref: (domain: string) => string
    focusedDomainHref: (
      domain: string,
      focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
    ) => string
    queryFamilyHref: (
      familyId: string,
      profileIdOverride?: string | null,
    ) => string
    refindHref: (canonicalUrl: string) => string
    scopeLabel: string
    secondaryReady: boolean
    trailHref: (trailId: string, profileIdOverride?: string | null) => string
  }) => {
    sectionsRenderMock(props)

    return (
      <div data-testid="intelligence-sections">
        <span data-testid="section-scope">{props.scopeLabel}</span>
        <span data-testid="section-secondary">
          {props.secondaryReady ? 'secondary-ready' : 'secondary-pending'}
        </span>
        <a data-testid="domain-href" href={props.domainHref('example.com')}>
          domain
        </a>
        <a
          data-testid="focused-domain-href"
          href={props.focusedDomainHref('example.com', {
            focusType: 'compare-set',
            focusId: 'compare-1',
          })}
        >
          focused-domain
        </a>
        <a
          data-testid="query-family-href"
          href={props.queryFamilyHref('family-1')}
        >
          query-family
        </a>
        <a
          data-testid="query-family-override-href"
          href={props.queryFamilyHref('family-2', 'safari:Work')}
        >
          query-family-override
        </a>
        <a
          data-testid="refind-href"
          href={props.refindHref('https://example.com/article')}
        >
          refind
        </a>
        <a data-testid="trail-href" href={props.trailHref('trail-1')}>
          trail
        </a>
        <a
          data-testid="trail-override-href"
          href={props.trailHref('trail-2', 'safari:Work')}
        >
          trail-override
        </a>
        <a
          data-testid="compare-set-href"
          href={props.compareSetHref('compare-1')}
        >
          compare-set
        </a>
        <a data-testid="day-href" href={props.dayHref('2026-04-20')}>
          day
        </a>
      </div>
    )
  },
  IntelligenceSectionsSkeleton: () => (
    <div data-testid="intelligence-sections-skeleton" />
  ),
}))

vi.mock('./use-staged-intelligence-overview', () => ({
  useStagedIntelligenceOverview: useStagedIntelligenceOverviewMock,
}))

vi.mock('./paper-intelligence-panel', () => ({
  PaperIntelligencePanel: (props: {
    onSelectDomain: (domain: string) => void
  }) => (
    <div data-testid="paper-intelligence-panel-mock">
      <button
        type="button"
        data-testid="paper-intel-select"
        onClick={() => props.onSelectDomain('example.com')}
      >
        select-domain
      </button>
    </div>
  ),
}))

describe('IntelligencePage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    routeState.dateRange = { start: '2026-04-01', end: '2026-04-30' }
    routeState.effectiveProfileId = null
    routeState.preset = 'month'
    routeState.profileScopeLabel = null
    useIntelligenceRouteStateMock.mockImplementation(() => ({
      ...routeState,
      setCustomRange: setCustomRangeMock,
      setPreset: setPresetMock,
    }))
    useStagedIntelligenceOverviewMock.mockReturnValue({
      scopeKey: '2026-04-01:2026-04-30:archive-wide',
      primaryReady: false,
      primaryError: null,
      secondaryReady: false,
      secondaryLoading: false,
      secondaryError: null,
    })
    useShellDataMock.mockReturnValue({
      dashboard: { totalVisits: 12 },
      snapshot: {
        archiveStatus: { unlocked: true },
        config: { initialized: true },
      },
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue(null)
  })

  test('renders skeleton state and access-strip navigation for archive-wide scope', async () => {
    const user = userEvent.setup()
    const { container } = renderIntelligencePage()

    expect(screen.getByTestId('intelligence-sections-skeleton')).toBeVisible()
    expect(screen.queryByText(/Scoped to/)).not.toBeInTheDocument()
    expect(screen.getByTestId('runtime-digest')).toHaveTextContent(
      'initialized /unlocked',
    )

    await user.click(screen.getByRole('button', { name: 'preset-week' }))
    expect(setPresetMock).toHaveBeenCalledWith('week')
    await user.click(screen.getByRole('button', { name: 'preset-all' }))
    expect(setPresetMock).toHaveBeenCalledWith('all')
    await user.click(screen.getByRole('button', { name: 'custom-range' }))
    expect(setCustomRangeMock).toHaveBeenCalledWith({
      start: '2026-04-10',
      end: '2026-04-11',
    })

    const dayInput = screen.getByLabelText('insightAccessDayLabel')
    fireEvent.change(dayInput, { target: { value: '2026-04-18' } })
    await user.click(screen.getByRole('button', { name: 'openDayInsights' }))
    expect(screen.getByTestId('location')).toHaveTextContent(
      '/intelligence/day/2026-04-18',
    )

    const domainInput = screen.getByLabelText('insightAccessDomainLabel')
    fireEvent.change(domainInput, { target: { value: ' example.com ' } })
    await user.click(screen.getByRole('button', { name: 'openDomainInsights' }))
    expect(screen.getByTestId('location')).toHaveTextContent(
      '/intelligence/domain/example.com?range=month',
    )
    expect(
      container.querySelector('#intelligence-domain-suggestions option'),
    ).toBeNull()
  })

  test('renders ready sections with scoped href factories and cached suggestions', () => {
    routeState.effectiveProfileId = 'chrome:Default'
    routeState.preset = 'all'
    useStagedIntelligenceOverviewMock.mockReturnValue({
      scopeKey: '2026-04-01:2026-04-30:chrome:Default',
      primaryReady: true,
      primaryError: null,
      secondaryReady: true,
      secondaryLoading: false,
      secondaryError: null,
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue({
      topSites: {
        data: [
          { registrableDomain: 'example.com', displayName: 'Example' },
          { registrableDomain: 'fallback.test', displayName: null },
        ],
      },
    })

    const { container } = renderIntelligencePage()

    expect(screen.getByText('Scoped to chrome:Default')).toBeVisible()
    expect(screen.getByTestId('section-scope')).toHaveTextContent(
      'chrome:Default',
    )
    expect(screen.getByTestId('section-secondary')).toHaveTextContent(
      'secondary-ready',
    )
    expect(sectionsRenderMock).toHaveBeenCalledWith(
      expect.objectContaining({
        dashboard: { totalVisits: 12 },
        profileId: 'chrome:Default',
      }),
    )
    expect(screen.getByTestId('domain-href')).toHaveAttribute(
      'href',
      '/intelligence/domain/example.com?range=all&profileId=chrome%3ADefault',
    )
    expect(screen.getByTestId('focused-domain-href')).toHaveAttribute(
      'href',
      '/intelligence/domain/example.com?range=all&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare-1',
    )
    expect(screen.getByTestId('query-family-href')).toHaveAttribute(
      'href',
      '/intelligence/query-family/family-1?range=all&profileId=chrome%3ADefault',
    )
    expect(screen.getByTestId('query-family-override-href')).toHaveAttribute(
      'href',
      '/intelligence/query-family/family-2?range=all&profileId=safari%3AWork',
    )
    expect(screen.getByTestId('refind-href')).toHaveAttribute(
      'href',
      '/intelligence/refind/https%3A%2F%2Fexample.com%2Farticle?range=all&profileId=chrome%3ADefault',
    )
    expect(screen.getByTestId('trail-href')).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=all&profileId=chrome%3ADefault',
    )
    expect(screen.getByTestId('trail-override-href')).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-2?range=all&profileId=safari%3AWork',
    )
    expect(screen.getByTestId('compare-set-href')).toHaveAttribute(
      'href',
      '/intelligence/compare-set/compare-1?range=all&profileId=chrome%3ADefault',
    )
    expect(screen.getByTestId('day-href')).toHaveAttribute(
      'href',
      '/intelligence/day/2026-04-20?profileId=chrome%3ADefault',
    )
    expect(
      container.querySelector(
        '#intelligence-domain-suggestions option[value="example.com"]',
      ),
    ).toHaveTextContent('Example')
    expect(
      container.querySelector(
        '#intelligence-domain-suggestions option[value="fallback.test"]',
      ),
    ).toHaveTextContent('fallback.test')
  })

  test('paper layout: clicking a domain in PaperIntelligencePanel navigates via scoped domainHref', async () => {
    routeState.effectiveProfileId = 'chrome:Default'
    routeState.preset = 'all'
    useStagedIntelligenceOverviewMock.mockReturnValue({
      scopeKey: '2026-04-01:2026-04-30:chrome:Default',
      primaryReady: true,
      primaryError: null,
      secondaryReady: true,
      secondaryLoading: false,
      secondaryError: null,
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue({
      topSites: { data: [] },
    })

    const user = userEvent.setup()
    render(
      <MemoryRouter initialEntries={['/intelligence?layout=paper']}>
        <IntelligencePage />
        <LocationProbe />
      </MemoryRouter>,
    )

    expect(screen.getByTestId('paper-intelligence-panel-mock')).toBeVisible()
    await user.click(screen.getByTestId('paper-intel-select'))
    expect(screen.getByTestId('location')).toHaveTextContent(
      '/intelligence/domain/example.com',
    )
  })
})

function renderIntelligencePage() {
  return render(
    <MemoryRouter initialEntries={['/intelligence']}>
      <IntelligencePage />
      <LocationProbe />
    </MemoryRouter>,
  )
}

function LocationProbe() {
  const location = useLocation()

  return (
    <div data-testid="location">
      {location.pathname}
      {location.search}
    </div>
  )
}
