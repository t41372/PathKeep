/**
 * @file stable-sources-section.test.tsx
 * @description Render-level coverage for the secondary stable-sources card.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify stable-source visibility, column rows, and empty-column fallbacks.
 * - Keep domain route links covered without mounting the full Intelligence page.
 *
 * ## Not responsible for
 * - Re-testing the backend stable-source query.
 * - Re-testing secondary-grid ordering.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` so the card can be exercised with deterministic payloads.
 *
 * ## Performance notes
 * - Pure render tests avoid asynchronous overview fetches.
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
  StableSource,
} from '../../../../lib/core-intelligence'
import { StableSourcesSection } from './stable-sources-section'

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

const dateRange: DateRange = {
  start: '2026-04-01',
  end: '2026-04-30',
}

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('StableSourcesSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading and degraded empty states', () => {
    useAsyncDataMock.mockReturnValue({ data: null, loading: true })
    const { rerender } = renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()

    useAsyncDataMock.mockReturnValue({
      data: stableSourceResult([], 'degraded'),
      loading: false,
    })
    rerender(sectionNode())
    expect(screen.getByText('stableSourcesEmpty')).toBeVisible()
  })

  test('hides ready low-signal stable-source payloads', () => {
    useAsyncDataMock.mockReturnValue({
      data: stableSourceResult([sourceFixture('entry.example', 'entry')]),
      loading: false,
    })

    const { container } = renderSection()

    expect(container.firstChild).toBeNull()
  })

  test('renders entry and landing columns with domain links', () => {
    useAsyncDataMock.mockReturnValue({
      data: stableSourceResult([
        sourceFixture('entry.example', 'entry', {
          displayName: 'Entry Example',
          trailCount: 7,
        }),
        sourceFixture('landing.example', 'landing', {
          displayName: null,
          stableLandingCount: 4,
        }),
      ]),
      loading: false,
    })

    renderSection()

    expect(screen.getByText('Entry Example')).toBeVisible()
    expect(screen.getByText('landing.example')).toBeVisible()
    expect(
      screen.getByText('stableSourcesEntryCount:{"count":7}'),
    ).toBeVisible()
    expect(
      screen.getByText('stableSourcesLandingCount:{"count":4}'),
    ).toBeVisible()
    expect(screen.getByRole('link', { name: /Entry Example/ })).toHaveAttribute(
      'href',
      '/intelligence/domain/entry.example',
    )
  })

  test('keeps degraded one-sided evidence readable with empty column copy', () => {
    useAsyncDataMock.mockReturnValue({
      data: stableSourceResult(
        [sourceFixture('entry-only.example', 'entry')],
        'degraded',
      ),
      loading: false,
    })

    renderSection()

    expect(screen.getByText('entry-only.example')).toBeVisible()
    expect(screen.getByText('stableSourcesNoLanding')).toBeVisible()
  })
})

function renderSection() {
  return render(sectionNode())
}

function sectionNode() {
  return (
    <MemoryRouter>
      <I18nProvider>
        <StableSourcesSection
          dateRange={dateRange}
          domainHref={(domain) => `/intelligence/domain/${domain}`}
          profileId={null}
          scopeLabel="All profiles"
          t={t}
        />
      </I18nProvider>
    </MemoryRouter>
  )
}

function stableSourceResult(
  data: StableSource[],
  state: CoreIntelligenceSectionMeta['state'] = 'ready',
): CoreIntelligenceSectionResult<StableSource[]> {
  return {
    data,
    meta: {
      sectionId: 'stable-sources',
      generatedAt: '2026-04-25T12:00:00.000Z',
      window: {
        kind: 'date-range',
        dateRange,
      },
      moduleIds: ['stable_sources'],
      sourceTables: ['visits'],
      includesEnrichment: false,
      state,
      stateReason: null,
      notes: [],
    },
  }
}

function sourceFixture(
  registrableDomain: string,
  sourceRole: StableSource['sourceRole'],
  overrides: Partial<StableSource> = {},
): StableSource {
  return {
    registrableDomain,
    displayName: registrableDomain,
    sourceRole,
    trailCount: 3,
    stableLandingCount: 2,
    effectivenessScore: 0.7,
    ...overrides,
  }
}
