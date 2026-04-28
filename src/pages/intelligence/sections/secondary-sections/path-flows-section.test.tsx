/**
 * @file path-flows-section.test.tsx
 * @description Coverage for the secondary Core Intelligence path-flow section.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify low-signal hiding, step-count switching, focused domain links, and archive-wide explainability boundaries.
 * - Keep the route-level href contract covered without mounting the full Intelligence page.
 *
 * ## Not responsible for
 * - Re-testing the shared path-flow heuristic internals.
 * - Re-testing section metadata rendering.
 *
 * ## Dependencies
 * - Mocks Core Intelligence path-flow API cache/read functions.
 * - Uses MemoryRouter because flow steps are route links.
 *
 * ## Performance notes
 * - Uses cached payloads so the section renders synchronously while preserving state transitions.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  PathFlow,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { PathFlowsSection } from './path-flows-section'

vi.mock('../../../../components/intelligence/section-meta', () => ({
  IntelligenceSectionMeta: ({ scopeLabel }: { scopeLabel: string }) => (
    <span>{scopeLabel}</span>
  ),
}))

const dateRange: DateRange = { start: '2026-04-01', end: '2026-04-30' }

describe('PathFlowsSection', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('hides ready low-signal path-flow payloads', () => {
    vi.spyOn(api, 'peekPathFlows').mockReturnValue(
      section(
        [
          flowFixture({
            flowId: 'utility',
            flowPattern: 'localhost -> callback',
            occurrenceCount: 8,
          }),
        ],
        'ready',
      ),
    )
    vi.spyOn(api, 'getPathFlows').mockImplementation(pendingPathFlows)

    const { container } = renderSection()

    expect(container).toBeEmptyDOMElement()
  })

  test('renders meaningful flows, focused links, explainability, and step-count changes', async () => {
    const user = userEvent.setup()
    const getPathFlows = vi
      .spyOn(api, 'getPathFlows')
      .mockImplementation(pendingPathFlows)
    vi.spyOn(api, 'peekPathFlows').mockImplementation(
      (_range, _profileId, stepCount) =>
        stepCount === 2
          ? section([flowFixture({ flowId: 'flow-2', stepCount: 2 })], 'ready')
          : section([flowFixture()], 'ready'),
    )

    renderSection({ profileId: 'chrome:Default' })

    expect(screen.getByText('Path flows')).toBeVisible()
    expect(screen.getByText('Chrome Default')).toBeVisible()
    expect(screen.getByRole('link', { name: 'docs.example' })).toHaveAttribute(
      'href',
      '/domain/docs.example?focus=path-flow:flow-1',
    )
    expect(screen.getByText('external tool')).toBeVisible()
    expect(screen.getByText('3 occurrences')).toBeVisible()
    expect(screen.getByRole('button', { name: /explainTitle/ })).toBeVisible()

    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Step count' }),
      ['2'],
    )
    expect(getPathFlows).toHaveBeenLastCalledWith(
      dateRange,
      'chrome:Default',
      2,
      15,
    )
    expect(screen.getByRole('link', { name: 'docs.example' })).toHaveAttribute(
      'href',
      '/domain/docs.example?focus=path-flow:flow-2',
    )
  })

  test('omits explainability for archive-wide flows', () => {
    vi.spyOn(api, 'peekPathFlows').mockReturnValue(
      section([flowFixture()], 'ready'),
    )
    vi.spyOn(api, 'getPathFlows').mockImplementation(pendingPathFlows)

    renderSection({ profileId: null })

    expect(
      screen.queryByRole('button', { name: /How this was determined/ }),
    ).not.toBeInTheDocument()
  })
})

function renderSection({ profileId }: { profileId?: string | null } = {}) {
  const resolvedProfileId =
    profileId === undefined ? 'chrome:Default' : profileId
  return render(
    <MemoryRouter>
      <PathFlowsSection
        dateRange={dateRange}
        focusedDomainHref={(domain, focus) =>
          `/domain/${domain}?focus=${focus.focusType}:${focus.focusId}`
        }
        profileId={resolvedProfileId}
        scopeLabel={profileId === null ? 'Archive-wide' : 'Chrome Default'}
        t={translate}
      />
    </MemoryRouter>,
  )
}

function section(
  data: PathFlow[],
  state: CoreIntelligenceSectionMeta['state'],
): CoreIntelligenceSectionResult<PathFlow[]> {
  return {
    data,
    meta: {
      generatedAt: '2026-04-25T12:00:00Z',
      includesEnrichment: false,
      moduleIds: ['path-flows'],
      notes: [],
      sectionId: 'path-flows',
      sourceTables: ['path_flows'],
      state,
      stateReason: null,
      window: {
        dateRange,
        kind: 'date-range',
      },
    },
  }
}

function pendingPathFlows() {
  return new Promise<CoreIntelligenceSectionResult<PathFlow[]>>(() => {})
}

function flowFixture(overrides: Partial<PathFlow> = {}): PathFlow {
  return {
    flowId: 'flow-1',
    flowPattern: 'docs.example -> tool.example -> blog.example',
    lastSeenAt: '2026-04-25T12:00:00Z',
    occurrenceCount: 3,
    stepCount: 3,
    steps: [
      {
        index: 0,
        label: 'docs.example',
        registrableDomain: 'docs.example',
      },
      {
        index: 1,
        label: 'external tool',
        registrableDomain: null,
      },
      {
        index: 2,
        label: 'blog.example',
        registrableDomain: 'blog.example',
      },
    ],
    ...overrides,
  }
}

function translate(key: string, vars?: Record<string, string | number>) {
  switch (key) {
    case 'pathFlowsTitle':
      return 'Path flows'
    case 'pathFlowsStepLabel':
      return 'Step count'
    case 'pathFlowsStep2':
      return '2 steps'
    case 'pathFlowsStep3':
      return '3 steps'
    case 'pathFlowsEmpty':
      return 'No path flows'
    case 'pathFlowsOccurrences':
      return `${vars?.count ?? 0} occurrences`
    case 'explainabilityToggle':
      return 'How this was determined'
    default:
      return key
  }
}
