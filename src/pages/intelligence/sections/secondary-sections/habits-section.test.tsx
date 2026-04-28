/**
 * @file habits-section.test.tsx
 * @description Render-level coverage for the combined habits secondary card.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify loading, empty, interrupted-habit, and stable-pattern render paths.
 * - Keep domain and explainability route affordances covered without mounting the full Intelligence page.
 *
 * ## Not responsible for
 * - Re-testing habit-pattern backend scoring.
 * - Re-testing secondary-grid ordering.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` because this card reads two deterministic overview sections.
 * - Uses MemoryRouter for domain links.
 *
 * ## Performance notes
 * - Pure render fixtures keep the card bounded and cheap to exercise.
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
  HabitPattern,
  InterruptedHabit,
} from '../../../../lib/core-intelligence'
import { HabitsSection } from './habits-section'

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

describe('HabitsSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading and empty states across both habit feeds', () => {
    setHabitAsyncData({
      patterns: null,
      patternsLoading: true,
      interrupted: [],
    })
    const { rerender } = renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()

    setHabitAsyncData({ patterns: [], interrupted: [] })
    rerender(sectionNode())
    expect(screen.getByText('habitsEmpty')).toBeVisible()
  })

  test('renders interrupted and stable habits with domain links and profile explainability', () => {
    setHabitAsyncData({
      patterns: [habitPatternFixture()],
      interrupted: [interruptedHabitFixture()],
    })

    renderSection({ profileId: 'chrome:Default' })

    expect(screen.getByText('habitsInterruptedTitle')).toBeVisible()
    expect(screen.getByText('habitsPatternsTitle')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Docs Habit' })).toHaveAttribute(
      'href',
      '/domain/docs.example',
    )
    expect(
      screen.getByRole('link', { name: 'Interrupted Habit' }),
    ).toHaveAttribute('href', '/domain/interrupted.example')
    expect(
      screen.getByText('habitPatternSummary:{"interval":"3.5","days":12}'),
    ).toBeVisible()
    expect(
      screen.getByText('habitInterruptedSummary:{"days":21,"expected":"4.0"}'),
    ).toBeVisible()
    expect(screen.getAllByRole('button', { name: /explain/ })).toHaveLength(2)
  })

  test('renders one-sided habit feeds without profile explainability', () => {
    setHabitAsyncData({
      patterns: [],
      interrupted: [
        interruptedHabitFixture({
          displayName: null,
          registrableDomain: 'plain-interrupted.example',
        }),
      ],
    })
    const { rerender } = renderSection()

    expect(
      screen.getByRole('link', { name: 'plain-interrupted.example' }),
    ).toHaveAttribute('href', '/domain/plain-interrupted.example')
    expect(screen.queryByText('habitsPatternsTitle')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /explain/ })).toBeNull()

    setHabitAsyncData({
      patterns: [
        habitPatternFixture({
          displayName: null,
          registrableDomain: 'plain-pattern.example',
        }),
      ],
      interrupted: [],
    })
    rerender(sectionNode())

    expect(
      screen.getByRole('link', { name: 'plain-pattern.example' }),
    ).toHaveAttribute('href', '/domain/plain-pattern.example')
    expect(screen.queryByText('habitsInterruptedTitle')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /explain/ })).toBeNull()
  })
})

function renderSection({ profileId }: { profileId?: string | null } = {}) {
  return render(sectionNode({ profileId }))
}

function sectionNode({ profileId }: { profileId?: string | null } = {}) {
  return (
    <MemoryRouter>
      <I18nProvider>
        <HabitsSection
          dateRange={dateRange}
          domainHref={(domain) => `/domain/${domain}`}
          profileId={profileId ?? null}
          scopeLabel="All profiles"
          t={t}
        />
      </I18nProvider>
    </MemoryRouter>
  )
}

function setHabitAsyncData({
  interrupted,
  patterns,
  interruptedLoading = false,
  patternsLoading = false,
}: {
  interrupted: InterruptedHabit[]
  patterns: HabitPattern[] | null
  interruptedLoading?: boolean
  patternsLoading?: boolean
}) {
  useAsyncDataMock
    .mockReturnValueOnce({
      data:
        patterns === null ? null : sectionResult('habit-patterns', patterns),
      loading: patternsLoading,
    })
    .mockReturnValueOnce({
      data: sectionResult('interrupted-habits', interrupted),
      loading: interruptedLoading,
    })
}

function sectionResult<T>(
  sectionId: string,
  data: T,
  state: CoreIntelligenceSectionMeta['state'] = 'ready',
): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: {
      sectionId,
      generatedAt: '2026-04-25T12:00:00.000Z',
      window: { kind: 'date-range', dateRange },
      moduleIds: [sectionId],
      sourceTables: ['visits'],
      includesEnrichment: false,
      state,
      stateReason: null,
      notes: [],
    },
  }
}

function habitPatternFixture(
  overrides: Partial<HabitPattern> = {},
): HabitPattern {
  return {
    registrableDomain: 'docs.example',
    displayName: 'Docs Habit',
    habitType: 'weekly_habit',
    meanIntervalDays: 3.5,
    cv: 0.2,
    visitCount: 12,
    lastVisitedAt: '2026-04-20T10:00:00Z',
    isInterrupted: false,
    ...overrides,
  }
}

function interruptedHabitFixture(
  overrides: Partial<InterruptedHabit> = {},
): InterruptedHabit {
  return {
    ...habitPatternFixture(),
    registrableDomain: 'interrupted.example',
    displayName: 'Interrupted Habit',
    meanIntervalDays: 4,
    daysSinceLastVisit: 21,
    interruptionThresholdDays: 8,
    isInterrupted: true,
    ...overrides,
  }
}
