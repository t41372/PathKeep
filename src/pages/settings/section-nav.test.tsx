/**
 * @file section-nav.test.tsx
 * @description Focused coverage for same-route Settings and Maintenance section-anchor scrolling.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify the shared sticky nav scrolls and focuses existing panels.
 * - Cover defensive branches for missing panels and older focus implementations.
 *
 * ## Not responsible for
 * - Re-testing Settings or Maintenance page content.
 * - Re-testing the translated section descriptor factories.
 *
 * ## Dependencies
 * - Uses React Router memory state because the nav builds route-aware hash hrefs.
 *
 * ## Performance notes
 * - This test is DOM-only and does not load Settings backend state.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { SettingsSectionNav } from './section-nav'
import type { SettingsSectionNavItem } from './section-nav-items'

const navItems: SettingsSectionNavItem[] = [
  {
    icon: 'settings',
    id: 'settings-general',
    key: 'general',
    label: 'General',
  },
  {
    icon: 'download',
    id: 'settings-migration',
    key: 'migration',
    label: 'Data migration',
  },
]

describe('SettingsSectionNav', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  test('scrolls existing same-route targets and falls back when preventScroll focus is unsupported', async () => {
    const user = userEvent.setup()
    const originalScrollIntoView = Reflect.get(
      Element.prototype,
      'scrollIntoView',
    )
    const originalRequestAnimationFrame = window.requestAnimationFrame
    const originalCancelAnimationFrame = window.cancelAnimationFrame
    const scrollIntoView = vi.fn()
    const targetFocus = vi.fn()

    Element.prototype.scrollIntoView = scrollIntoView
    window.requestAnimationFrame = vi.fn((callback: FrameRequestCallback) => {
      callback(0)
      return 1
    })
    window.cancelAnimationFrame = vi.fn()

    try {
      render(
        <MemoryRouter initialEntries={['/settings']}>
          <section id="settings-migration" tabIndex={0} />
          <SettingsSectionNav items={navItems} label="Settings sections" />
        </MemoryRouter>,
      )
      const remotePanel = document.getElementById('settings-migration')
      if (!(remotePanel instanceof HTMLElement)) {
        throw new Error('Expected settings remote panel')
      }
      Object.defineProperty(remotePanel, 'focus', {
        configurable: true,
        value: targetFocus,
      })
      targetFocus
        .mockImplementationOnce(() => {
          throw new Error('preventScroll unsupported')
        })
        .mockImplementation(() => undefined)

      await user.click(
        screen.getByRole('link', {
          name: 'Data migration',
        }),
      )

      // No reduced-motion preference (jsdom has no matchMedia): smooth scroll.
      expect(scrollIntoView).toHaveBeenCalledWith({
        behavior: 'smooth',
        block: 'start',
      })
      expect(document.getElementById('settings-migration')).toHaveAttribute(
        'tabindex',
        '0',
      )
      expect(targetFocus).toHaveBeenCalledTimes(2)
    } finally {
      Element.prototype.scrollIntoView = originalScrollIntoView
      window.requestAnimationFrame = originalRequestAnimationFrame
      window.cancelAnimationFrame = originalCancelAnimationFrame
    }
  })

  test('jumps instantly (no smooth scroll) when the user prefers reduced motion', async () => {
    const user = userEvent.setup()
    const originalScrollIntoView = Reflect.get(
      Element.prototype,
      'scrollIntoView',
    )
    const originalRequestAnimationFrame = window.requestAnimationFrame
    const originalCancelAnimationFrame = window.cancelAnimationFrame
    const originalMatchMedia = window.matchMedia
    const scrollIntoView = vi.fn()

    Element.prototype.scrollIntoView = scrollIntoView
    window.requestAnimationFrame = vi.fn((callback: FrameRequestCallback) => {
      callback(0)
      return 1
    })
    window.cancelAnimationFrame = vi.fn()
    window.matchMedia = vi.fn(
      (query: string) =>
        ({
          matches: query.includes('prefers-reduced-motion'),
          media: query,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
          addListener: vi.fn(),
          removeListener: vi.fn(),
          onchange: null,
          dispatchEvent: vi.fn(),
        }) as unknown as MediaQueryList,
    )

    try {
      render(
        <MemoryRouter initialEntries={['/settings']}>
          <section id="settings-migration" tabIndex={0} />
          <SettingsSectionNav items={navItems} label="Settings sections" />
        </MemoryRouter>,
      )

      await user.click(
        screen.getByRole('link', {
          name: 'Data migration',
        }),
      )

      expect(scrollIntoView).toHaveBeenCalledWith({
        behavior: 'auto',
        block: 'start',
      })
    } finally {
      Element.prototype.scrollIntoView = originalScrollIntoView
      window.requestAnimationFrame = originalRequestAnimationFrame
      window.cancelAnimationFrame = originalCancelAnimationFrame
      window.matchMedia = originalMatchMedia
    }
  })

  test('ignores hash targets that do not have a mounted panel', async () => {
    const user = userEvent.setup()
    const originalRequestAnimationFrame = window.requestAnimationFrame
    const originalScrollIntoView = Reflect.get(
      Element.prototype,
      'scrollIntoView',
    )
    const scrollIntoView = vi.fn()

    Element.prototype.scrollIntoView = scrollIntoView
    window.requestAnimationFrame =
      undefined as unknown as typeof window.requestAnimationFrame

    try {
      render(
        <MemoryRouter initialEntries={['/maintenance']}>
          <SettingsSectionNav items={navItems} label="Maintenance sections" />
        </MemoryRouter>,
      )

      await user.click(
        screen.getByRole('link', {
          name: 'Data migration',
        }),
      )
      await new Promise((resolve) => window.setTimeout(resolve, 0))
      expect(scrollIntoView).not.toHaveBeenCalled()
    } finally {
      Element.prototype.scrollIntoView = originalScrollIntoView
      window.requestAnimationFrame = originalRequestAnimationFrame
    }
  })

  test('clears pending fallback initial-hash scroll work on unmount', () => {
    vi.useFakeTimers()
    const originalRequestAnimationFrame = window.requestAnimationFrame
    const originalScrollIntoView = Reflect.get(
      Element.prototype,
      'scrollIntoView',
    )
    const clearTimeout = vi.spyOn(window, 'clearTimeout')

    Element.prototype.scrollIntoView = vi.fn()
    window.requestAnimationFrame =
      undefined as unknown as typeof window.requestAnimationFrame

    try {
      const { unmount } = render(
        <MemoryRouter initialEntries={['/settings#settings-migration']}>
          <section id="settings-migration" />
          <SettingsSectionNav items={navItems} label="Settings sections" />
        </MemoryRouter>,
      )

      unmount()

      expect(clearTimeout).toHaveBeenCalled()
    } finally {
      Element.prototype.scrollIntoView = originalScrollIntoView
      window.requestAnimationFrame = originalRequestAnimationFrame
    }
  })
})
