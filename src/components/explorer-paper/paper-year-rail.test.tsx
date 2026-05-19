/**
 * Tests for PaperYearRail — the right-edge 60-year scrubber.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { PaperYearRail } from './paper-year-rail'
import { pickYearJumpIso } from './paper-year-rail-helpers'

function makeBounds(
  overrides: Partial<{
    firstYear: number
    lastYear: number
    lastIso: string
  }> = {},
) {
  return {
    firstYear: 1990,
    lastYear: 2026,
    lastIso: '2026-05-17',
    ...overrides,
  }
}

function makeDensity(): Map<number, number> {
  const map = new Map<number, number>()
  map.set(2026, 180_000)
  map.set(2025, 70_000)
  map.set(2024, 30_000)
  map.set(2020, 4_000)
  map.set(2010, 0)
  return map
}

describe('PaperYearRail', () => {
  test('renders one cell per year between bounds, newest first', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2026-05-17"
        onJump={() => {}}
        testId="rail"
      />,
    )

    const rail = screen.getByTestId('rail')
    const cells = rail.querySelectorAll('[data-year]')
    // 1990..2026 inclusive = 37 cells
    expect(cells.length).toBe(37)
    expect(cells[0].getAttribute('data-year')).toBe('2026')
    expect(cells[cells.length - 1].getAttribute('data-year')).toBe('1990')
  })

  test('highlights the active year and renders a month indicator', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2024-07-15"
        onJump={() => {}}
        testId="rail-current"
      />,
    )

    const rail = screen.getByTestId('rail-current')
    const currentCell = rail.querySelector('[data-current="true"]')
    expect(currentCell?.getAttribute('data-year')).toBe('2024')
    expect(
      rail.querySelector('[data-testid="year-month-indicator"]'),
    ).not.toBeNull()
  })

  test('falls back to the last year when currentDate is unparseable', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="garbage"
        onJump={() => {}}
        testId="rail-garbage"
      />,
    )
    const rail = screen.getByTestId('rail-garbage')
    expect(
      rail.querySelector('[data-current="true"]')?.getAttribute('data-year'),
    ).toBe('2026')
  })

  test('clicking a year cell fires onJump with the pickYearJumpIso target', () => {
    const onJump = vi.fn()
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2026-05-17"
        onJump={onJump}
        testId="rail-jump"
      />,
    )

    const rail = screen.getByTestId('rail-jump')
    fireEvent.click(rail.querySelector('[data-year="2020"]') as HTMLElement)
    expect(onJump).toHaveBeenCalledWith('2020-06-15')

    fireEvent.click(rail.querySelector('[data-year="2026"]') as HTMLElement)
    expect(onJump).toHaveBeenCalledWith('2026-05-17')
  })

  test('marks decade boundaries with data-decade', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2026-05-17"
        onJump={() => {}}
        testId="rail-decade"
      />,
    )

    const rail = screen.getByTestId('rail-decade')
    const decades = rail.querySelectorAll('[data-decade="true"]')
    // 1990..2026 has four decade boundaries: 1990, 2000, 2010, 2020.
    expect(decades.length).toBe(4)
  })

  test('uses titleFor to label cells when provided', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2026-05-17"
        onJump={() => {}}
        titleFor={(year, count) => `Yr ${year}: ${count}p`}
        testId="rail-title"
      />,
    )

    const cell = screen
      .getByTestId('rail-title')
      .querySelector('[data-year="2024"]')
    expect(cell?.getAttribute('title')).toBe('Yr 2024: 30000p')
  })

  test('falls back to a deterministic label when titleFor is omitted', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2026-05-17"
        onJump={() => {}}
        testId="rail-default-title"
      />,
    )

    const cell = screen
      .getByTestId('rail-default-title')
      .querySelector('[data-year="2026"]')
    expect(cell?.getAttribute('title')).toBe('2026 · 180,000 pages')
  })

  test('every density tier gets its data-tier attribute', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2026-05-17"
        onJump={() => {}}
        testId="rail-tiers"
      />,
    )

    const rail = screen.getByTestId('rail-tiers')
    // periodDensityTier thresholds: <5k=t1, <30k=t2, <90k=t3, >=90k=t4.
    expect(
      rail.querySelector('[data-year="2026"]')?.getAttribute('data-tier'),
    ).toBe('t4') // 180k → t4
    expect(
      rail.querySelector('[data-year="2025"]')?.getAttribute('data-tier'),
    ).toBe('t3') // 70k → t3
    expect(
      rail.querySelector('[data-year="2024"]')?.getAttribute('data-tier'),
    ).toBe('t3') // 30k boundary → t3
    expect(
      rail.querySelector('[data-year="2020"]')?.getAttribute('data-tier'),
    ).toBe('t1') // 4k → t1
    expect(
      rail.querySelector('[data-year="2010"]')?.getAttribute('data-tier'),
    ).toBe('t0') // 0 → t0
  })

  test('respects a custom ariaLabel', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2026-05-17"
        onJump={() => {}}
        ariaLabel="Archive timeline"
      />,
    )

    expect(screen.getByLabelText('Archive timeline')).toBeInTheDocument()
  })

  test('clamps an out-of-range month index to zero', () => {
    render(
      <PaperYearRail
        densityByYear={makeDensity()}
        bounds={makeBounds()}
        currentDate="2024-99-15"
        onJump={() => {}}
        testId="rail-bad-month"
      />,
    )
    // Indicator should still be present, just positioned at top:0.
    const indicator = screen
      .getByTestId('rail-bad-month')
      .querySelector('[data-testid="year-month-indicator"]')
    expect(indicator).not.toBeNull()
    expect((indicator as HTMLElement).style.top).toBe('0%')
  })
})

describe('pickYearJumpIso', () => {
  test('returns the archive last ISO for the most recent year', () => {
    expect(
      pickYearJumpIso(2026, { lastYear: 2026, lastIso: '2026-05-17' }),
    ).toBe('2026-05-17')
  })

  test('returns mid-June for any older year', () => {
    expect(
      pickYearJumpIso(1990, { lastYear: 2026, lastIso: '2026-05-17' }),
    ).toBe('1990-06-15')
  })
})
