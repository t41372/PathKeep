/**
 * This test file protects the favicon primitive used by Explorer history rows.
 *
 * Why this file exists:
 * - Stored favicon payloads should render when present, and broken or missing payloads should fall back to the deterministic placeholder instead of collapsing the layout.
 * - Keeping these assertions close to the primitive makes route regressions cheaper to catch.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - The fallback contract follows `docs/features/recall.md`.
 * - Tests should verify behavior, not styling internals beyond the classes that communicate the trust state.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { HistoryFavicon } from './history-favicon'

describe('HistoryFavicon', () => {
  test('renders the stored favicon payload when one is available', () => {
    render(
      <HistoryFavicon
        domain="example.com"
        favicon={{ dataUrl: 'data:image/png;base64,AQI=' }}
      />,
    )

    expect(document.querySelector('.favicon-image')).toHaveAttribute(
      'src',
      'data:image/png;base64,AQI=',
    )
    expect(document.querySelector('.favicon-placeholder')).toHaveClass(
      'has-image',
    )
  })

  test('falls back to the domain initial when no favicon payload exists', () => {
    render(<HistoryFavicon domain="example.com" favicon={null} />)

    expect(screen.getByText('E')).toBeInTheDocument()
    expect(screen.queryByRole('img', { hidden: true })).not.toBeInTheDocument()
    expect(document.querySelector('.favicon-placeholder')).toHaveAttribute(
      'class',
      'favicon-placeholder ',
    )
  })

  test('falls back to the placeholder when the image fails to load', () => {
    render(
      <HistoryFavicon
        domain="example.com"
        favicon={{ dataUrl: 'data:image/png;base64,AQI=' }}
      />,
    )

    const image = document.querySelector('.favicon-image')
    expect(image).not.toBeNull()
    fireEvent.error(image as Element)

    expect(screen.getByText('E')).toBeInTheDocument()
    expect(document.querySelector('.favicon-placeholder')).not.toHaveClass(
      'has-image',
    )
  })

  test('uses the generic fallback when no domain initial is available', () => {
    render(<HistoryFavicon domain="   " favicon={null} />)

    expect(screen.getByText('?')).toBeInTheDocument()
  })

  test('uses the generic fallback when the domain is missing entirely', () => {
    render(<HistoryFavicon favicon={null} />)

    expect(screen.getByText('?')).toBeInTheDocument()
  })
})
