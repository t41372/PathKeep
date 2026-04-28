/**
 * @file metric-grid.test.tsx
 * @description Guards the Intelligence metric grid trend rendering contract.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Verify metric cards can render with and without optional icons.
 * - Verify trend badges cover positive, negative, flat, and absent change values.
 *
 * ## Not responsible for
 * - Re-testing route-specific digest metric builders.
 *
 * ## Dependencies
 * - Depends only on the reusable Intelligence metric grid component.
 *
 * ## Performance notes
 * - Tiny static fixture keeps trend branch coverage cheap.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { IntelligenceMetricGrid } from './metric-grid'

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('IntelligenceMetricGrid', () => {
  test('renders icon, trend, and no-trend metric variants', () => {
    const { container } = render(
      <IntelligenceMetricGrid
        items={[
          {
            icon: '↑',
            label: 'Visits',
            value: '1,200',
            trend: { value: 1200, changePercent: 12.4, trend: 'up' },
          },
          {
            label: 'Searches',
            value: '42',
            trend: { value: 42, changePercent: -5.2, trend: 'down' },
          },
          {
            label: 'Flat',
            value: '8',
            trend: { value: 8, changePercent: 0, trend: 'flat' },
          },
          {
            label: 'Pending',
            value: '0',
            trend: { value: 0, changePercent: null, trend: 'flat' },
          },
          {
            label: 'Plain',
            value: 'n/a',
          },
        ]}
        t={t}
      />,
    )

    expect(container.firstElementChild).toHaveClass('digest-cards')
    expect(screen.getByText('↑')).toHaveClass('digest-card__icon')
    expect(screen.getByText('+12% ↑')).toHaveClass('trend-badge--up')
    expect(screen.getByText('+12% ↑')).toHaveAccessibleName(
      'trendLabel:{"direction":"up","percent":12.4}',
    )
    expect(screen.getByText('-5% ↓')).toHaveClass('trend-badge--down')
    expect(screen.getByText('-5% ↓')).toHaveAccessibleName(
      'trendLabel:{"direction":"down","percent":5.2}',
    )
    expect(screen.getByText('0% =')).toHaveClass('trend-badge--flat')
    expect(screen.queryByLabelText(/Pending/)).not.toBeInTheDocument()
    expect(screen.getByText('Plain')).toBeVisible()
  })

  test('suppresses trend badges when no translator is supplied', () => {
    const { container } = render(
      <IntelligenceMetricGrid
        className="compact-metrics"
        items={[
          {
            label: 'Visits',
            value: '1,200',
            trend: { value: 1200, changePercent: 12, trend: 'up' },
          },
        ]}
      />,
    )

    expect(container.firstElementChild).toHaveClass('compact-metrics')
    expect(screen.queryByText('+12% ↑')).not.toBeInTheDocument()
    expect(container.querySelector('.trend-badge')).toBeNull()
  })
})
