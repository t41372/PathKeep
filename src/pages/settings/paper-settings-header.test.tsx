import { describe, expect, test, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { PaperSettingsHeader } from './paper-settings-header'
import type { SettingsSectionNavItem } from './section-nav-items'

const items: SettingsSectionNavItem[] = [
  {
    key: 'general',
    id: 'settings-general',
    label: 'General',
    icon: 'settings',
  },
  {
    key: 'applock',
    id: 'settings-applock',
    label: 'App Lock',
    icon: 'shield',
  },
]

function renderHeader(
  overrides: Partial<React.ComponentProps<typeof PaperSettingsHeader>> = {},
) {
  return render(
    <MemoryRouter initialEntries={['/settings']}>
      <PaperSettingsHeader
        eyebrow="Preferences"
        title="Settle the page before you read."
        subtitle="Persistent choices live here."
        jumpLabel="Jump to"
        items={items}
        {...overrides}
      />
    </MemoryRouter>,
  )
}

describe('PaperSettingsHeader', () => {
  test('renders the eyebrow, title, and subtitle from props', () => {
    renderHeader()
    expect(screen.getByText('Preferences')).toBeInTheDocument()
    expect(
      screen.getByRole('heading', {
        name: 'Settle the page before you read.',
      }),
    ).toBeInTheDocument()
    expect(
      screen.getByText('Persistent choices live here.'),
    ).toBeInTheDocument()
  })

  test('renders one jump anchor per item with the labelled href', () => {
    renderHeader()
    const generalLink = screen.getByRole<HTMLAnchorElement>('link', {
      name: 'General',
    })
    expect(generalLink.getAttribute('href')).toContain('#settings-general')
    const lockLink = screen.getByRole<HTMLAnchorElement>('link', {
      name: 'App Lock',
    })
    expect(lockLink.getAttribute('href')).toContain('#settings-applock')
  })

  test('scrolls the corresponding section into view when an anchor is clicked', () => {
    document.body.innerHTML = '<div id="settings-applock"></div>'
    const target = document.getElementById('settings-applock')
    if (!(target instanceof HTMLElement)) throw new Error('target missing')
    const scrollSpy = vi.fn()
    Object.defineProperty(target, 'scrollIntoView', {
      value: scrollSpy,
      configurable: true,
    })
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0)
        return 1
      })
    renderHeader()
    fireEvent.click(
      screen.getByRole<HTMLAnchorElement>('link', { name: 'App Lock' }),
    )
    expect(scrollSpy).toHaveBeenCalledWith({ block: 'start' })
    rafSpy.mockRestore()
  })

  test('scrolls without overwriting tabindex when the target already has one', () => {
    document.body.innerHTML = '<div id="settings-applock" tabindex="0"></div>'
    const target = document.getElementById('settings-applock')
    if (!(target instanceof HTMLElement)) throw new Error('target missing')
    const scrollSpy = vi.fn()
    Object.defineProperty(target, 'scrollIntoView', {
      value: scrollSpy,
      configurable: true,
    })
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0)
        return 1
      })
    renderHeader()
    fireEvent.click(
      screen.getByRole<HTMLAnchorElement>('link', { name: 'App Lock' }),
    )
    expect(scrollSpy).toHaveBeenCalledWith({ block: 'start' })
    expect(target.getAttribute('tabindex')).toBe('0')
    rafSpy.mockRestore()
  })

  test('uses the provided testId', () => {
    renderHeader({ testId: 'paper-settings-header-x' })
    expect(screen.getByTestId('paper-settings-header-x')).toBeInTheDocument()
  })

  test('falls back to the default testId when not provided', () => {
    renderHeader()
    expect(screen.getByTestId('paper-settings-header')).toBeInTheDocument()
  })

  test('still scrolls when focus() throws — paper header survives focusable mismatch', () => {
    document.body.innerHTML = '<div id="settings-applock"></div>'
    const target = document.getElementById('settings-applock')
    if (!(target instanceof HTMLElement)) throw new Error('target missing')
    const scrollSpy = vi.fn()
    Object.defineProperty(target, 'scrollIntoView', {
      value: scrollSpy,
      configurable: true,
    })
    // First focus throws; the focusSection helper catches the throw and
    // retries the unparameterised focus() — both calls land here.
    let focusCalls = 0
    Object.defineProperty(target, 'focus', {
      value: () => {
        focusCalls += 1
        if (focusCalls === 1) {
          throw new Error('preventScroll unsupported')
        }
      },
      configurable: true,
    })
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0)
        return 1
      })
    renderHeader()
    fireEvent.click(
      screen.getByRole<HTMLAnchorElement>('link', { name: 'App Lock' }),
    )
    expect(scrollSpy).toHaveBeenCalledWith({ block: 'start' })
    expect(focusCalls).toBe(2)
    rafSpy.mockRestore()
  })

  test('falls back to setTimeout when requestAnimationFrame is missing', () => {
    document.body.innerHTML = '<div id="settings-applock"></div>'
    const target = document.getElementById('settings-applock')
    if (!(target instanceof HTMLElement)) throw new Error('target missing')
    const scrollSpy = vi.fn()
    Object.defineProperty(target, 'scrollIntoView', {
      value: scrollSpy,
      configurable: true,
    })
    const originalRaf = window.requestAnimationFrame
    // Casting to `unknown` first lets us strip the function without
    // tripping the strict @typescript-eslint rule banning direct `any`.
    ;(
      window as unknown as { requestAnimationFrame: unknown }
    ).requestAnimationFrame = undefined
    vi.useFakeTimers()
    try {
      renderHeader()
      fireEvent.click(
        screen.getByRole<HTMLAnchorElement>('link', { name: 'App Lock' }),
      )
      vi.runAllTimers()
      expect(scrollSpy).toHaveBeenCalledWith({ block: 'start' })
    } finally {
      vi.useRealTimers()
      ;(
        window as unknown as { requestAnimationFrame: typeof originalRaf }
      ).requestAnimationFrame = originalRaf
    }
  })

  test('hash-driven mount scrolls to the matching section on load', () => {
    document.body.innerHTML = '<div id="settings-applock"></div>'
    const target = document.getElementById('settings-applock')
    if (!(target instanceof HTMLElement)) throw new Error('target missing')
    const scrollSpy = vi.fn()
    Object.defineProperty(target, 'scrollIntoView', {
      value: scrollSpy,
      configurable: true,
    })
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0)
        return 1
      })
    render(
      <MemoryRouter initialEntries={['/settings#settings-applock']}>
        <PaperSettingsHeader
          eyebrow="Preferences"
          title="Settle the page before you read."
          subtitle="Persistent choices live here."
          jumpLabel="Jump to"
          items={items}
        />
      </MemoryRouter>,
    )
    expect(scrollSpy).toHaveBeenCalledWith({ block: 'start' })
    rafSpy.mockRestore()
  })

  test('hash that does not match any section is ignored', () => {
    document.body.innerHTML = ''
    const scrollSpy = vi.fn()
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0)
        return 1
      })
    render(
      <MemoryRouter initialEntries={['/settings#not-a-section']}>
        <PaperSettingsHeader
          eyebrow="Preferences"
          title="Title"
          subtitle="Sub"
          jumpLabel="Jump to"
          items={items}
        />
      </MemoryRouter>,
    )
    expect(scrollSpy).not.toHaveBeenCalled()
    rafSpy.mockRestore()
  })
})
