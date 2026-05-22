/**
 * Smoke test for the paper-redesign Appearance section.
 *
 * Verifies:
 * - The section renders with localized labels.
 * - Switching the theme toggle updates the document data-theme attribute and
 *   persists the choice to localStorage.
 * - Switching the fonts toggle persists 'system' and applies data-fonts.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import { AppearanceSection } from './appearance-section'

describe('AppearanceSection', () => {
  beforeEach(() => {
    window.localStorage.clear()
    document.documentElement.removeAttribute('data-theme')
    document.documentElement.removeAttribute('data-fonts')
    document.documentElement.removeAttribute('data-density')
    document.documentElement.style.removeProperty('--noise-opacity')
  })

  test('renders the four appearance fields', () => {
    render(
      <I18nProvider>
        <AppearanceSection />
      </I18nProvider>,
    )
    expect(
      screen.getByTestId('settings-appearance-section'),
    ).toBeInTheDocument()
    expect(screen.getByText('Theme')).toBeInTheDocument()
    expect(screen.getByText('Density')).toBeInTheDocument()
    expect(
      screen.getByRole('radio', { name: /Darkroom · dark/i }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('radio', { name: /System fonts only/i }),
    ).toBeInTheDocument()
    expect(screen.getByRole('switch')).toBeInTheDocument()
  })

  test('switching the theme persists and applies the new value', async () => {
    const user = userEvent.setup()
    render(
      <I18nProvider>
        <AppearanceSection />
      </I18nProvider>,
    )
    const darkButton = screen.getByRole('radio', { name: /Darkroom · dark/i })
    await user.click(darkButton)
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
    expect(window.localStorage.getItem('pathkeep.theme')).toBe('dark')
  })

  test('switching fonts to system applies data-fonts and persists', async () => {
    const user = userEvent.setup()
    render(
      <I18nProvider>
        <AppearanceSection />
      </I18nProvider>,
    )
    const systemButton = screen.getByRole('radio', {
      name: /System fonts only/i,
    })
    await user.click(systemButton)
    expect(document.documentElement.getAttribute('data-fonts')).toBe('system')
    expect(window.localStorage.getItem('pathkeep.fonts')).toBe('system')
  })

  test('switching density persists and applies the new value', async () => {
    const user = userEvent.setup()
    render(
      <I18nProvider>
        <AppearanceSection />
      </I18nProvider>,
    )
    // Cozy (the default) → Compact. The label is the i18n key in test
    // since this section uses a deeply-namespaced translator.
    const compactButton = screen.getByRole('radio', { name: /Compact/i })
    await user.click(compactButton)
    expect(document.documentElement.getAttribute('data-density')).toBe(
      'compact',
    )
    expect(window.localStorage.getItem('pathkeep.density')).toBe('compact')
  })

  test('toggling paper texture off sets --noise-opacity to 0', async () => {
    const user = userEvent.setup()
    render(
      <I18nProvider>
        <AppearanceSection />
      </I18nProvider>,
    )
    const toggle = screen.getByRole('switch')
    await user.click(toggle)
    expect(
      document.documentElement.style.getPropertyValue('--noise-opacity'),
    ).toBe('0')
    expect(window.localStorage.getItem('pathkeep.paperTexture')).toBe('off')
  })

  test('switching to 24-hour clock persists the format and fires CLOCK_FORMAT_EVENT', async () => {
    const user = userEvent.setup()
    const events: string[] = []
    const listener = (event: Event) => {
      const detail = (event as CustomEvent<{ format: string }>).detail
      events.push(detail?.format ?? 'unknown')
    }
    window.addEventListener('pathkeep.clockFormatChanged', listener)
    try {
      render(
        <I18nProvider>
          <AppearanceSection />
        </I18nProvider>,
      )
      const twentyFour = screen.getByRole('radio', { name: /24-hour/i })
      await user.click(twentyFour)
      expect(window.localStorage.getItem('pathkeep.clockFormat')).toBe('24h')
      expect(events).toContain('24h')
    } finally {
      window.removeEventListener('pathkeep.clockFormatChanged', listener)
      window.localStorage.removeItem('pathkeep.clockFormat')
    }
  })

  test('the appearance card reflows when CLOCK_FORMAT_EVENT fires from a peer surface', async () => {
    render(
      <I18nProvider>
        <AppearanceSection />
      </I18nProvider>,
    )
    // The 12-hour option is the default; the segmented-control radio
    // exposes its `aria-checked` state which we can read to verify the
    // local mirror updated after the event.
    const twelve = screen.getByRole('radio', { name: /12-hour/i })
    const twentyFour = screen.getByRole('radio', { name: /24-hour/i })
    expect(twelve.getAttribute('aria-checked')).toBe('true')
    expect(twentyFour.getAttribute('aria-checked')).toBe('false')
    // Simulate a peer (e.g. a palette command or another open Settings
    // window) flipping the preference. The card's useEffect listener
    // must catch the event and update its local mirror without remount.
    await import('@testing-library/react').then(({ act }) =>
      act(() => {
        window.dispatchEvent(
          new CustomEvent('pathkeep.clockFormatChanged', {
            detail: { format: '24h' },
          }),
        )
      }),
    )
    expect(twentyFour.getAttribute('aria-checked')).toBe('true')
    expect(twelve.getAttribute('aria-checked')).toBe('false')
  })
})
