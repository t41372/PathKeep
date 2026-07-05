/**
 * @file ai-search-tuning-section.test.tsx
 * @description Component coverage for the hybrid-search tuning disclosure (W-AI-9 / W-AI-6).
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify each knob renders its value, slider, and number input.
 * - Verify slider/input edits raise `onChange` with the right knob and the raw
 *   value (the route handler clamps; the control passes `valueAsNumber`).
 * - Verify the disabled (AI-off / saving) state freezes every control + reset.
 * - Verify the reset affordance is gated on drift and raises `onReset`.
 * - Verify the null-draft guard returns nothing.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { AiSettings } from '../../lib/types'
import { AiSearchTuningSection } from './ai-search-tuning-section'
import type { SearchTuningKnob } from './search-tuning-helpers'

function settingsFixture(overrides: Partial<AiSettings> = {}): AiSettings {
  return {
    enabled: true,
    assistantEnabled: false,
    semanticIndexEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: true,
    enrichmentPlugins: [],
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt: '',
    llmProviders: [],
    embeddingProviders: [],
    ...overrides,
  }
}

function renderSection(props: {
  settings?: AiSettings | null
  disabled?: boolean
  onChange?: (knob: SearchTuningKnob, value: number) => void
  onReset?: () => void
}) {
  const onChange = vi.fn(props.onChange)
  const onReset = vi.fn(props.onReset)
  render(
    <I18nProvider>
      <AiSearchTuningSection
        settings={
          props.settings === undefined ? settingsFixture() : props.settings
        }
        disabled={props.disabled ?? false}
        onChange={onChange}
        onReset={onReset}
      />
    </I18nProvider>,
  )
  return { onChange, onReset }
}

describe('AiSearchTuningSection', () => {
  test('returns nothing when there is no draft', () => {
    const { container } = render(
      <I18nProvider>
        <AiSearchTuningSection
          settings={null}
          disabled={false}
          onChange={vi.fn()}
          onReset={vi.fn()}
        />
      </I18nProvider>,
    )
    expect(container).toBeEmptyDOMElement()
  })

  test('renders every knob with its resolved value', () => {
    renderSection({})
    expect(screen.getByTestId('ai-search-tuning')).toBeInTheDocument()
    // Defaults: 60 / 1.0 / 1.0 / 0.15.
    expect(
      screen.getByTestId('ai-search-tuning-hybridRrfK-value').textContent,
    ).toBe('60')
    expect(
      screen.getByTestId('ai-search-tuning-lexicalWeight-value').textContent,
    ).toBe('1.0')
    expect(
      screen.getByTestId('ai-search-tuning-semanticWeight-value').textContent,
    ).toBe('1.0')
    expect(
      screen.getByTestId('ai-search-tuning-starredBoost-value').textContent,
    ).toBe('0.15')
  })

  test('a slider edit raises onChange with the knob and raw value', () => {
    const { onChange } = renderSection({})
    fireEvent.change(
      screen.getByTestId('ai-search-tuning-lexicalWeight-slider'),
      {
        target: { value: '2.5' },
      },
    )
    expect(onChange).toHaveBeenCalledWith('lexicalWeight', 2.5)
  })

  test('a number-input edit raises onChange with the knob and raw value', () => {
    const { onChange } = renderSection({})
    fireEvent.change(screen.getByTestId('ai-search-tuning-hybridRrfK-input'), {
      target: { value: '80' },
    })
    expect(onChange).toHaveBeenCalledWith('hybridRrfK', 80)
  })

  test('an emptied number input raises onChange with NaN (handler resets it)', () => {
    const { onChange } = renderSection({})
    fireEvent.change(
      screen.getByTestId('ai-search-tuning-starredBoost-input'),
      {
        target: { value: '' },
      },
    )
    expect(onChange).toHaveBeenCalledTimes(1)
    const [knob, value] = onChange.mock.calls[0]
    expect(knob).toBe('starredBoost')
    expect(Number.isNaN(value)).toBe(true)
  })

  test('disables every control and the reset button when AI is off / saving', () => {
    const { onChange, onReset } = renderSection({ disabled: true })
    expect(
      screen.getByTestId('ai-search-tuning-hybridRrfK-slider'),
    ).toBeDisabled()
    expect(
      screen.getByTestId('ai-search-tuning-starredBoost-input'),
    ).toBeDisabled()
    const reset = screen.getByTestId('ai-search-tuning-reset')
    expect(reset).toBeDisabled()
    fireEvent.click(reset)
    expect(onReset).not.toHaveBeenCalled()
    expect(onChange).not.toHaveBeenCalled()
  })

  test('keeps reset disabled on defaults and enables it once a knob drifts', () => {
    renderSection({})
    expect(screen.getByTestId('ai-search-tuning-reset')).toBeDisabled()
  })

  test('enables reset on drift and raises onReset on click', () => {
    const { onReset } = renderSection({
      settings: settingsFixture({ starredBoost: 0.4 }),
    })
    const reset = screen.getByTestId('ai-search-tuning-reset')
    expect(reset).toBeEnabled()
    fireEvent.click(reset)
    expect(onReset).toHaveBeenCalledTimes(1)
  })
})
