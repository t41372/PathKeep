/**
 * @file ai-step.test.tsx
 * @description Coverage for the REAL AI opt-in onboarding step.
 *
 * Proves: the honest enable/assistant explanation + skip hint render; "Enable" routes to onEnable,
 * "Not now" routes to onSkip, and Back routes back. The step itself never enables AI — it only
 * surfaces the two explicit choices and hands them back to the route owner.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { AiStep } from './ai-step'

function renderStep(
  overrides: Partial<{
    onEnable: () => void
    onSkip: () => void
    onBack: () => void
  }> = {},
) {
  const handlers = {
    onEnable: vi.fn(),
    onSkip: vi.fn(),
    onBack: vi.fn(),
    ...overrides,
  }
  render(
    <I18nProvider>
      <AiStep
        onEnable={handlers.onEnable}
        onSkip={handlers.onSkip}
        onBack={handlers.onBack}
      />
    </I18nProvider>,
  )
  return handlers
}

describe('AiStep', () => {
  test('renders the enable explanation, the separate-assistant note, and the skip hint', () => {
    renderStep()
    expect(screen.getByTestId('onboarding-ai-step')).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: 'Optional: on-device AI search' }),
    ).toBeInTheDocument()
    expect(screen.getByText('Enable local semantic search')).toBeInTheDocument()
    expect(screen.getByText('The AI assistant is separate')).toBeInTheDocument()
    expect(screen.getByTestId('onboarding-ai-skip-hint')).toHaveTextContent(
      'Skipping is completely fine',
    )
    // Both explicit choices are one click and clearly visible — nothing is pre-selected.
    expect(screen.getByTestId('onboarding-ai-enable')).toHaveTextContent(
      'Enable',
    )
    expect(screen.getByTestId('onboarding-ai-skip')).toHaveTextContent(
      'Not now',
    )
  })

  test('routes "Enable" to the enable handler', () => {
    const onEnable = vi.fn()
    renderStep({ onEnable })
    fireEvent.click(screen.getByTestId('onboarding-ai-enable'))
    expect(onEnable).toHaveBeenCalledTimes(1)
  })

  test('routes "Not now" to the skip handler so the flow advances with AI off', () => {
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
