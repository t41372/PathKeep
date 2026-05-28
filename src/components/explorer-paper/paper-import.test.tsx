/**
 * Tests for the Import wizard primitives — stepper + method card.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { PaperImportMethodCard, PaperImportStepper } from './index'

const STEPS = ['Upload', 'Scan', 'Preview', 'Confirm', 'Import']

describe('PaperImportStepper', () => {
  test('renders each step with its label', () => {
    render(
      <PaperImportStepper steps={STEPS} currentStep={2} testId="stepper" />,
    )

    for (const label of STEPS) {
      expect(screen.getByText(label)).toBeVisible()
    }
  })

  test('marks the active step with data-step="active" and the done steps with "done"', () => {
    render(
      <PaperImportStepper
        steps={STEPS}
        currentStep={2}
        testId="stepper-state"
      />,
    )

    expect(screen.getByTestId('paper-import-step-0').dataset.step).toBe('done')
    expect(screen.getByTestId('paper-import-step-1').dataset.step).toBe('done')
    expect(screen.getByTestId('paper-import-step-2').dataset.step).toBe(
      'active',
    )
    expect(screen.getByTestId('paper-import-step-3').dataset.step).toBe('idle')
  })

  test('renders the checkmark glyph on done steps', () => {
    render(<PaperImportStepper steps={STEPS} currentStep={2} />)
    const done = screen.getByTestId('paper-import-step-0')
    expect(within(done).getByText('✓')).toBeVisible()
  })

  test('renders connectors between steps but not after the last', () => {
    const { container } = render(
      <PaperImportStepper steps={STEPS} currentStep={2} />,
    )
    // Each step except the last gets a connector div via `mx-[14px]` className.
    const connectors = container.querySelectorAll('[aria-hidden="true"].h-px')
    expect(connectors.length).toBe(STEPS.length - 1)
  })
})

describe('PaperImportMethodCard', () => {
  test('renders title, description, and hint', () => {
    render(
      <PaperImportMethodCard
        id="takeout"
        title="Google Takeout"
        description="Import an exported Google archive."
        hint="Recommended · ZIP or unpacked"
        onSelect={() => {}}
      />,
    )

    expect(screen.getByText('Google Takeout')).toBeVisible()
    expect(screen.getByText('Import an exported Google archive.')).toBeVisible()
    expect(screen.getByText('Recommended · ZIP or unpacked')).toBeVisible()
  })

  test('clicking the card forwards the id to onSelect', () => {
    const onSelect = vi.fn()
    render(
      <PaperImportMethodCard
        id="takeout"
        title="Google Takeout"
        description="…"
        onSelect={onSelect}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-import-method-takeout'))
    expect(onSelect).toHaveBeenCalledWith('takeout')
  })

  test('renders the active-state styling and data-active attribute', () => {
    render(
      <PaperImportMethodCard
        id="takeout"
        title="Google Takeout"
        description="…"
        onSelect={() => {}}
        active
      />,
    )

    const card = screen.getByTestId('paper-import-method-takeout')
    expect(card.dataset.active).toBe('true')
    expect(card.className).toContain('border-accent')
  })

  test('disabled when no handler is supplied', () => {
    render(
      <PaperImportMethodCard id="csv" title="CSV / JSON" description="…" />,
    )

    expect(
      screen.getByTestId<HTMLButtonElement>('paper-import-method-csv').disabled,
    ).toBe(true)
  })

  test('renders the icon node when supplied', () => {
    render(
      <PaperImportMethodCard
        id="takeout"
        title="Google Takeout"
        description="…"
        icon={<span data-testid="method-icon">★</span>}
        onSelect={() => {}}
      />,
    )

    expect(screen.getByTestId('method-icon')).toBeVisible()
  })

  test('renders the icon with the inactive accent class when active is false', () => {
    render(
      <PaperImportMethodCard
        id="zip"
        title="ZIP archive"
        description="…"
        icon={<span data-testid="inactive-icon">▦</span>}
        onSelect={() => {}}
      />,
    )
    // The `active ? 'text-accent' : 'text-ink-muted'` ternary at line 67
    // of paper-import-method-card.tsx fires the falsy branch.
    const iconWrapper = screen.getByTestId('inactive-icon').parentElement
    expect(iconWrapper?.className).toContain('text-ink-muted')
  })

  test('renders the icon with the active accent class when active is true', () => {
    render(
      <PaperImportMethodCard
        id="zip-active"
        title="ZIP archive (active)"
        description="…"
        icon={<span data-testid="active-icon">▦</span>}
        onSelect={() => {}}
        active
      />,
    )
    // Active branch of the same ternary at line 67.
    const iconWrapper = screen.getByTestId('active-icon').parentElement
    expect(iconWrapper?.className).toContain('text-accent')
  })
})
