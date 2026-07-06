/**
 * @file button.test.tsx
 * @description Direct behavioral coverage for the paper-token Button primitive.
 * @module components/ui
 *
 * ## Responsibilities
 * - Assert every variant/size resolves the expected paper-token classes.
 * - Assert the `loading` affordance (aria-busy, spinner, non-interactive,
 *   stable label/width) and its interaction with `asChild`.
 * - Assert the primitive contract survives: `asChild`, `buttonVariants`
 *   export, `data-slot`/`data-variant`/`data-size`, prop/ref forwarding.
 *
 * ## Not responsible for
 * - Visual regression / pixel assertions (jsdom has no layout engine).
 * - Dialog's own `variant="outline"` call site — that's exercised by dialog
 *   consumers, not here; this file only guarantees `outline` keeps existing.
 *
 * ## Dependencies
 * - Real i18n catalog (`createNamespaceTranslator`) for a realistic
 *   `loadingLabel` value, matching how a real caller would supply it.
 */

import { createRef } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '@/lib/i18n'
import { Button, buttonVariants } from './button'

const commonT = createNamespaceTranslator('en', 'common')

describe('Button variants', () => {
  test('primary is a filled accent CTA with a fixed on-accent light label', () => {
    render(<Button variant="primary">Save</Button>)
    const button = screen.getByRole('button', { name: 'Save' })
    expect(button).toHaveClass(
      'bg-accent',
      'text-primary-foreground',
      'border-accent',
    )
    expect(button.className).not.toMatch(/\btext-paper\b/)
    expect(button).toHaveAttribute('data-variant', 'primary')
  })

  test('outline is the neutral workhorse secondary and is the default', () => {
    render(<Button>Cancel</Button>)
    const button = screen.getByRole('button', { name: 'Cancel' })
    expect(button).toHaveClass('border-border-default', 'text-ink-muted')
    expect(button).toHaveAttribute('data-variant', 'outline')
  })

  test('accent variant is an accent-bordered prominent action', () => {
    render(<Button variant="accent">Continue</Button>)
    const button = screen.getByRole('button', { name: 'Continue' })
    expect(button).toHaveClass('border-accent', 'text-accent-text')
  })

  test('ghost variant is borderless', () => {
    render(<Button variant="ghost">Dismiss</Button>)
    const button = screen.getByRole('button', { name: 'Dismiss' })
    expect(button).toHaveClass('border-transparent', 'text-ink-muted')
  })

  test('destructive variant uses error tokens, never the inert danger classes', () => {
    render(<Button variant="destructive">Delete</Button>)
    const button = screen.getByRole('button', { name: 'Delete' })
    expect(button).toHaveClass('text-error', 'border-error')
    expect(button.className).not.toMatch(/\btext-danger\b|\bborder-danger\b/)
  })

  test('link variant renders as an underlined text action', () => {
    render(<Button variant="link">Learn more</Button>)
    const button = screen.getByRole('button', { name: 'Learn more' })
    expect(button).toHaveClass('text-accent-text', 'underline-offset-4')
  })
})

describe('Button sizes', () => {
  test('default size targets the ~36px action height', () => {
    render(<Button>Default</Button>)
    expect(screen.getByRole('button', { name: 'Default' })).toHaveClass('h-9')
  })

  test('sm size targets the ~28px compact height', () => {
    render(<Button size="sm">Small</Button>)
    expect(screen.getByRole('button', { name: 'Small' })).toHaveClass('h-7')
  })

  test('lg size is larger than default', () => {
    render(<Button size="lg">Large</Button>)
    expect(screen.getByRole('button', { name: 'Large' })).toHaveClass('h-10')
  })

  test('icon size is the convergent 28px icon box', () => {
    render(<Button size="icon" aria-label="Icon action" />)
    expect(screen.getByRole('button', { name: 'Icon action' })).toHaveClass(
      'size-7',
    )
  })

  test('icon-sm size is 24px', () => {
    render(<Button size="icon-sm" aria-label="Small icon action" />)
    expect(
      screen.getByRole('button', { name: 'Small icon action' }),
    ).toHaveClass('size-6')
  })

  test('icon-lg size is 36px', () => {
    render(<Button size="icon-lg" aria-label="Large icon action" />)
    expect(
      screen.getByRole('button', { name: 'Large icon action' }),
    ).toHaveClass('size-9')
  })
})

