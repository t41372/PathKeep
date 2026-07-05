import { describe, expect, test } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Sparkline } from './sparkline'

describe('Sparkline', () => {
  test('renders role=img with the caller-supplied aria-label', () => {
    render(<Sparkline values={[1, 4, 2, 8]} ariaLabel="24-hour activity" />)
    expect(
      screen.getByRole('img', { name: '24-hour activity' }),
    ).toBeInTheDocument()
  })

  test('renders a line + area path from 2+ points', () => {
    render(<Sparkline values={[1, 4, 2, 8]} ariaLabel="trend" testId="spark" />)
    const svg = screen.getByTestId('spark')
    const polyline = svg.querySelector('polyline')
    const path = svg.querySelector('path')
    expect(polyline).not.toBeNull()
    expect(polyline?.getAttribute('points')).not.toBe('')
    expect(path).not.toBeNull()
    expect(path?.getAttribute('d')).not.toBe('')
  })

  test('degrades gracefully for an empty series (no crash, no line)', () => {
    render(
      <Sparkline values={[]} ariaLabel="empty trend" testId="spark-empty" />,
    )
    const svg = screen.getByTestId('spark-empty')
    expect(svg.querySelector('polyline')).toBeNull()
    expect(svg.querySelector('path')).toBeNull()
  })

  test('degrades gracefully for a single point (no crash, no line)', () => {
    render(
      <Sparkline values={[5]} ariaLabel="single point" testId="spark-single" />,
    )
    const svg = screen.getByTestId('spark-single')
    expect(svg.querySelector('polyline')).toBeNull()
    expect(svg.querySelector('path')).toBeNull()
  })

  test('renders optional tick labels at their series index position', () => {
    render(
      <Sparkline
        values={[0, 1, 2, 3]}
        ariaLabel="ticked trend"
        testId="spark-ticks"
        ticks={[
          { index: 0, label: '0' },
          { index: 3, label: '23' },
        ]}
      />,
    )
    const svg = screen.getByTestId('spark-ticks')
    expect(svg.textContent).toContain('0')
    expect(svg.textContent).toContain('23')
  })

  test('renders no tick text elements when ticks are omitted', () => {
    render(
      <Sparkline
        values={[1, 2]}
        ariaLabel="no ticks"
        testId="spark-no-ticks"
      />,
    )
    const svg = screen.getByTestId('spark-no-ticks')
    expect(svg.querySelectorAll('text')).toHaveLength(0)
  })

  test('accepts custom width/height/padding', () => {
    render(
      <Sparkline
        values={[1, 2, 3]}
        ariaLabel="custom size"
        width={100}
        height={20}
        padding={2}
        testId="spark-sized"
      />,
    )
    const svg = screen.getByTestId('spark-sized')
    expect(svg).toHaveAttribute('viewBox', '0 0 100 20')
  })
})
