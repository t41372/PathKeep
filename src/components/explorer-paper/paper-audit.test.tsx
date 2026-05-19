/**
 * Tests for the Audit primitives — chain block + storage breakdown bar.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { PaperChainBlock, PaperStorageBar } from './index'

describe('PaperChainBlock', () => {
  test('renders id, hash, type, and when', () => {
    render(
      <PaperChainBlock
        id="#1847"
        hash="0a4c…ef82"
        type="BACKUP"
        when="2h ago"
        testId="chain"
      />,
    )

    expect(screen.getByText('#1847')).toBeVisible()
    expect(screen.getByText('0a4c…ef82')).toBeVisible()
    expect(screen.getByText('BACKUP')).toBeVisible()
    expect(screen.getByText('2h ago')).toBeVisible()
  })

  test('marks current and forwards click', () => {
    const onClick = vi.fn()
    render(
      <PaperChainBlock
        id="#1847"
        hash="0a4c…ef82"
        current
        onClick={onClick}
        testId="chain-current"
      />,
    )

    const block = screen.getByTestId('chain-current')
    expect(block.dataset.current).toBe('true')
    fireEvent.click(block)
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  test('disabled when no handler is supplied', () => {
    render(<PaperChainBlock id="#1846" hash="8b71…d3a9" testId="chain-bare" />)
    expect(screen.getByTestId<HTMLButtonElement>('chain-bare').disabled).toBe(
      true,
    )
  })

  test('omits the type/when row when both are missing', () => {
    render(
      <PaperChainBlock id="#1845" hash="4e29…91c7" testId="chain-no-meta" />,
    )
    expect(screen.queryByText(/·/)).toBeNull()
  })
})

describe('PaperStorageBar', () => {
  test('renders label, size, and a bar at the requested percentage', () => {
    const { container } = render(
      <PaperStorageBar
        label="Core archive · pages + visits"
        size="8.2 GB"
        pct={66}
        tone="primary"
        testId="storage"
      />,
    )

    expect(screen.getByText('Core archive · pages + visits')).toBeVisible()
    expect(screen.getByText('8.2 GB')).toBeVisible()
    const fill = container.querySelector('span[style*="width"]') as HTMLElement
    expect(fill.style.width).toBe('66%')
  })

  test('clamps a percentage out of range to 0..100', () => {
    const { container, rerender } = render(
      <PaperStorageBar label="A" size="1 GB" pct={150} tone="primary" />,
    )
    expect(
      (container.querySelector('span[style*="width"]') as HTMLElement).style
        .width,
    ).toBe('100%')

    rerender(<PaperStorageBar label="A" size="0 GB" pct={-20} tone="primary" />)
    expect(
      (container.querySelector('span[style*="width"]') as HTMLElement).style
        .width,
    ).toBe('0%')
  })

  test('exposes the tone via data-tone', () => {
    render(
      <PaperStorageBar
        label="Snapshots"
        size="0.8 GB"
        pct={6}
        tone="muted"
        testId="storage-tone"
      />,
    )
    expect(screen.getByTestId('storage-tone').dataset.tone).toBe('muted')
  })

  test('every tone renders without throwing', () => {
    for (const tone of ['primary', 'secondary', 'tertiary', 'muted'] as const) {
      const { unmount } = render(
        <PaperStorageBar
          label="row"
          size="1 GB"
          pct={50}
          tone={tone}
          testId={`storage-${tone}`}
        />,
      )
      expect(screen.getByTestId(`storage-${tone}`).dataset.tone).toBe(tone)
      unmount()
    }
  })
})
