/**
 * Coverage test for the paper-shell glyph renderer.
 *
 * ## Responsibilities
 * - Render every glyph in PK_GLYPH_NAMES so each path entry executes at least
 *   once (Object.keys order, no semantic assertions per glyph).
 * - Exercise the null fallback for an unknown icon name.
 * - Cover the size + strokeWidth + className prop branches.
 */

import { describe, expect, test } from 'vitest'
import { render } from '@testing-library/react'
import { PKGlyph, PK_GLYPH_NAMES, type GlyphIconName } from './pk-glyph'

describe('PKGlyph', () => {
  test('renders an svg with the expected defaults for every catalogued icon', () => {
    for (const icon of PK_GLYPH_NAMES) {
      const { container, unmount } = render(<PKGlyph icon={icon} />)
      const svg = container.querySelector('svg')
      expect(svg, `glyph ${icon}`).not.toBeNull()
      expect(svg?.getAttribute('width')).toBe('18')
      expect(svg?.getAttribute('height')).toBe('18')
      expect(svg?.getAttribute('stroke-width')).toBe('1.8')
      expect(svg?.querySelector('path,rect,circle')).not.toBeNull()
      unmount()
    }
  })

  test('returns null for an unknown icon name', () => {
    const { container } = render(
      <PKGlyph icon={'definitely_not_a_glyph' as GlyphIconName} />,
    )
    expect(container.querySelector('svg')).toBeNull()
  })

  test('honors custom size, strokeWidth, and className', () => {
    const { container } = render(
      <PKGlyph
        icon="search"
        size={32}
        strokeWidth={2.5}
        className="text-ink-strong"
      />,
    )
    const svg = container.querySelector('svg')
    expect(svg?.getAttribute('width')).toBe('32')
    expect(svg?.getAttribute('height')).toBe('32')
    expect(svg?.getAttribute('stroke-width')).toBe('2.5')
    expect(svg?.className.baseVal).toContain('text-ink-strong')
  })
})
