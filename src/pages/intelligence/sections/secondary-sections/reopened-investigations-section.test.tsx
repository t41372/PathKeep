/**
 * @file reopened-investigations-section.test.tsx
 * @description Render-level coverage for the Reopened Investigations secondary card.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify loading, degraded empty, hidden low-signal, and populated card states.
 * - Protect the shared query-family deep-link used by reopened investigation cards.
 *
 * ## Not responsible for
 * - Re-testing the backend reopened-investigation detector.
 * - Re-testing the heuristic predicates in detail.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` so section rendering is deterministic.
 * - Uses `MemoryRouter` because visible cards render route links.
 *
 * ## Performance notes
 * - Pure render tests keep the bounded secondary-card contract cheap under strict coverage.
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
  ReopenedInvestigation,
} from '../../../../lib/core-intelligence'
import { ReopenedInvestigationsSection } from './reopened-investigations-section'

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

describe('ReopenedInvestigationsSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading, degraded empty, and hides ready low-signal payloads', () => {
    useAsyncDataMock.mockReturnValue({ data: null, loading: true })
    const { container, rerender } = renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()

    useAsyncDataMock.mockReturnValue({
      data: reopenedResult([], 'degraded'),
      loading: false,
    })
    rerender(sectionNode())
    expect(screen.getByText('reopenedEmpty')).toBeVisible()

    useAsyncDataMock.mockReturnValue({
      data: reopenedResult([
        reopenedFixture({
          anchorLabel: 'https://accounts.example/login',
          occurrenceCount: 1,
        }),
      ]),
      loading: false,
    })
    rerender(sectionNode())
    expect(container.firstChild).toBeNull()
  })

  test('renders search-backed reopened investigations with shared deep links', () => {
    useAsyncDataMock.mockReturnValue({
      data: reopenedResult([reopenedFixture()]),
      loading: false,
    })

    renderSection()

    expect(screen.getByText('reopenedTitle')).toBeVisible()
    expect(screen.getByText('reopenedAnchorQuery')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'why sqlite wal' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/query-family/family-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
    )
    expect(screen.getByText('reopenedOccurrences:{"count":3}')).toBeVisible()
    expect(screen.getByText('reopenedDistinctDays:{"days":2}')).toBeVisible()
    expect(screen.getByText('explainTitle')).toBeVisible()
  })
})

function renderSection() {
  return render(sectionNode())
}

function sectionNode() {
  return (
    <MemoryRouter>
      <I18nProvider>
        <ReopenedInvestigationsSection
          dateRange={dateRange}
          profileId="chrome:Default"
          scopeLabel="Chrome"
          t={t}
        />
      </I18nProvider>
    </MemoryRouter>
  )
}

function reopenedResult(
  data: ReopenedInvestigation[],
  state: CoreIntelligenceSectionMeta['state'] = 'ready',
): CoreIntelligenceSectionResult<ReopenedInvestigation[]> {
  return {
    data,
    meta: {
      sectionId: 'reopened',
      generatedAt: '2026-04-25T12:00:00.000Z',
      window: { kind: 'date-range', dateRange },
      moduleIds: ['reopened'],
      sourceTables: ['visits'],
      includesEnrichment: false,
      state,
      stateReason: null,
      notes: [],
    },
  }
}

function reopenedFixture(
  overrides: Partial<ReopenedInvestigation> = {},
): ReopenedInvestigation {
  return {
    investigationId: 'investigation-1',
    anchorType: 'query_family',
    anchorId: 'family-1',
    anchorLabel: 'why sqlite wal',
    occurrenceCount: 3,
    distinctDays: 2,
    firstSeenAt: '2026-04-01T00:00:00Z',
    lastSeenAt: '2026-04-20T00:00:00Z',
    ...overrides,
  }
}
