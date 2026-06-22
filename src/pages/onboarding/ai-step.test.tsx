/**
 * @file ai-step.test.tsx
 * @description Coverage for the optional AI-setup onboarding step.
 *
 * Proves: the calm trust bullets + skip hint render; "Set up AI in Settings" and "Skip for now"
 * route to their handlers; and Back routes back. The step is presentational and never enables AI.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { AiStep } from './ai-step'

function renderStep(
  overrides: Partial<{
    onSetUpAi: () => void
    onSkip: () => void
    onBack: () => void
  }> = {},
) {
  const handlers = {
    onSetUpAi: vi.fn(),
    onSkip: vi.fn(),
    onBack: vi.fn(),
    ...overrides,
  }
  render(
    <I18nProvider>
      <AiStep
        onSetUpAi={handlers.onSetUpAi}
        onSkip={handlers.onSkip}
        onBack={handlers.onBack}
      />
    </I18nProvider>,
  )
  return handlers
}

describe('AiStep', () => {
  test('renders the optional/off-by-default/local-first reassurance and the skip hint', () => {
    renderStep()
    expect(screen.getByTestId('onboarding-ai-step')).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: 'Optional: AI features' }),
    ).toBeInTheDocument()
    expect(screen.getByText('Off by default')).toBeInTheDocument()
    expect(screen.getByText('Local-first')).toBeInTheDocument()
    expect(screen.getByText('Always cited')).toBeInTheDocument()
    expect(screen.getByTestId('onboarding-ai-skip-hint')).toHaveTextContent(
      'Skipping is completely fine',
    )
  })

  test('routes "Set up AI in Settings" to the deep-link handler', () => {
    const onSetUpAi = vi.fn()
    renderStep({ onSetUpAi })
    fireEvent.click(screen.getByTestId('onboarding-ai-setup'))
    expect(onSetUpAi).toHaveBeenCalledTimes(1)
  })

  test('routes "Skip for now" to the skip handler so the flow advances', () => {
    const onSkip = vi.fn()
    renderStep({ onSkip })
    fireEvent.click(screen.getByTestId('onboarding-ai-skip'))
    expect(onSkip).toHaveBeenCalledTimes(1)
  })

  test('routes Back to the back handler', () => {
    const onBack = vi.fn()
    renderStep({ onBack })
    fireEvent.click(screen.getByRole('button', { name: '← Back' }))
    expect(onBack).toHaveBeenCalledTimes(1)
  })
})
