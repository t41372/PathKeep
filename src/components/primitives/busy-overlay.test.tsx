import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { BusyOverlay } from './busy-overlay'

describe('BusyOverlay', () => {
  test('renders the primary status label on its own', () => {
    render(<BusyOverlay label="Running backup" />)

    expect(screen.getByRole('status')).toHaveTextContent('Running backup')
  })

  test('renders detail text and step progress when provided', () => {
    render(
      <BusyOverlay
        label="Writing archive facts"
        detail="Large real-world profiles can take a while here."
        steps={[
          'Inspect selected browser profiles',
          'Write the canonical archive run',
          'Refresh dashboard and shell state',
        ]}
        activeStep={1}
      />,
    )

    expect(screen.getByText('Writing archive facts')).toBeVisible()
    expect(
      screen.getByText('Large real-world profiles can take a while here.'),
    ).toBeVisible()
    expect(
      screen
        .getByText('Inspect selected browser profiles')
        .closest('.busy-overlay__step'),
    ).toHaveClass('busy-overlay__step--done')
    expect(
      screen
        .getByText('Write the canonical archive run')
        .closest('.busy-overlay__step'),
    ).toHaveClass('busy-overlay__step--active')
    expect(
      screen
        .getByText('Refresh dashboard and shell state')
        .closest('.busy-overlay__step'),
    ).toHaveClass('busy-overlay__step--pending')
  })
})
