/**
 * This test file protects the shared UI glyph registry.
 *
 * Why this file exists:
 * - Route chrome and icon buttons depend on named glyphs resolving to visible SVG paths.
 * - A missing glyph silently degrades many surfaces, so the registry needs a small direct smoke test.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions on SVG presence and stable icon names rather than exact vector paths.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { DataRow, FieldBlock, Glyph, Surface, ToggleRow } from './ui'

describe('Glyph', () => {
  test('renders copy and back-arrow glyphs from the shared registry', () => {
    render(
      <>
        <span aria-label="copy icon">
          <Glyph icon="content_copy" />
        </span>
        <span aria-label="back icon">
          <Glyph icon="arrow_back" />
        </span>
      </>,
    )

    expect(
      screen.getByLabelText('copy icon').querySelector('svg'),
    ).not.toBeNull()
    expect(
      screen.getByLabelText('back icon').querySelector('svg'),
    ).not.toBeNull()
  })

  test('renders the settings glyph as a cog, not a sunburst', () => {
    const { container } = render(<Glyph icon="settings" />)
    const pathData = Array.from(container.querySelectorAll('path')).map(
      (path) => path.getAttribute('d'),
    )
    const cogBody = pathData.find((path) => path?.startsWith('M12.2 2h-.4'))

    expect(cogBody).toContain('a2 2 0 0 0 2 2h.4')
    expect(pathData).not.toContain('M12 3.5v2.2')
  })
})

describe('shared shell UI primitives', () => {
  test('renders surface headers with and without optional title actions', () => {
    render(
      <>
        <Surface
          actions={<button type="button">Refresh</button>}
          eyebrow="Archive"
          icon="content_copy"
          title="Snapshot"
        >
          <p>Surface body</p>
        </Surface>
        <Surface eyebrow="Quiet" icon="arrow_back" title="">
          <p>Plain body</p>
        </Surface>
      </>,
    )

    expect(screen.getByRole('heading', { name: 'Snapshot' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeVisible()
    expect(screen.queryByRole('heading', { name: '' })).not.toBeInTheDocument()
    expect(screen.getByText('Plain body')).toBeVisible()
  })

  test('renders field and data rows from explicit controls or children', () => {
    render(
      <>
        <FieldBlock
          control={<input aria-label="Retention days" />}
          label="Retention"
        />
        <FieldBlock label="Archive note">
          <textarea aria-label="Archive note input" />
        </FieldBlock>
        <DataRow label="Profile" value="Chrome" />
        <DataRow label="Fallback">
          <span>Archive wide</span>
        </DataRow>
      </>,
    )

    expect(screen.getByLabelText('Retention days')).toBeVisible()
    expect(screen.getByLabelText('Archive note input')).toBeVisible()
    expect(screen.getByText('Chrome')).toBeVisible()
    expect(screen.getByText('Archive wide')).toBeVisible()
  })

  test('emits toggle row state changes from the checkbox event target', () => {
    const onChange = vi.fn()
    render(
      <ToggleRow checked={false} label="Enable sync" onChange={onChange} />,
    )

    fireEvent.click(screen.getByRole('checkbox', { name: 'Enable sync' }))

    expect(onChange).toHaveBeenCalledWith(true)
  })
})
