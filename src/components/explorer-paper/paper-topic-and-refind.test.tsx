/**
 * Tests for PaperTopicTimeline + PaperRefindShelf.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperRefindShelf,
  PaperTopicTimeline,
  type PaperRefindItem,
  type PaperTopicRow,
} from './index'

const TOPICS: PaperTopicRow[] = [
  {
    id: 'rust',
    name: 'Rust async runtime',
    color: '#3d5a80',
    count: 89,
    trend: 'up',
    bars: [
      { left: 0, width: 18, opacity: 0.3 },
      { left: 22, width: 12, opacity: 0.5 },
      { left: 82, width: 18, opacity: 1.0 },
    ],
  },
  {
    id: 'llm',
    name: 'LLM fine-tuning',
    color: '#8b4049',
    count: 54,
    trend: 'down',
    bars: [{ left: 0, width: 28, opacity: 0.9 }],
  },
  {
    id: 'sqlite',
    name: 'SQLite internals',
    color: '#7e8d50',
    count: 41,
    trend: 'flat',
    bars: [{ left: 50, width: 12, opacity: 0.4 }],
  },
]

describe('PaperTopicTimeline', () => {
  test('renders each topic with its name, count, and trend glyph', () => {
    render(<PaperTopicTimeline rows={TOPICS} testId="timeline" />)

    expect(screen.getByText('Rust async runtime')).toBeVisible()
    expect(screen.getByText('LLM fine-tuning')).toBeVisible()
    expect(screen.getByText('SQLite internals')).toBeVisible()

    expect(
      within(screen.getByTestId('paper-topic-trend-rust')).getByText(/↑/),
    ).toBeVisible()
    expect(
      within(screen.getByTestId('paper-topic-trend-llm')).getByText(/↓/),
    ).toBeVisible()
    expect(
      within(screen.getByTestId('paper-topic-trend-sqlite')).getByText(/—/),
    ).toBeVisible()
  })

  test('renders the bar segments inside each topic track', () => {
    render(<PaperTopicTimeline rows={TOPICS} testId="timeline-bars" />)
    expect(screen.getByTestId('paper-topic-bar-rust-0')).toBeInTheDocument()
    expect(screen.getByTestId('paper-topic-bar-rust-1')).toBeInTheDocument()
    expect(screen.getByTestId('paper-topic-bar-rust-2')).toBeInTheDocument()
    // LLM row has only one segment
    expect(screen.getByTestId('paper-topic-bar-llm-0')).toBeInTheDocument()
    expect(screen.queryByTestId('paper-topic-bar-llm-1')).toBeNull()
  })

  test('renders the axis labels when provided', () => {
    render(
      <PaperTopicTimeline
        rows={TOPICS}
        axisLabels={['Apr 17', 'Apr 24', 'May 1', 'May 8', 'May 17']}
        testId="timeline-axis"
      />,
    )

    const axis = screen.getByTestId('paper-topic-axis')
    expect(within(axis).getByText('Apr 17')).toBeVisible()
    expect(within(axis).getByText('May 17')).toBeVisible()
  })

  test('omits the axis row when no labels are supplied', () => {
    render(<PaperTopicTimeline rows={TOPICS} testId="timeline-no-axis" />)
    expect(screen.queryByTestId('paper-topic-axis')).toBeNull()
  })

  test('uses the trend colour class for up / down / flat', () => {
    render(<PaperTopicTimeline rows={TOPICS} />)
    expect(screen.getByTestId('paper-topic-trend-rust').className).toContain(
      'text-success',
    )
    expect(screen.getByTestId('paper-topic-trend-llm').className).toContain(
      'text-error',
    )
    expect(screen.getByTestId('paper-topic-trend-sqlite').className).toContain(
      'text-ink-secondary',
    )
  })
})

const REFIND_ITEMS: PaperRefindItem[] = [
  {
    id: 'tokio',
    title: 'tokio-rs/tokio: A runtime for writing reliable async applications',
    domain: 'github.com',
    meta: '47 visits · over 11 months',
  },
  {
    id: 'sqlx',
    title: 'docs.rs — sqlx',
    domain: 'docs.rs',
    meta: '31 visits · over 6 months',
  },
]

describe('PaperRefindShelf', () => {
  const resolveColor = (domain: string) =>
    domain === 'github.com' ? '#24292e' : '#7b5b3a'
  const resolveAbbr = (domain: string) =>
    domain === 'github.com' ? 'GIT' : 'DOC'

  test('renders each row with title, meta, and domain swatch', () => {
    render(
      <PaperRefindShelf
        items={REFIND_ITEMS}
        resolveDomainColor={resolveColor}
        resolveDomainAbbr={resolveAbbr}
        testId="refind"
      />,
    )

    expect(
      screen.getByText(
        'tokio-rs/tokio: A runtime for writing reliable async applications',
      ),
    ).toBeVisible()
    expect(screen.getByText('47 visits · over 11 months')).toBeVisible()
    expect(screen.getByText('GIT')).toBeVisible()
    expect(screen.getByText('DOC')).toBeVisible()
  })

  test('clicking a row forwards the canonical item to onSelect', () => {
    const onSelect = vi.fn()
    render(
      <PaperRefindShelf
        items={REFIND_ITEMS}
        resolveDomainColor={resolveColor}
        resolveDomainAbbr={resolveAbbr}
        onSelect={onSelect}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-refind-sqlx'))
    expect(onSelect).toHaveBeenCalledWith(REFIND_ITEMS[1])
  })

  test('rows are disabled when no handler is supplied', () => {
    render(
      <PaperRefindShelf
        items={REFIND_ITEMS}
        resolveDomainColor={resolveColor}
        resolveDomainAbbr={resolveAbbr}
      />,
    )

    for (const item of REFIND_ITEMS) {
      expect(
        screen.getByTestId<HTMLButtonElement>(`paper-refind-${item.id}`)
          .disabled,
      ).toBe(true)
    }
  })
})
