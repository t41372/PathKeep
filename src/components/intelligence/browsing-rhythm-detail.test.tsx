/**
 * @file browsing-rhythm-detail.test.tsx
 * @description Focused render coverage for browsing-rhythm detail visualizers.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Verify hourly heat levels cover each intensity threshold.
 * - Verify activity proportions sort by share and use localized number formatting.
 *
 * ## Not responsible for
 * - Re-testing route-level day-insights composition.
 * - Re-testing Core Intelligence API readers.
 *
 * ## Dependencies
 * - Pure render tests for shared Intelligence visual components.
 *
 * ## Performance notes
 * - Uses 24 tiny hourly buckets and no async work.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import {
  RhythmActivityProportionBar,
  RhythmHourStrip,
} from './browsing-rhythm-detail'

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('browsing rhythm detail visualizers', () => {
  test('assigns all hourly heat levels from empty through peak activity', () => {
    render(
      <RhythmHourStrip
        date="2026-04-25"
        hourly={[
          { hour: 0, visitCount: 0 },
          { hour: 1, visitCount: 1 },
          { hour: 2, visitCount: 3 },
          { hour: 3, visitCount: 5 },
          { hour: 4, visitCount: 8 },
        ]}
        t={t}
      />,
    )

    const strip = screen.getByTestId('rhythm-hour-strip')
    expect(strip).toHaveAttribute(
      'aria-label',
      'rhythmHourStripLabel:{"date":"2026-04-25"}',
    )
    const cells = Array.from(strip.querySelectorAll('[data-level]'))
    const levels = cells.map((cell) => cell.getAttribute('data-level'))

    expect(cells).toHaveLength(24)
    expect(levels).toEqual(expect.arrayContaining(['0', '1', '2', '3', '4']))
    expect(cells[0]).toHaveAttribute(
      'title',
      'rhythmHourTooltip:{"hour":"00:00-01:00","count":0}',
    )
    expect(cells[23]).toHaveAttribute(
      'title',
      'rhythmHourTooltip:{"hour":"23:00-00:00","count":0}',
    )
    expect(
      Array.from(
        strip.querySelectorAll('.rhythm-distribution__labels span'),
      ).map((label) => label.textContent),
    ).toEqual(['0', '6', '12', '18', '23'])
  })

  test('keeps heat threshold boundaries and hourly rerenders observable', () => {
    const view = render(
      <RhythmHourStrip
        date="2026-04-25"
        hourly={[
          { hour: 0, visitCount: 0 },
          { hour: 1, visitCount: 1 },
          { hour: 2, visitCount: 2 },
          { hour: 3, visitCount: 3 },
          { hour: 4, visitCount: 4 },
        ]}
        t={t}
      />,
    )

    const firstLevels = Array.from(
      screen.getByTestId('rhythm-hour-strip').querySelectorAll('[data-level]'),
    )
      .slice(0, 5)
      .map((cell) => cell.getAttribute('data-level'))

    expect(firstLevels).toEqual(['0', '2', '3', '4', '4'])

    view.rerender(
      <RhythmHourStrip
        date="2026-04-26"
        hourly={[
          { hour: 0, visitCount: 4 },
          { hour: 1, visitCount: 0 },
        ]}
        t={t}
      />,
    )

    const rerenderedCells = Array.from(
      screen.getByTestId('rhythm-hour-strip').querySelectorAll('[data-level]'),
    )
    expect(rerenderedCells[0]).toHaveAttribute('data-level', '4')
    expect(rerenderedCells[1]).toHaveAttribute('data-level', '0')
    expect(screen.getByTestId('rhythm-hour-strip')).toHaveAttribute(
      'aria-label',
      'rhythmHourStripLabel:{"date":"2026-04-26"}',
    )
  })

  test('renders sorted activity proportions with localized counts', () => {
    const view = render(
      <RhythmActivityProportionBar
        categories={[
          { domainCategory: 'docs', visitCount: 12, share: 0.4 },
          { domainCategory: 'video', visitCount: 18, share: 0.6 },
        ]}
        categoryLabel={(category) => `category:${category}`}
        language="en"
        t={t}
      />,
    )

    const bar = screen.getByTestId('rhythm-activity-proportion')
    const rows = screen.getAllByText(/^category:/)
    expect(rows.map((row) => row.textContent)).toEqual([
      'category:video',
      'category:docs',
    ])
    const segments = Array.from(
      bar.querySelectorAll(
        '.rhythm-proportion__bar .rhythm-proportion__segment',
      ),
    )
    expect(segments).toHaveLength(2)
    expect(segments[0]).toHaveAttribute('data-category', 'video')
    expect(segments[0]).toHaveStyle({ width: '60%' })
    expect(segments[1]).toHaveAttribute('data-category', 'docs')
    expect(segments[1]).toHaveStyle({ width: '40%' })
    expect(screen.getByText('60%')).toBeVisible()
    expect(screen.getByText('40%')).toBeVisible()
    expect(screen.getByText('18 visits')).toBeVisible()

    view.rerender(
      <RhythmActivityProportionBar
        categories={[
          { domainCategory: 'search', visitCount: 1_234, share: 0.25 },
          { domainCategory: 'docs', visitCount: 3_210, share: 0.75 },
        ]}
        categoryLabel={(category) => `category:${category}`}
        language="en"
        t={t}
      />,
    )

    expect(
      screen.getAllByText(/^category:/).map((row) => row.textContent),
    ).toEqual(['category:docs', 'category:search'])
    expect(screen.getByText('3,210 visits')).toBeVisible()
    expect(screen.getByText('75%')).toBeVisible()
    expect(screen.getByText('25%')).toBeVisible()
  })
})
