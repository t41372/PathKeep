/**
 * @file hub-layout.test.tsx
 * @description Unit tests for the newspaper hub layout primitives.
 * @module pages/intelligence/sections
 *
 * ## Responsibilities
 * - Verify AxisCard renders title, "See all" CTA, and children.
 * - Verify SpotlightCard renders accent card with sentence.
 *
 * ## Not responsible for
 * - Testing the full sections coordinator (covered in sections.test.tsx).
 * - Testing lazy secondary-section mounting (covered in lazy-section.test.tsx).
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { AxisCard, SpotlightCard } from './hub-layout'

describe('AxisCard', () => {
  test('renders title, children, and see-all badge', () => {
    const onSeeAll = vi.fn()
    render(
      <AxisCard
        title="Time"
        seeAllLabel="See all"
        onSeeAll={onSeeAll}
        testId="test-axis"
      >
        <p>Preview content</p>
      </AxisCard>,
    )

    expect(screen.getByText('Time')).toBeInTheDocument()
    expect(screen.getByText('Preview content')).toBeInTheDocument()
    expect(screen.getByTestId('test-axis')).toBeInTheDocument()
  })

  test('calls onSeeAll when badge is clicked', async () => {
    const user = userEvent.setup()
    const onSeeAll = vi.fn()
    render(
      <AxisCard
        title="Sources"
        seeAllLabel="See all"
        onSeeAll={onSeeAll}
        testId="test-axis"
      >
        <p>Content</p>
      </AxisCard>,
    )

    await user.click(screen.getByRole('button'))
    expect(onSeeAll).toHaveBeenCalledOnce()
  })
})

describe('SpotlightCard', () => {
  test('renders sentence in an accent card', () => {
    render(
      <SpotlightCard sentence="Your most active day was Tuesday with 142 visits." />,
    )

    expect(
      screen.getByText('Your most active day was Tuesday with 142 visits.'),
    ).toBeInTheDocument()
    expect(screen.getByTestId('hub-spotlight')).toBeInTheDocument()
  })

  test('renders nothing when sentence is null', () => {
    const { container } = render(<SpotlightCard sentence={null} />)
    expect(container.innerHTML).toBe('')
  })
})
