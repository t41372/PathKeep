/**
 * @file breadth-index-section.test.tsx
 * @description Render-level coverage for the secondary breadth-index card.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify loading, empty, metadata, and bounded score rendering.
 * - Keep the breadth card covered without mounting the full Intelligence route.
 *
 * ## Not responsible for
 * - Re-testing backend breadth-index calculations.
 * - Re-testing shared section metadata rendering.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` so the card can be exercised deterministically.
 *
 * ## Performance notes
 * - Uses tiny fixtures and never touches backend IPC.
 */

import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../../../lib/core-intelligence'
import type {
  BreadthIndex,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
} from '../../../../lib/core-intelligence'
import { BreadthIndexSection } from './breadth-index-section'

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

vi.mock('../../../../components/intelligence/section-meta', () => ({
  IntelligenceSectionMeta: ({ scopeLabel }: { scopeLabel: string }) => (
    <span>{scopeLabel}</span>
  ),
}))

const dateRange: DateRange = { start: '2026-04-01', end: '2026-04-30' }

describe('BreadthIndexSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading and empty breadth states', () => {
    useAsyncDataMock.mockReturnValue({ data: null, loading: true })
    const { rerender } = renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()

    useAsyncDataMock.mockReturnValue({
      data: breadthResult(null),
      loading: false,
    })
    rerender(sectionNode())

    expect(screen.getByText('breadthEmpty')).toBeVisible()
  })

  test('renders breadth metrics and clamps the visible score', () => {
    useAsyncDataMock.mockReturnValue({
      data: breadthResult({
        breadthScore: 125,
        concentrationDomainCount: 4,
        hhi: 0.42,
      }),
      loading: false,
    })

    renderSection()

    expect(screen.getByText('Archive')).toBeVisible()
    expect(screen.getByText('100')).toBeVisible()
    expect(screen.getByText('4')).toBeVisible()
    expect(screen.getByText('0.420')).toBeVisible()
  })
})

function renderSection() {
  return render(sectionNode())
}

function sectionNode() {
  return (
    <BreadthIndexSection
      dateRange={dateRange}
      profileId={null}
      scopeLabel="Archive"
      t={translate}
    />
  )
}

function breadthResult(
  data: BreadthIndex | null,
): CoreIntelligenceSectionResult<BreadthIndex | null> {
  return {
    data,
    meta: metaFixture(),
  }
}

function metaFixture(): CoreIntelligenceSectionMeta {
  return {
    sectionId: 'breadth-index',
    generatedAt: '2026-04-25T12:00:00Z',
    window: { kind: 'date-range', dateRange },
    moduleIds: ['breadth-index'],
    sourceTables: ['domain_daily_rollups'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function translate(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}