describe('Button loading state', () => {
  test('sets aria-busy, disables the button, and renders a reduced-motion-safe spinner', () => {
    const handleClick = vi.fn()
    render(
      <Button loading onClick={handleClick}>
        Submit
      </Button>,
    )
    const button = screen.getByRole('button')

    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(button).toBeDisabled()
    expect(button).toHaveAttribute('data-loading', 'true')

    const spinner = button.querySelector('[data-slot="button-spinner"]')
    expect(spinner).not.toBeNull()
    expect(spinner).toHaveClass('motion-safe:animate-spin')
    expect(spinner).toHaveAttribute('aria-hidden', 'true')
  })

  test('keeps the idle label in the DOM (opacity-0) so width does not reflow', () => {
    render(<Button loading>Submit</Button>)
    // The label text is still present (not removed/replaced), just visually hidden
    // via opacity (not visibility), so the button's content box keeps the same
    // footprint as the idle state.
    const label = screen.getByText('Submit')
    expect(label).toHaveClass('opacity-0')
    expect(label.className).not.toMatch(/\binvisible\b/)
  })

  test('is non-interactive while loading — click handler does not fire', async () => {
    const user = userEvent.setup()
    const handleClick = vi.fn()
    render(
      <Button loading onClick={handleClick}>
        Submit
      </Button>,
    )
    await user.click(screen.getByRole('button'))
    expect(handleClick).not.toHaveBeenCalled()
  })

  test('renders a caller-supplied accessible loading label', () => {
    render(
      <Button loading loadingLabel={commonT('loading')}>
        Submit
      </Button>,
    )
    expect(screen.getByText(commonT('loading'))).toHaveClass('sr-only')
  })

  test('omits the sr-only label but keeps an accessible name when the caller supplies none', () => {
    const { container } = render(<Button loading>Submit</Button>)
    expect(container.querySelector('.sr-only')).toBeNull()
    // The visually-hidden (opacity-0) label stays in the accessibility tree,
    // so a loading button with no loadingLabel is still nameable by
    // assistive tech instead of announcing as an unnamed busy button.
    expect(screen.getByRole('button', { name: 'Submit' })).toHaveAttribute(
      'aria-busy',
      'true',
    )
  })

  test('idle (non-loading) button has no aria-busy, no data-loading, no spinner', () => {
    render(<Button>Submit</Button>)
    const button = screen.getByRole('button')
    expect(button).not.toHaveAttribute('aria-busy')
    expect(button).not.toHaveAttribute('data-loading')
    expect(button.querySelector('[data-slot="button-spinner"]')).toBeNull()
    expect(button).not.toBeDisabled()
  })

  test('asChild + loading skips the spinner overlay and renders the child unchanged', () => {
    render(
      <Button asChild loading>
        <a href="/somewhere">Open</a>
      </Button>,
    )
    const link = screen.getByRole('link', { name: 'Open' })
    expect(link.querySelector('[data-slot="button-spinner"]')).toBeNull()
    expect(link).toHaveAttribute('aria-busy', 'true')
  })

  test('asChild + loading is non-interactive: aria-disabled, pointer-events-none, and the click/navigation is swallowed', async () => {
    const user = userEvent.setup()
    const handleClick = vi.fn()
    render(
      <Button asChild loading onClick={handleClick}>
        <a href="/somewhere">Open</a>
      </Button>,
    )
    const link = screen.getByRole('link', { name: 'Open' })

    expect(link).toHaveAttribute('aria-disabled', 'true')
    expect(link).not.toHaveAttribute('disabled')
    expect(link).toHaveClass('pointer-events-none', 'cursor-not-allowed')
    // href stays intact (the child element is unmodified) but the click is
    // swallowed before it reaches the caller's handler or native navigation.
    expect(link).toHaveAttribute('href', '/somewhere')

    await user.click(link)
    expect(handleClick).not.toHaveBeenCalled()
  })

  test('asChild + disabled (no loading) is also non-interactive via the same asChild guard', async () => {
    const user = userEvent.setup()
    const handleClick = vi.fn()
    render(
      <Button asChild disabled onClick={handleClick}>
        <a href="/somewhere">Open</a>
      </Button>,
    )
    const link = screen.getByRole('link', { name: 'Open' })

    expect(link).toHaveAttribute('aria-disabled', 'true')
    expect(link).not.toHaveAttribute('aria-busy')
    await user.click(link)
    expect(handleClick).not.toHaveBeenCalled()
  })

  test('asChild + neither loading nor disabled stays fully interactive with no aria-disabled', async () => {
    const user = userEvent.setup()
    const handleClick = vi.fn()
    render(
      <Button asChild onClick={handleClick}>
        <a href="/somewhere">Open</a>
      </Button>,
    )
    const link = screen.getByRole('link', { name: 'Open' })

    expect(link).not.toHaveAttribute('aria-disabled')
    // Note: the base variant classes always include the arbitrary selector
    // `[&_svg]:pointer-events-none` (icons ignore pointer events) — that is
    // a different class token from the bare `pointer-events-none` this test
    // guards against, so assert via toHaveClass's token match, not a regex.
    expect(link).not.toHaveClass('pointer-events-none')
    await user.click(link)
    expect(handleClick).toHaveBeenCalledTimes(1)
  })
})

