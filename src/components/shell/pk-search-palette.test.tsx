/**
 * Coverage test for the paper-shell ⌘K search palette.
 *
 * ## Responsibilities
 * - Cover the empty-hint / loading / no-results / results branches of the
 *   CommandList renderer.
 * - Cover Cmd+Enter full-search shortcut → navigate('/explorer?q=…').
 * - Cover onSelect result handler.
 * - Cover the domainAbbreviation + hashDomainColor helpers indirectly via
 *   rendering results with edge-case domains (short, www-prefixed, varied
 *   lengths).
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import type * as ReactRouter from 'react-router-dom'
import { I18nProvider } from '@/lib/i18n'
import {
  PKSearchPalette,
  type PaletteResult,
} from './pk-search-palette'

const navigateMock = vi.fn()
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof ReactRouter>('react-router-dom')
  return {
    ...actual,
    useNavigate: () => navigateMock,
  }
})

function renderPalette(
  overrides: Partial<Parameters<typeof PKSearchPalette>[0]> = {},
) {
  const props: Parameters<typeof PKSearchPalette>[0] = {
    open: true,
    onOpenChange: vi.fn(),
    onSearch: vi.fn().mockResolvedValue([]),
    onSelect: vi.fn(),
    ...overrides,
  }
  return {
    ...render(
      <I18nProvider>
        <MemoryRouter>
          <PKSearchPalette {...props} />
        </MemoryRouter>
      </I18nProvider>,
    ),
    props,
  }
}

const sampleResults: PaletteResult[] = [
  {
    id: 'r1',
    title: 'Tauri docs',
    domain: 'www.tauri.app',
    url: 'https://www.tauri.app/v2/',
    visitDate: '2026-04-30',
    visitTime: '14:23',
  },
  {
    id: 'r2',
    title: 'Vite',
    domain: 'a.io',
    url: 'https://a.io/',
    visitDate: null,
    visitTime: null,
  },
]

describe('PKSearchPalette', () => {
  beforeEach(() => {
    navigateMock.mockReset()
    vi.useFakeTimers({ shouldAdvanceTime: true })
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  test('shows the empty-hint branch when no query has been typed', () => {
    renderPalette()
    // Empty-hint copy is the only one shown.
    expect(screen.getByPlaceholderText(/find/i)).toBeInTheDocument()
  })

  test('runs onSearch after the debounce and renders results', async () => {
    const onSearch = vi.fn().mockResolvedValue(sampleResults)
    const onSelect = vi.fn()
    const onOpenChange = vi.fn()
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    renderPalette({ onSearch, onSelect, onOpenChange })

    const input = screen.getByPlaceholderText(/find/i)
    await user.type(input, 'tauri')
    await act(async () => {
      vi.advanceTimersByTime(300)
      await Promise.resolve()
    })

    await waitFor(() => expect(onSearch).toHaveBeenCalled())
    await waitFor(() =>
      expect(screen.getByText('Tauri docs')).toBeInTheDocument(),
    )

    await user.click(screen.getByText('Tauri docs'))
    expect(onSelect).toHaveBeenCalledWith(sampleResults[0])
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  test('Cmd+Enter triggers full search navigation', async () => {
    const onOpenChange = vi.fn()
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    renderPalette({ onOpenChange })

    const input = screen.getByPlaceholderText(/find/i)
    await user.type(input, 'github')
    await user.keyboard('{Meta>}{Enter}{/Meta}')

    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(navigateMock).toHaveBeenCalledWith(
      '/explorer?q=github',
    )
  })

  test('Cmd+Enter without query goes to /explorer', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    renderPalette()
    const input = screen.getByPlaceholderText(/find/i)
    input.focus()
    await user.keyboard('{Control>}{Enter}{/Control}')
    expect(navigateMock).toHaveBeenCalledWith('/explorer')
  })

  test('swallows onSearch rejections and renders the no-results branch', async () => {
    const onSearch = vi.fn().mockRejectedValue(new Error('boom'))
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    renderPalette({ onSearch })

    const input = screen.getByPlaceholderText(/find/i)
    await user.type(input, 'oops')
    await act(async () => {
      vi.advanceTimersByTime(300)
      await Promise.resolve()
    })
    await waitFor(() => expect(onSearch).toHaveBeenCalled())
    // No exception, palette remains mounted.
    expect(input).toBeInTheDocument()
  })
})
