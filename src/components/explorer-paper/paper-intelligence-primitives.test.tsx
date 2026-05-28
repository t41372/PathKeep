/**
 * Tests for the paper Intelligence primitives — KPI strip, domain rank
 * list, thread list.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperDomainRankList,
  PaperKpiStrip,
  PaperThreadList,
  type PaperKpiCell,
  type PaperThreadRow,
} from './index'

const KPI_CELLS: PaperKpiCell[] = [
  { id: 'week', label: 'This week', value: '1,247', sub: '↑ 14% vs last week' },
  {
    id: 'top-domain',
    label: 'Top domain',
    value: 'github.com',
    monoValue: true,
    sub: '342 visits · 27.4%',
  },
  {
    id: 'explore-exploit',
    label: 'Explore / exploit',
    value: '38',
    monoTail: '/ 62',
    sub: 'focused mode',
  },
  { id: 'threads', label: 'Active threads', value: '7' },
]

describe('PaperKpiStrip', () => {
  test('renders each cell with label, value, and sub line', () => {
    render(<PaperKpiStrip cells={KPI_CELLS} testId="strip" />)

    expect(screen.getByText('This week')).toBeVisible()
    expect(screen.getByText('1,247')).toBeVisible()
    expect(screen.getByText('↑ 14% vs last week')).toBeVisible()
    expect(screen.getByText('github.com')).toBeVisible()
    expect(screen.getByText('/ 62')).toBeVisible()
    expect(screen.getByText('focused mode')).toBeVisible()
  })

  test('omits the sub line when not provided', () => {
    render(<PaperKpiStrip cells={[KPI_CELLS[3]]} testId="strip-no-sub" />)
    const cell = screen.getByTestId('paper-kpi-threads')
    expect(cell).toHaveTextContent('Active threads')
    expect(cell).toHaveTextContent('7')
    expect(cell.textContent).not.toMatch(/vs/)
  })

  test('renders mono-styled identifier values without the serif scale', () => {
    render(<PaperKpiStrip cells={[KPI_CELLS[1]]} />)
    const value = screen.getByText('github.com')
    expect(value.className).toContain('font-mono')
    expect(value.className).not.toContain('font-serif')
  })
})

describe('PaperDomainRankList', () => {
  const rows = [
    { domain: 'github.com', count: 342 },
    { domain: 'docs.rs', count: 178 },
    { domain: 'arxiv.org', count: 68 },
  ]

  test('renders ranks, domains, and counts', () => {
    render(<PaperDomainRankList rows={rows} testId="rank" />)
    expect(screen.getByText('01')).toBeVisible()
    expect(screen.getByText('github.com')).toBeVisible()
    expect(screen.getByText('178')).toBeVisible()
    expect(screen.getByText('arxiv.org')).toBeVisible()
  })

  test('forwards the clicked domain to onSelectDomain', () => {
    const onSelect = vi.fn()
    render(<PaperDomainRankList rows={rows} onSelectDomain={onSelect} />)
    fireEvent.click(screen.getByTestId('paper-domain-rank-docs.rs'))
    expect(onSelect).toHaveBeenCalledWith('docs.rs')
  })

  test('rows are disabled without a handler', () => {
    render(<PaperDomainRankList rows={rows} />)
    expect(
      screen.getByTestId<HTMLButtonElement>('paper-domain-rank-github.com')
        .disabled,
    ).toBe(true)
  })
})

describe('PaperThreadList', () => {
  const rows: PaperThreadRow[] = [
    {
      id: 't1',
      title: 'Rust async runtime',
      meta: '12d · today',
      count: 89,
      tone: 'hot',
    },
    {
      id: 't2',
      title: 'Tauri 2 plugin development',
      meta: '3d · today',
      count: 34,
      tone: 'warm',
    },
    {
      id: 't3',
      title: 'Wavetable synthesis & Vital',
      meta: '1d · 3d ago',
      count: 12,
      tone: 'cool',
    },
  ]

  test('renders each row with title, meta, and count', () => {
    render(<PaperThreadList rows={rows} testId="threads" />)
    expect(screen.getByText('Rust async runtime')).toBeVisible()
    expect(screen.getByText('12d · today')).toBeVisible()
    expect(screen.getByText('89')).toBeVisible()
  })

  test('clicking a row forwards the canonical row object', () => {
    const onSelect = vi.fn()
    render(<PaperThreadList rows={rows} onSelect={onSelect} />)
    fireEvent.click(screen.getByTestId('paper-thread-t2'))
    expect(onSelect).toHaveBeenCalledWith(rows[1])
  })

  test('disables all rows when no handler is supplied', () => {
    render(<PaperThreadList rows={rows} />)
    for (const row of rows) {
      expect(
        screen.getByTestId<HTMLButtonElement>(`paper-thread-${row.id}`)
          .disabled,
      ).toBe(true)
    }
  })

  test('honours a custom count label', () => {
    render(
      <PaperThreadList rows={rows} onSelect={() => {}} countLabel="items" />,
    )
    const item = screen.getAllByText('items')
    expect(item.length).toBe(rows.length)
  })
})
