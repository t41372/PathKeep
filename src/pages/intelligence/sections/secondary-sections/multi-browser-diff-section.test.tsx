import { render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  BrowserDiff,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { I18nProvider } from '../../../../lib/i18n'
import { MultiBrowserDiffSection } from './multi-browser-diff-section'

const dateRange: DateRange = { start: '2026-04-01', end: '2026-04-30' }

describe('MultiBrowserDiffSection', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'peekMultiBrowserDiff').mockReturnValue(null)
  })

  test('renders the empty state when the archive has fewer than two profiles', async () => {
    vi.spyOn(api, 'getMultiBrowserDiff').mockResolvedValue(
      section({
        profiles: [
          {
            profileId: 'chrome:Default',
            profileName: 'Chrome Default',
            browserFamily: 'Chrome',
            visitCount: 12,
            domainCount: 4,
          },
        ],
        sharedDomains: [],
        exclusiveDomains: [],
        categoryDistributions: [],
      }),
    )

    renderSection()

    await waitFor(() =>
      expect(
        screen.getByText('No multi-browser overlap yet'),
      ).toBeInTheDocument(),
    )
  })

  test('renders profile summaries, shared links, exclusive domains, and category shares', async () => {
    vi.spyOn(api, 'getMultiBrowserDiff').mockResolvedValue(
      section({
        profiles: [
          {
            profileId: 'chrome:Default',
            profileName: 'Chrome Default',
            browserFamily: 'Chrome',
            visitCount: 120,
            domainCount: 8,
          },
          {
            profileId: 'safari:Personal',
            profileName: 'Safari Personal',
            browserFamily: 'Safari',
            visitCount: 80,
            domainCount: 6,
          },
        ],
        sharedDomains: ['example.com'],
        exclusiveDomains: [
          {
            profileId: 'chrome:Default',
            registrableDomain: 'chrome-only.test',
            visitCount: 9,
          },
          {
            profileId: 'unknown:Ghost',
            registrableDomain: 'ghost-only.test',
            visitCount: 2,
          },
        ],
        categoryDistributions: [
          {
            profileId: 'chrome:Default',
            profileName: 'Chrome Default',
            categories: [
              { domainCategory: 'work', share: 0.75, visitCount: 90 },
              { domainCategory: 'unknown', share: 0.25, visitCount: 30 },
            ],
          },
          {
            profileId: 'safari:Personal',
            profileName: 'Safari Personal',
            categories: [
              { domainCategory: 'learning', share: 0.4, visitCount: 32 },
            ],
          },
        ],
      }),
    )

    renderSection()

    expect(
      (await screen.findAllByText('Chrome Default')).length,
    ).toBeGreaterThanOrEqual(2)
    expect(
      screen.getAllByText('Safari Personal').length,
    ).toBeGreaterThanOrEqual(1)
    expect(screen.getByText(/120 visits/)).toBeInTheDocument()
    expect(screen.getByText(/8 domains/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'example.com' })).toHaveAttribute(
      'href',
      '/domain/example.com',
    )
    expect(
      screen.getByRole('link', { name: 'chrome-only.test' }),
    ).toHaveAttribute('href', '/domain/chrome-only.test')
    expect(screen.getByText('unknown:Ghost')).toBeInTheDocument()
    expect(screen.getByText('Work')).toBeInTheDocument()
    expect(screen.getByText('Learning')).toBeInTheDocument()
    expect(screen.getByText('unknown')).toBeInTheDocument()
    expect(screen.getByTitle('Chrome Default: 75%')).toBeInTheDocument()
    expect(
      screen.getAllByTitle('Safari Personal: 0%').length,
    ).toBeGreaterThanOrEqual(2)
    expect(screen.getByTitle('Chrome Default: 0%')).toBeInTheDocument()
    expect(screen.getByTitle('Safari Personal: 40%')).toBeInTheDocument()
  })
})

function renderSection() {
  render(
    <MemoryRouter>
      <I18nProvider>
        <MultiBrowserDiffSection
          dateRange={dateRange}
          domainHref={(domain) => `/domain/${domain}`}
          language="en"
          scopeLabel="Archive"
          t={translate}
        />
      </I18nProvider>
    </MemoryRouter>,
  )
}

function section(
  data: BrowserDiff,
): CoreIntelligenceSectionResult<BrowserDiff> {
  return {
    data,
    meta: metaFixture(),
  }
}

function metaFixture(): CoreIntelligenceSectionMeta {
  return {
    sectionId: 'multi-browser-diff',
    generatedAt: '2026-04-25T12:00:00Z',
    window: { kind: 'date-range', dateRange },
    moduleIds: ['multi-browser-diff'],
    sourceTables: ['profiles'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function translate(key: string, vars?: Record<string, string | number>) {
  switch (key) {
    case 'multiBrowserTitle':
      return 'Multi-browser diff'
    case 'multiBrowserEmpty':
      return 'No multi-browser overlap yet'
    case 'multiBrowserVisits':
      return `${vars?.count ?? 0} visits`
    case 'multiBrowserDomains':
      return `${vars?.count ?? 0} domains`
    case 'multiBrowserShared':
      return `Shared domains ${vars?.count ?? 0}`
    case 'multiBrowserExclusive':
      return 'Exclusive domains'
    case 'multiBrowserCategories':
      return 'Category mix'
    case 'archiveWideBadge':
      return 'Archive-wide'
    case 'category_work':
      return 'Work'
    case 'category_learning':
      return 'Learning'
    case 'category_unknown':
      return ''
    default:
      return key
  }
}
