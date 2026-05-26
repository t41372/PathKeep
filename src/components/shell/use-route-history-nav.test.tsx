/**
 * Coverage for `useRouteHistoryNav`.
 *
 * The hook is the single source of truth for "is back/forward currently
 * possible" in the v0.3 shell, so these tests pin both the disabled-state
 * matrix (history root, forward-after-back, forward-after-push) and the
 * keyboard shortcut policy (Cmd/Ctrl plus [ / ], skip editable targets,
 * skip when alt/shift are also held).
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import { fireEvent } from '@testing-library/dom'
import { MemoryRouter, useNavigate } from 'react-router-dom'
import { useRouteHistoryNav } from './use-route-history-nav'

function NavHarness({
  onMount,
}: {
  onMount: (api: ReturnType<typeof useRouteHistoryNav>) => void
}) {
  const api = useRouteHistoryNav()
  onMount(api)
  const navigate = useNavigate()
  return (
    <div>
      <button
        data-testid="harness-push"
        onClick={() => void navigate('/second')}
      >
        push
      </button>
      <button data-testid="harness-back" onClick={api.goBack}>
        back
      </button>
      <button data-testid="harness-forward" onClick={api.goForward}>
        forward
      </button>
      <span data-testid="harness-can-back">{api.canGoBack ? 'y' : 'n'}</span>
      <span data-testid="harness-can-forward">
        {api.canGoForward ? 'y' : 'n'}
      </span>
      <span data-testid="harness-modifier">{api.modifierLabel}</span>
      <input data-testid="harness-input" />
    </div>
  )
}

const platformDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'platform',
)
const userAgentDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'userAgent',
)

function setPlatform(value: string) {
  Object.defineProperty(window.navigator, 'platform', {
    configurable: true,
    value,
  })
}

function setUserAgent(value: string) {
  Object.defineProperty(window.navigator, 'userAgent', {
    configurable: true,
    value,
  })
}

afterEach(() => {
  if (platformDescriptor) {
    Object.defineProperty(window.navigator, 'platform', platformDescriptor)
  }
  if (userAgentDescriptor) {
    Object.defineProperty(window.navigator, 'userAgent', userAgentDescriptor)
  }
})

describe('useRouteHistoryNav', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  test('starts disabled at history root and enables back after a push', () => {
    const calls: ReturnType<typeof useRouteHistoryNav>[] = []
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={(api) => calls.push(api)} />
      </MemoryRouter>,
    )
    expect(screen.getByTestId('harness-can-back')).toHaveTextContent('n')
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')

    act(() => {
      screen.getByTestId('harness-push').click()
    })
    expect(screen.getByTestId('harness-can-back')).toHaveTextContent('y')
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('goBack arms forward and goForward clears it again', () => {
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    act(() => {
      screen.getByTestId('harness-back').click()
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('y')
    act(() => {
      screen.getByTestId('harness-forward').click()
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('goBack is a no-op at history root', () => {
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-back').click()
    })
    // No new render side-effects; still at the disabled baseline.
    expect(screen.getByTestId('harness-can-back')).toHaveTextContent('n')
  })

  test('goForward is a no-op when there is no forward branch', () => {
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-forward').click()
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('Cmd+[ fires goBack on Mac platforms', () => {
    setPlatform('MacIntel')
    setUserAgent('Mozilla/5.0 (Macintosh)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    act(() => {
      fireEvent.keyDown(document, { key: '[', metaKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('y')
  })

  test('Ctrl+] fires goForward on non-Mac platforms after a back step', () => {
    setPlatform('Linux x86_64')
    setUserAgent('Mozilla/5.0 (X11; Linux x86_64)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    act(() => {
      fireEvent.keyDown(document, { key: '[', ctrlKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('y')
    act(() => {
      fireEvent.keyDown(document, { key: ']', ctrlKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('keyboard shortcut is ignored while focus is in an editable target', () => {
    setPlatform('MacIntel')
    setUserAgent('Mozilla/5.0 (Macintosh)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    const input = screen.getByTestId('harness-input')
    input.focus()
    act(() => {
      fireEvent.keyDown(input, { key: '[', metaKey: true })
    })
    // Editable focus suppressed the shortcut → still no forward branch.
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('keyboard shortcut requires the platform-specific modifier', () => {
    setPlatform('MacIntel')
    setUserAgent('Mozilla/5.0 (Macintosh)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    // On Mac, Ctrl+[ should be ignored — only Cmd (meta) counts.
    act(() => {
      fireEvent.keyDown(document, { key: '[', ctrlKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
    // Alt/Shift modifiers disqualify even with the correct base mod.
    act(() => {
      fireEvent.keyDown(document, { key: '[', metaKey: true, altKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
    act(() => {
      fireEvent.keyDown(document, {
        key: '[',
        metaKey: true,
        shiftKey: true,
      })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
    // Unrelated key never fires either branch.
    act(() => {
      fireEvent.keyDown(document, { key: 'a', metaKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('non-Mac platforms reject Cmd+[ — only Ctrl counts', () => {
    setPlatform('Linux x86_64')
    setUserAgent('Mozilla/5.0 (X11; Linux x86_64)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    act(() => {
      fireEvent.keyDown(document, { key: '[', metaKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('modifier label adapts to the running platform', () => {
    setPlatform('MacIntel')
    setUserAgent('Mozilla/5.0 (Macintosh)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    expect(screen.getByTestId('harness-modifier')).toHaveTextContent('⌘')
  })

  test('modifier label falls back to Ctrl on other platforms', () => {
    setPlatform('Linux x86_64')
    setUserAgent('Mozilla/5.0 (X11; Linux x86_64)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    expect(screen.getByTestId('harness-modifier')).toHaveTextContent('Ctrl+')
  })

  test('UA-based fallback flags macOS when navigator.platform is blank', () => {
    setPlatform('')
    setUserAgent('Mozilla/5.0 (Mac OS X 14.0)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    expect(screen.getByTestId('harness-modifier')).toHaveTextContent('⌘')
  })

  test('keyboard shortcut bails out when the target is contenteditable', () => {
    setPlatform('MacIntel')
    setUserAgent('Mozilla/5.0 (Macintosh)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
        <div data-testid="harness-editable" contentEditable />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    const editable = screen.getByTestId('harness-editable')
    act(() => {
      fireEvent.keyDown(editable, { key: '[', metaKey: true })
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('n')
  })

  test('keyboard shortcut tolerates non-element keydown targets', () => {
    setPlatform('MacIntel')
    setUserAgent('Mozilla/5.0 (Macintosh)')
    render(
      <MemoryRouter initialEntries={['/']}>
        <NavHarness onMount={() => {}} />
      </MemoryRouter>,
    )
    act(() => {
      screen.getByTestId('harness-push').click()
    })
    const event = new KeyboardEvent('keydown', {
      key: '[',
      metaKey: true,
      bubbles: true,
    })
    Object.defineProperty(event, 'target', { value: null })
    act(() => {
      document.dispatchEvent(event)
    })
    expect(screen.getByTestId('harness-can-forward')).toHaveTextContent('y')
  })
})