describe('Button disabled state', () => {
  test('disabled prop alone (no loading) is non-interactive with the standard dim treatment', async () => {
    const user = userEvent.setup()
    const handleClick = vi.fn()
    render(
      <Button disabled onClick={handleClick}>
        Submit
      </Button>,
    )
    const button = screen.getByRole('button')
    expect(button).toBeDisabled()
    expect(button).toHaveClass(
      'disabled:opacity-50',
      'disabled:cursor-not-allowed',
    )
    expect(button).not.toHaveAttribute('aria-busy')
    await user.click(button)
    expect(handleClick).not.toHaveBeenCalled()
  })
})

describe('Button primitive contract', () => {
  test('asChild renders the child element (not a <button>) carrying the variant classes', () => {
    render(
      <Button asChild variant="primary" size="lg">
        <a href="/help">Help</a>
      </Button>,
    )
    const link = screen.getByRole('link', { name: 'Help' })
    expect(link.tagName).toBe('A')
    expect(link).toHaveClass('bg-accent', 'text-primary-foreground', 'h-10')
  })

  test('forwards refs to the underlying button element', () => {
    const ref = createRef<HTMLButtonElement>()
    render(<Button ref={ref}>Ref target</Button>)
    expect(ref.current).toBeInstanceOf(HTMLButtonElement)
    expect(ref.current?.textContent).toBe('Ref target')
  })

  test('forwards arbitrary DOM props and exposes data-slot/data-variant/data-size', () => {
    render(
      <Button variant="ghost" size="sm" title="hover hint" data-testid="cta">
        Go
      </Button>,
    )
    const button = screen.getByTestId('cta')
    expect(button).toHaveAttribute('data-slot', 'button')
    expect(button).toHaveAttribute('data-variant', 'ghost')
    expect(button).toHaveAttribute('data-size', 'sm')
    expect(button).toHaveAttribute('title', 'hover hint')
  })

  test('merges a caller className with the resolved variant classes', () => {
    render(<Button className="w-full">Full width</Button>)
    const button = screen.getByRole('button', { name: 'Full width' })
    expect(button).toHaveClass('w-full', 'border-border-default')
  })

  test('buttonVariants is exported and resolves the same classes the component renders', () => {
    expect(buttonVariants({ variant: 'destructive', size: 'sm' })).toContain(
      'text-error',
    )
    expect(buttonVariants({ variant: 'destructive', size: 'sm' })).toContain(
      'h-7',
    )
  })
})
