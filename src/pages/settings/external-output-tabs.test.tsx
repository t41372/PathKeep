/**
 * @file external-output-tabs.test.tsx
 * @description Guards the public and widget external-output snapshot tabs.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify public snapshot lists, empty states, notes, and timestamp fallback rendering.
 * - Verify widget snapshot trusted-card warnings, card badges, optional copy, and timestamp fallback rendering.
 *
 * ## Not responsible for
 * - Re-testing the settings route's tab state or payload fetching hooks.
 * - Re-testing shared review primitives beyond their integration points.
 *
 * ## Dependencies
 * - Uses the real i18n provider and MemoryRouter because the tabs render first-party drilldown links.
 *
 * ## Performance notes
 * - Fixtures stay intentionally tiny; backend output payloads are capped before they reach these tabs.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import type {
  DateRange,
  DigestSummary,
  IntelligenceEmbedCardPayload,
  IntelligencePublicSnapshot,
  IntelligenceWidgetSnapshot,
} from '../../lib/core-intelligence'
import { createNamespaceTranslator, I18nProvider } from '../../lib/i18n'
import { ExternalOutputsEmbedTab } from './external-outputs-embed-tab'
import { ExternalOutputsPublicTab } from './external-outputs-public-tab'
import { OutputTargetLinks } from './external-outputs-target-links'
import { ExternalOutputsWidgetTab } from './external-outputs-widget-tab'

const dateRange: DateRange = {
  start: '2026-01-01',
  end: '2026-01-31',
}

const digestSummary: DigestSummary = {
  dateRange,
  totalVisits: { value: 1200, trend: 'up' },
  totalSearches: { value: 42, trend: 'flat' },
  newDomains: { value: 8, trend: 'down' },
  deepReadPages: { value: 5, trend: 'flat' },
  refindPages: { value: 2, trend: 'flat' },
}

const settingsT = createNamespaceTranslator('en', 'settings')
const commonT = createNamespaceTranslator('en', 'common')
const intelligenceT = createNamespaceTranslator('en', 'intelligence')

function renderTab(ui: React.ReactElement) {
  return render(
    <I18nProvider>
      <MemoryRouter>{ui}</MemoryRouter>
    </I18nProvider>,
  )
}

function publicSnapshot(
  overrides: Partial<IntelligencePublicSnapshot> = {},
): IntelligencePublicSnapshot {
  return {
    generatedAt: '2026-04-21T10:15:00.000Z',
    dateRange,
    digestSummary,
    topDomains: ['example.com'],
    searchEngines: [
      {
        searchEngine: 'google',
        displayName: null,
        searchCount: 7,
      },
    ],
    discoveryTrend: {
      points: [
        {
          dateKey: '2026-01-05',
          discoveryRate: 0.42,
          newDomainCount: 3,
          totalVisits: 9,
        },
      ],
      availableYears: [2026],
    },
    notes: ['Public note'],
    ...overrides,
  }
}

function widgetSnapshot(
  overrides: Partial<IntelligenceWidgetSnapshot> = {},
): IntelligenceWidgetSnapshot {
  return {
    generatedAt: '2026-04-21T10:15:00.000Z',
    dateRange,
    digestSummary,
    highlights: [
      {
        cardId: 'trusted-card',
        cardType: 'digest',
        title: 'Visits',
        eyebrow: 'top site',
        body: 'Total visits in the selected intelligence window.',
        href: '/intelligence',
        primaryTarget: null,
        secondaryTargets: [],
        internalOnly: true,
      },
      {
        cardId: 'plain-card',
        cardType: 'digest',
        title: 'Custom title',
        eyebrow: null,
        body: 'Custom body',
        href: null,
        primaryTarget: null,
        secondaryTargets: [],
        internalOnly: false,
      },
    ],
    notes: ['Widget note'],
    ...overrides,
  }
}

describe('external-output snapshot tabs', () => {
  test('renders embed cards with optional eyebrow, metrics, links, and empty state', () => {
    const cards: IntelligenceEmbedCardPayload[] = [
      {
        cardId: 'plain-card',
        cardType: 'digest',
        title: 'Plain card',
        eyebrow: null,
        body: 'Plain body',
        metricLabel: null,
        metricValue: null,
        href: null,
        primaryTarget: null,
        secondaryTargets: [],
        internalOnly: false,
      },
      {
        cardId: 'metric-card',
        cardType: 'digest',
        title: 'Metric card',
        eyebrow: 'Top site',
        body: 'Metric body',
        metricLabel: 'visit_count',
        metricValue: '128',
        href: null,
        primaryTarget: { kind: 'domain', domain: 'example.com' },
        secondaryTargets: [{ kind: 'day', date: '2026-01-05' }],
        internalOnly: true,
      },
    ]

    const { rerender } = renderTab(
      <ExternalOutputsEmbedTab
        activeProfileId="chrome:Default"
        cards={cards}
        commonT={commonT}
        copyFeedback={null}
        copyLabel="Copy JSON"
        dateRange={dateRange}
        json="[]"
        onCopy={vi.fn()}
        t={settingsT}
      />,
    )

    expect(screen.getByText('Plain card')).toBeVisible()
    expect(screen.getByText('Metric card')).toBeVisible()
    expect(screen.getByText('TOP SITE')).toBeVisible()
    expect(screen.getByText('visit_count')).toBeVisible()
    expect(
      screen.getByText(settingsT('externalOutputsTrustedOnlyBadge')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', {
        name: settingsT('externalOutputsOpenInsights'),
      }),
    ).toHaveAttribute(
      'href',
      '/intelligence/domain/example.com?range=custom&start=2026-01-01&end=2026-01-31&profileId=chrome%3ADefault',
    )

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <ExternalOutputsEmbedTab
            activeProfileId={null}
            cards={[]}
            commonT={commonT}
            copyFeedback={null}
            copyLabel="Copy JSON"
            dateRange={dateRange}
            json="[]"
            onCopy={vi.fn()}
            t={settingsT}
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expect(
      screen.getByText(settingsT('externalOutputsEmbedEmpty')),
    ).toBeVisible()
  })

  test('renders populated public snapshots with drilldown lists and notes', () => {
    renderTab(
      <ExternalOutputsPublicTab
        activeProfileId="chrome:Default"
        commonT={commonT}
        copyFeedback={null}
        copyLabel="Copy JSON"
        intelligenceT={intelligenceT}
        json="{}"
        language="en"
        onCopy={vi.fn()}
        snapshot={publicSnapshot()}
        t={settingsT}
      />,
    )

    expect(screen.getByText('example.com')).toHaveAttribute(
      'href',
      expect.stringContaining('/intelligence/domain/example.com'),
    )
    expect(screen.getByText('google')).toBeInTheDocument()
    expect(screen.getByText('0.42')).toBeInTheDocument()
    expect(screen.getByText('Public note')).toBeInTheDocument()
  })

  test('renders public snapshot empty states and raw timestamp fallback', () => {
    renderTab(
      <ExternalOutputsPublicTab
        activeProfileId={null}
        commonT={commonT}
        copyFeedback={null}
        copyLabel="Copy JSON"
        intelligenceT={intelligenceT}
        json="{}"
        language="en"
        onCopy={vi.fn()}
        snapshot={publicSnapshot({
          generatedAt: 'not-a-date',
          searchEngines: [],
          discoveryTrend: { points: [], availableYears: [] },
          notes: [],
        })}
        t={settingsT}
      />,
    )

    expect(screen.getByText('not-a-date')).toBeInTheDocument()
    expect(
      screen.getByText(settingsT('externalOutputsNoSearchEngines')),
    ).toBeInTheDocument()
    expect(
      screen.getByText(settingsT('externalOutputsNoDiscoveryTrend')),
    ).toBeInTheDocument()
    expect(screen.queryByText('Public note')).not.toBeInTheDocument()
  })

  test('renders trusted widget cards, optional card copy, links, and notes', () => {
    renderTab(
      <ExternalOutputsWidgetTab
        activeProfileId="chrome:Default"
        commonT={commonT}
        copyFeedback={null}
        copyLabel="Copy JSON"
        intelligenceT={intelligenceT}
        json="{}"
        language="en"
        onCopy={vi.fn()}
        snapshot={widgetSnapshot()}
        t={settingsT}
        trustedCards
      />,
    )

    expect(
      screen.getByText(settingsT('externalOutputsWidgetTrustedTitle')),
    ).toBeInTheDocument()
    expect(screen.getByText('TOP SITE')).toBeInTheDocument()
    expect(
      screen.getByText(settingsT('externalOutputsTrustedOnlyBadge')),
    ).toBeInTheDocument()
    expect(screen.getByText('/intelligence')).toBeInTheDocument()
    expect(screen.getByText('Widget note')).toBeInTheDocument()
    expect(screen.getByText('Custom title')).toBeInTheDocument()
  })

  test('renders widget snapshots without trusted warnings, notes, or formatted timestamp', () => {
    renderTab(
      <ExternalOutputsWidgetTab
        activeProfileId={null}
        commonT={commonT}
        copyFeedback={null}
        copyLabel="Copy JSON"
        intelligenceT={intelligenceT}
        json="{}"
        language="en"
        onCopy={vi.fn()}
        snapshot={widgetSnapshot({
          generatedAt: 'not-a-date',
          highlights: [
            {
              cardId: 'plain-card',
              cardType: 'digest',
              title: 'Custom title',
              eyebrow: null,
              body: 'Custom body',
              href: null,
              primaryTarget: null,
              secondaryTargets: [],
              internalOnly: false,
            },
          ],
          notes: [],
        })}
        t={settingsT}
        trustedCards={false}
      />,
    )

    expect(screen.getByText('not-a-date')).toBeInTheDocument()
    expect(
      screen.queryByText(settingsT('externalOutputsWidgetTrustedTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(settingsT('externalOutputsTrustedOnlyBadge')),
    ).not.toBeInTheDocument()
    expect(screen.queryByText('Widget note')).not.toBeInTheDocument()
  })

  test('renders output target links from primary and secondary entity references', () => {
    renderTab(
      <OutputTargetLinks
        activeProfileId="chrome:Default"
        card={{
          cardId: 'entity-card',
          cardType: 'digest',
          title: 'Custom title',
          body: 'Custom body',
          href: null,
          primaryTarget: { kind: 'domain', domain: 'example.com' },
          secondaryTargets: [{ kind: 'day', date: '2026-01-05' }],
          internalOnly: false,
        }}
        dateRange={dateRange}
        t={settingsT}
      />,
    )

    expect(
      screen.getByRole('link', {
        name: settingsT('externalOutputsOpenInsights'),
      }),
    ).toHaveAttribute(
      'href',
      '/intelligence/domain/example.com?range=custom&start=2026-01-01&end=2026-01-31&profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: '2026-01-05' })).toHaveAttribute(
      'href',
      '/intelligence/day/2026-01-05?profileId=chrome%3ADefault',
    )
  })
})
