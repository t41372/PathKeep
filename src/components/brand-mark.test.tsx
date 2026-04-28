/**
 * @file brand-mark.test.tsx
 * @description Guards the shared PathKeep brand mark props.
 * @module components
 *
 * ## Responsibilities
 * - Verify the default accessible label and base class.
 * - Verify callers can append a route-specific class without losing the base class.
 *
 * ## Not responsible for
 * - Validating the SVG asset contents.
 *
 * ## Dependencies
 * - Depends only on the shared brand mark component.
 *
 * ## Performance notes
 * - Static render assertions only.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { BrandMark } from './brand-mark'

describe('BrandMark', () => {
  test('renders default and custom class variants', () => {
    render(
      <>
        <BrandMark />
        <BrandMark alt="Custom mark" className="welcome-mark" />
      </>,
    )

    expect(screen.getByAltText('PathKeep')).toHaveClass('brand-mark')
    expect(screen.getByAltText('PathKeep')).toHaveAttribute(
      'class',
      'brand-mark',
    )
    expect(screen.getByAltText('Custom mark')).toHaveClass(
      'brand-mark',
      'welcome-mark',
    )
  })
})
