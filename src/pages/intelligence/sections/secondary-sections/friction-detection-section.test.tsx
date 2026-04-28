/**
 * @file friction-detection-section.test.tsx
 * @description Render-level coverage for the secondary friction-detection card.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify hidden low-signal ready payloads, loading/empty states, and meaningful signal cards.
 * - Keep domain-link and URL fallback rendering covered without mounting the full Intelligence page.
 *
 * ## Not responsible for
 * - Re-testing the backend friction detector.
 * - Re-testing shared friction heuristics in detail.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` to provide deterministic card payloads.
 * - Uses MemoryRouter for domain links.
 *
 * ## Performance notes
 * - Pure render tests preserve bounded list coverage without fetching overview data.
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
  FrictionSignal,
} from '../../../../lib/core-intelligence'
import { FrictionDetectionSection } from './friction-detection-section'

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

describe('FrictionDetectionSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading, degraded empty, and hides ready low-signal payloads', () => {
    useAsyncDataMock.mockReturnValue({ data: null, loading: true })
    const { container, rerender } = renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()

    useAsyncDataMock.mockReturnValue({
      data: frictionResult([], 'degraded'),
      loading: false,
    })
    rerender(sectionNode())
    expect(screen.getByText('frictionEmpty')).toBeVisible()

    useAsyncDataMock.mockReturnValue({
      data: frictionResult([
        frictionSignalFixture({
          description: '',
          evidenceType: 'strong',
          occurrenceCount: 12,
        }),
      ]),
      loading: false,
    })
    rerender(sectionNode())
    expect(container.firstChild).toBeNull()
  })

  test('renders meaningful strong and weak friction cards', () => {
    useAsyncDataMock.mockReturnValue({
      data: frictionResult([
        frictionSignalFixture(),
        frictionSignalFixture({
          registrableDomain: null,
          url: 'https://example.com/error',
          evidenceType: 'weak',
          signalKind: 'redirect_chain',
          occurrenceCount: 2,
          description: 'Redirect loop',
        }),
        frictionSignalFixture({
          registrableDomain: null,
          url: null,
          evidenceType: 'weak',
          signalKind: 'redirect_chain',
          occurrenceCount: 3,
          description: 'Unknown source loop',
        }),
      ]),
      loading: false,
    })

    renderSection()

    expect(screen.getByText('frictionTitle')).toBeVisible()
    expect(screen.getByText('frictionStrong')).toBeVisible()
    expect(screen.getAllByText('frictionWeak').length).toBeGreaterThanOrEqual(2)
    expect(screen.getByRole('link', { name: 'docs.example' })).toHaveAttribute(
      'href',
      '/domain/docs.example',
    )
    expect(screen.getByText('https://example.com/error')).toBeVisible()
    expect(screen.getByText('—')).toBeVisible()
    expect(screen.getByText('Repeated bounces')).toBeVisible()
    expect(screen.getByText('Redirect loop')).toBeVisible()
    expect(screen.getByText('Unknown source loop')).toBeVisible()
  })
})

function renderSection() {
  return render(sectionNode())
}

function sectionNode() {
  return (
    <MemoryRouter>
      <I18nProvider>
        <FrictionDetectionSection
          dateRange={dateRange}
          domainHref={(domain) => `/domain/${domain}`}
          profileId={null}
          scopeLabel="All profiles"
          t={t}
        />
      </I18nProvider>
    </MemoryRouter>
  )
}

function frictionResult(
  data: FrictionSignal[],
  state: CoreIntelligenceSectionMeta['state'] = 'ready',
): CoreIntelligenceSectionResult<FrictionSignal[]> {
  return {
    data,
    meta: {
      sectionId: 'friction',
      generatedAt: '2026-04-25T12:00:00.000Z',
      window: { kind: 'date-range', dateRange },
      moduleIds: ['friction'],
      sourceTables: ['visits'],
      includesEnrichment: false,
      state,
      stateReason: null,
      notes: [],
    },
  }
}

function frictionSignalFixture(
  overrides: Partial<FrictionSignal> = {},
): FrictionSignal {
  return {
    registrableDomain: 'docs.example',
    url: null,
    evidenceType: 'strong',
    signalKind: 'bounce_pattern',
    occurrenceCount: 3,
    description: 'Repeated bounces',
    ...overrides,
  }
}
