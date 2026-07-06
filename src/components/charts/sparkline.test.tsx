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

  test('applies an independent paddingX inset while paddingY falls back to padding', () => {
    let received: { xScale: (index: number) => number } | null = null
    render(
      <Sparkline
        values={[0, 10]}
        ariaLabel="asymmetric padding"
        width={220}
        height={36}
        paddingX={10}
        testId="spark-padding-x"
      >
        {(args) => {
          received = args
          return null
        }}
      </Sparkline>,
    )
    // Horizontal inset follows paddingX (10), not the default padding (4).
    expect(received!.xScale(0)).toBe(10)
    expect(received!.xScale(1)).toBe(210)
    const line = screen.getByTestId('spark-padding-x').querySelector('polyline')
    // First plotted x sits at the paddingX inset.
    expect(line?.getAttribute('points')?.startsWith('10.0,')).toBe(true)
  })

  test('floors the value domain at minDomainMax so a near-zero series stays flat', () => {
    const { rerender } = render(
      <Sparkline
        values={[0.001, 0.002, 0.001]}
        ariaLabel="tiny series"
        height={40}
        padding={0}
        minDomainMax={0.01}
        testId="spark-min-domain"
      />,
    )
    const flatPoints = screen
      .getByTestId('spark-min-domain')
      .querySelector('polyline')
      ?.getAttribute('points')
    // With the 0.01 floor applied, the peak (0.002) stays well short of the
    // top (y=0) — its highest point only reaches y=32 out of a 40-tall chart.
    expect(flatPoints).toBe('0.0,36.0 100.0,32.0 200.0,36.0')

    rerender(
      <Sparkline
        values={[0.001, 0.002, 0.001]}
        ariaLabel="tiny series"
        height={40}
        padding={0}
        testId="spark-min-domain"
      />,
    )
    const stretchedPoints = screen
      .getByTestId('spark-min-domain')
      .querySelector('polyline')
      ?.getAttribute('points')
    // Without a floor, the same tiny peak auto-scales to the very top (y=0).
    expect(stretchedPoints).toBe('0.0,20.0 100.0,0.0 200.0,20.0')
  })

  test('renders an optional <title> description distinct from the aria-label', () => {
    render(
      <Sparkline
        values={[1, 2, 3]}
        ariaLabel="trend"
        description="Mean: 42%"
        testId="spark-desc"
      />,
    )
    const svg = screen.getByTestId('spark-desc')
    expect(svg.querySelector('title')?.textContent).toBe('Mean: 42%')
  })

  test('renders no <title> when description is omitted', () => {
    render(
      <Sparkline values={[1, 2, 3]} ariaLabel="trend" testId="spark-nodesc" />,
    )
    const svg = screen.getByTestId('spark-nodesc')
    expect(svg.querySelector('title')).toBeNull()
  })

  test('renders dot markers at the given series indices, with an optional custom radius', () => {
    render(
      <Sparkline
        values={[1, 2, 3, 4]}
        ariaLabel="marked trend"
        testId="spark-markers"
        markers={[{ index: 1 }, { index: 3, radius: 4 }]}
      />,
    )
    const svg = screen.getByTestId('spark-markers')
    const circles = svg.querySelectorAll('circle')
    expect(circles).toHaveLength(2)
    expect(circles[0]).toHaveAttribute('r', '2') // default radius
    expect(circles[1]).toHaveAttribute('r', '4') // custom radius
  })

  test('skips a marker whose index has no corresponding point', () => {
    render(
      <Sparkline
        values={[1, 2]}
        ariaLabel="marked trend"
        testId="spark-marker-oob"
        markers={[{ index: 99 }]}
      />,
    )
    const svg = screen.getByTestId('spark-marker-oob')
    expect(svg.querySelectorAll('circle')).toHaveLength(0)
  })

  test('renders no markers when omitted', () => {
    render(
      <Sparkline values={[1, 2]} ariaLabel="trend" testId="spark-no-markers" />,
    )
    expect(
      screen.getByTestId('spark-no-markers').querySelectorAll('circle'),
    ).toHaveLength(0)
  })

  test('renders vertical gridlines at the given series indices', () => {
    render(
      <Sparkline
        values={[1, 2, 3, 4]}
        ariaLabel="gridded trend"
        testId="spark-gridlines"
        gridlines={[{ index: 0 }, { index: 2 }]}
      />,
    )
    const svg = screen.getByTestId('spark-gridlines')
    const lines = svg.querySelectorAll('line')
    expect(lines).toHaveLength(2)
    expect(lines[0]).toHaveAttribute('y1', '0')
    expect(lines[0].getAttribute('class')).toContain('stroke-border-light')
  })

  test('renders no gridlines when omitted', () => {
    render(
      <Sparkline
        values={[1, 2]}
        ariaLabel="trend"
        testId="spark-no-gridlines"
      />,
    )
    expect(
      screen.getByTestId('spark-no-gridlines').querySelectorAll('line'),
    ).toHaveLength(0)
  })

  test('renders a dashed horizontal reference line at the given value', () => {
    render(
      <Sparkline
        values={[1, 4, 2, 8]}
        ariaLabel="referenced trend"
        testId="spark-reference"
        referenceValue={3.75}
      />,
    )
    const svg = screen.getByTestId('spark-reference')
    const line = svg.querySelector('line')
    expect(line).not.toBeNull()
    expect(line).toHaveAttribute('stroke-dasharray', '4 3')
  })

  test('renders no reference line when referenceValue is omitted', () => {
    render(
      <Sparkline
        values={[1, 2]}
        ariaLabel="trend"
        testId="spark-no-reference"
      />,
    )
    expect(
      screen.getByTestId('spark-no-reference').querySelector('line'),
    ).toBeNull()
  })

  test('invokes the children render prop with the same points + scales the line/area use', () => {
    let received: {
      points: { x: number; y: number }[]
      xScale: (index: number) => number
      yScale: (value: number) => number
      width: number
      height: number
      padding: number
    } | null = null
    render(
      <Sparkline
        values={[0, 10]}
        ariaLabel="escape hatch"
        testId="spark-children"
        width={100}
        height={40}
        padding={5}
      >
        {(args) => {
          received = args
          return <rect data-testid="custom-adornment" x={0} y={0} />
        }}
      </Sparkline>,
    )
    expect(screen.getByTestId('custom-adornment')).toBeInTheDocument()
    expect(received).not.toBeNull()
    expect(received!.points).toHaveLength(2)
    expect(received!.width).toBe(100)
    expect(received!.height).toBe(40)
    expect(received!.padding).toBe(5)
    expect(received!.xScale(0)).toBe(5)
    expect(received!.yScale(10)).toBe(5)
  })

  test('renders nothing extra when children is omitted', () => {
    render(
      <Sparkline
        values={[1, 2]}
        ariaLabel="trend"
        testId="spark-no-children"
      />,
    )
    expect(
      screen.getByTestId('spark-no-children').querySelector('rect'),
    ).toBeNull()
  })
})
