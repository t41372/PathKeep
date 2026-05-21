/**
 * Tests for the PaperIntelligenceView composition.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { PaperIntelligenceView, type PaperIntelligenceViewCopy } from './index'
import type { PaperKpiCell } from './paper-kpi-strip'
import type { PaperTopicRow } from './paper-topic-timeline'
import type { PaperDomainRankRow } from './paper-domain-rank'
import type { PaperThreadRow } from './paper-thread-list'
import type { PaperRefindItem } from './paper-refind-shelf'

const COPY: PaperIntelligenceViewCopy = {
  topicsTitle: 'Topics, over the last 30 days',
  topicsRangeBadge: '30D · 90D · 1Y',
  topicsSummary: 'Rust internals dominated this week.',
  domainsTitle: 'Where you spent your time',
  domainsBadge: 'This week',
  sessionsTitle: 'Recent sessions',
  sessionsBadge: 'A session = pages in one sitting',
  threadsTitle: 'Active threads',
  refindTitle: 'Refind candidates',
  refindBadge: '3+ visits / 90d',
  sessionPagesLabel: 'pages',
  threadPagesLabel: 'pages',
}

const KPIS: PaperKpiCell[] = [
  { id: 'week', label: 'This week', value: '1,247', sub: '↑ 14% vs last' },
  { id: 'top', label: 'Top domain', value: 'github.com', monoValue: true },
]

const TOPICS: PaperTopicRow[] = [
  {
    id: 'rust',
    name: 'Rust async',
    color: '#3d5a80',
    count: 89,
    trend: 'up',
    bars: [{ left: 0, width: 18, opacity: 0.3 }],
  },
]

const DOMAINS: PaperDomainRankRow[] = [
  { domain: 'github.com', count: 342 },
  { domain: 'docs.rs', count: 178 },
]

const SESSIONS: PaperThreadRow[] = [
  { id: 's1', title: 'Rust async runtime deep dive', meta: 'today', count: 22 },
]

const THREADS: PaperThreadRow[] = [
  { id: 't1', title: 'PathKeep development', meta: '12d', count: 89 },
]

const REFIND: PaperRefindItem[] = [
  { id: 'r1', title: 'tokio docs', domain: 'docs.rs', meta: '47 visits' },
]

function renderView(
  overrides: Partial<Parameters<typeof PaperIntelligenceView>[0]> = {},
) {
  return render(
    <PaperIntelligenceView
      kpis={KPIS}
      topics={TOPICS}
      topicAxisLabels={['Apr 17', 'May 17']}
      domains={DOMAINS}
      sessions={SESSIONS}
      threads={THREADS}
      refindItems={REFIND}
      resolveDomainColor={() => '#888'}
      resolveDomainAbbr={(domain) => domain.slice(0, 3).toUpperCase()}
      copy={COPY}
      testId="intel"
      {...overrides}
    />,
  )
}

describe('PaperIntelligenceView', () => {
  test('renders all four cards and the KPI strip', () => {
    renderView()

    expect(screen.getByTestId('paper-intelligence-kpis')).toBeVisible()
    expect(screen.getByText('Topics, over the last 30 days')).toBeVisible()
    expect(screen.getByText('Where you spent your time')).toBeVisible()
    expect(screen.getByText('Recent sessions')).toBeVisible()
    expect(screen.getByText('Active threads')).toBeVisible()
    expect(screen.getByText('Refind candidates')).toBeVisible()
  })

  test('renders the topic summary block when copy provides one', () => {
    renderView()
    expect(
      screen.getByTestId('paper-intelligence-topics-summary'),
    ).toHaveTextContent('Rust internals dominated this week.')
  })

  test('omits the topic summary when copy does not supply one', () => {
    renderView({ copy: { ...COPY, topicsSummary: undefined } })
    expect(screen.queryByTestId('paper-intelligence-topics-summary')).toBeNull()
  })

  test('selecting a domain forwards to onSelectDomain', () => {
    const onSelectDomain = vi.fn()
    renderView({ onSelectDomain })
    fireEvent.click(screen.getByTestId('paper-domain-rank-docs.rs'))
    expect(onSelectDomain).toHaveBeenCalledWith('docs.rs')
  })

  test('selecting a session / thread / refind row forwards the canonical row', () => {
    const onSelectSession = vi.fn()
    const onSelectThread = vi.fn()
    const onSelectRefind = vi.fn()
    renderView({ onSelectSession, onSelectThread, onSelectRefind })

    fireEvent.click(screen.getByTestId('paper-thread-s1'))
    expect(onSelectSession).toHaveBeenCalledWith(SESSIONS[0])

    fireEvent.click(screen.getByTestId('paper-thread-t1'))
    expect(onSelectThread).toHaveBeenCalledWith(THREADS[0])

    fireEvent.click(screen.getByTestId('paper-refind-r1'))
    expect(onSelectRefind).toHaveBeenCalledWith(REFIND[0])
  })

  test('topic axis labels render when supplied', () => {
    renderView()
    expect(screen.getByText('Apr 17')).toBeVisible()
    expect(screen.getByText('May 17')).toBeVisible()
  })

  test('omits each badge when copy leaves the field undefined', () => {
    renderView({
      copy: {
        ...COPY,
        topicsRangeBadge: 'TOPICS-BADGE-X',
        domainsBadge: undefined,
        sessionsBadge: undefined,
        refindBadge: undefined,
      },
    })
    // The conditional `: undefined` branches at lines 129/144-148/178-180
    // of paper-intelligence-view.tsx fire when each *Badge field is
    // undefined. The KPI "This week" text comes from the kpi.label data,
    // not the badge slot, so we don't try to assert on it. Use unique
    // sentinels for the badges that do render.
    expect(screen.getByText('TOPICS-BADGE-X')).toBeVisible()
    expect(
      screen.queryByText('A session = pages in one sitting'),
    ).toBeNull()
    expect(screen.queryByText('3+ visits / 90d')).toBeNull()
  })
})
