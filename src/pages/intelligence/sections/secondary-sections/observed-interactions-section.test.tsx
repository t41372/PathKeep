/**
 * @file observed-interactions-section.test.tsx
 * @description Render-level coverage for capability-gated observed interaction signals.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify loading, empty, and populated observed-interaction states.
 * - Protect duration/key/load-failure metric formatting.
 *
 * ## Not responsible for
 * - Re-testing backend interaction collection.
 * - Re-testing secondary-grid ordering.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` and uses the shipped i18n provider for section meta.
 *
 * ## Performance notes
 * - Keeps capability-signal rendering covered without fetching overview data.
 */

import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../../../lib/core-intelligence'
import { I18nProvider } from '../../../../lib/i18n'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  ObservedInteraction,
} from '../../../../lib/core-intelligence'
import { ObservedInteractionsSection } from './observed-interactions-section'

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

describe('ObservedInteractionsSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading and empty interaction states', () => {
    useAsyncDataMock.mockReturnValue({ data: null, loading: true })
    const { rerender } = renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()

    useAsyncDataMock.mockReturnValue({
      data: interactionResult([]),
      loading: false,
    })
    rerender(sectionNode())
    expect(screen.getByText('observedEmpty')).toBeVisible()
  })

  test('renders bounded interaction metrics and fallback titles', () => {
    useAsyncDataMock.mockReturnValue({
      data: interactionResult([
        interactionFixture(1, {
          title: 'Reading PathKeep docs',
          foregroundDurationMs: 1500,
          scrollingTimeMs: 90_000,
          keyPresses: 12,
          loadSuccessful: false,
        }),
        interactionFixture(2, {
          title: null,
          url: 'https://example.com/fallback-title',
          foregroundDurationMs: null,
          scrollingTimeMs: null,
          keyPresses: 0,
          loadSuccessful: true,
        }),
      ]),
      loading: false,
    })

    renderSection()

    expect(screen.getByText('Reading PathKeep docs')).toBeVisible()
    expect(screen.getByText('https://example.com/fallback-title')).toBeVisible()
    expect(
      screen.getByText('observedForeground:{"duration":"1.5s"}'),
    ).toBeVisible()
    expect(screen.getByText('observedScroll:{"duration":"1.5m"}')).toBeVisible()
    expect(screen.getByText('observedKeyPresses:{"count":12}')).toBeVisible()
    expect(screen.getByText('observedLoadFailed')).toBeVisible()
  })
})

function renderSection() {
  return render(sectionNode())
}

function sectionNode() {
  return (
    <I18nProvider>
      <ObservedInteractionsSection
        dateRange={dateRange}
        profileId={null}
        scopeLabel="All profiles"
        t={t}
      />
    </I18nProvider>
  )
}

function interactionResult(
  data: ObservedInteraction[],
  state: CoreIntelligenceSectionMeta['state'] = 'ready',
): CoreIntelligenceSectionResult<ObservedInteraction[]> {
  return {
    data,
    meta: {
      sectionId: 'observed-interactions',
      generatedAt: '2026-04-25T12:00:00.000Z',
      window: {
        kind: 'date-range',
        dateRange,
      },
      moduleIds: ['observed_interactions'],
      sourceTables: ['visits'],
      includesEnrichment: false,
      state,
      stateReason: null,
      notes: [],
    },
  }
}

function interactionFixture(
  visitId: number,
  overrides: Partial<ObservedInteraction> = {},
): ObservedInteraction {
  return {
    visitId,
    url: `https://example.com/${visitId}`,
    title: `Example ${visitId}`,
    browserFamily: 'chrome',
    foregroundDurationMs: 1000,
    scrollingTimeMs: 2000,
    scrollingDistance: 300,
    keyPresses: 1,
    typingTimeMs: 400,
    loadSuccessful: true,
    pageEndReason: null,
    ...overrides,
  }
}
