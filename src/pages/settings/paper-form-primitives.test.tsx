/**
 * Coverage for the shared form primitives used by Settings sections.
 *
 * Most behaviour (label / help / textarea wiring) is exercised end-to-end
 * by each section's test file. This file focuses on the contracts that
 * are shared and worth pinning here so a single regression doesn't ripple
 * across every section: the SegmentedControl `disabled` prop semantics in
 * particular — the link-previews section relies on it to lock the fetch
 * mode picker when the master kill switch is off.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { SegmentedControl } from './paper-form-primitives'

const OPTIONS = [
  { id: 'off' as const, label: 'Off', hint: 'no fetching' },
  { id: 'on_demand' as const, label: 'On demand', hint: 'when scrolled' },
  { id: 'background' as const, label: 'Background', hint: 'pre-warm' },
]

describe('SegmentedControl', () => {
  test('renders each option with label, hint, and the correct radio role', () => {
    const onChange = vi.fn()
    render(
      <SegmentedControl
        options={OPTIONS}
        value="background"
        onChange={onChange}
        testId="seg"
      />,
    )
    for (const option of OPTIONS) {
      const node = screen.getByTestId(`seg-${option.id}`)
      expect(node).toBeInTheDocument()
      expect(node.getAttribute('role')).toBe('radio')
      expect(node.textContent).toContain(option.label)
      if (option.hint) {
        expect(node.textContent).toContain(option.hint)
      }
    }
  })

  test('marks aria-checked=true on the selected option only', () => {
    const onChange = vi.fn()
    render(
      <SegmentedControl
        options={OPTIONS}
        value="on_demand"
        onChange={onChange}
        testId="seg"
      />,
    )
    expect(
      screen.getByTestId('seg-on_demand').getAttribute('aria-checked'),
    ).toBe('true')
    expect(screen.getByTestId('seg-off').getAttribute('aria-checked')).toBe(
      'false',
    )
    expect(
      screen.getByTestId('seg-background').getAttribute('aria-checked'),
    ).toBe('false')
  })

  test('clicking a non-selected option fires onChange with that id', () => {
    const onChange = vi.fn()
    render(
      <SegmentedControl
        options={OPTIONS}
        value="off"
        onChange={onChange}
        testId="seg"
      />,
    )
    fireEvent.click(screen.getByTestId('seg-background'))
    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith('background')
  })

  test('disabled=true sets the disabled attribute on every option', () => {
    const onChange = vi.fn()
    render(
      <SegmentedControl
        options={OPTIONS}
        value="background"
        onChange={onChange}
        disabled
        testId="seg"
      />,
    )
    for (const option of OPTIONS) {
      expect(screen.getByTestId(`seg-${option.id}`)).toBeDisabled()
    }
  })

  test('disabled prevents onChange from firing on click', () => {
    const onChange = vi.fn()
    render(
      <SegmentedControl
        options={OPTIONS}
        value="background"
        onChange={onChange}
        disabled
        testId="seg"
      />,
    )
    // jsdom honours the `disabled` attribute on a button: click events
    // are suppressed at the browser level, so onChange must never fire
    // — pinning this behaviour matters because the link-previews
    // section relies on it as the policy gate when the master fetch
    // toggle is off.
    fireEvent.click(screen.getByTestId('seg-off'))
    fireEvent.click(screen.getByTestId('seg-on_demand'))
    expect(onChange).not.toHaveBeenCalled()
  })

  test('falsy disabled (default) keeps options interactive', () => {
    const onChange = vi.fn()
    render(
      <SegmentedControl
        options={OPTIONS}
        value="off"
        onChange={onChange}
        testId="seg"
      />,
    )
    for (const option of OPTIONS) {
      expect(screen.getByTestId(`seg-${option.id}`)).not.toBeDisabled()
    }
  })

  test('stacked renders flex-col instead of flex-row container', () => {
    const onChange = vi.fn()
    const { container } = render(
      <SegmentedControl
        options={OPTIONS}
        value="off"
        onChange={onChange}
        stacked
        testId="seg-stacked"
      />,
    )
    const group = container.querySelector('[data-testid="seg-stacked"]')
    expect(group?.className).toContain('flex-col')
    expect(group?.className).not.toContain('flex-row')
  })

  test('omitting testId still renders every option (no data-testid leak)', () => {
    const onChange = vi.fn()
    const { container } = render(
      <SegmentedControl options={OPTIONS} value="off" onChange={onChange} />,
    )
    // 3 radio buttons rendered, none carrying a data-testid attribute.
    const radios = container.querySelectorAll('button[role="radio"]')
    expect(radios.length).toBe(3)
    for (const radio of Array.from(radios)) {
      expect(radio.getAttribute('data-testid')).toBeNull()
    }
  })

  test('group container exposes radiogroup role for accessibility tools', () => {
    const onChange = vi.fn()
    const { container } = render(
      <SegmentedControl
        options={OPTIONS}
        value="off"
        onChange={onChange}
        testId="seg-role"
      />,
    )
    const group = container.querySelector('[data-testid="seg-role"]')
    expect(group?.getAttribute('role')).toBe('radiogroup')
  })
})
