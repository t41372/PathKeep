/**
 * Coverage test for the paper-shell bottom status bar.
 *
 * ## Responsibilities
 * - Cover the "All sources" click handler (line 188) and the per-source
 *   selection toggle handler (line 220).
 * - Cover the activeSource lookup ?? null fallback when selectedSourceId
 *   doesn't match any source (line 94).
 * - Smoke-cover the archiving / not-initialized status indicator branches.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nProvider } from '@/lib/i18n'
import { PKStatusBar, type PKStatusBarSource } from './pk-status-bar'

const sources: PKStatusBarSource[] = [
  {
    id: 'chrome:default',
    label: 'Chrome',
    profile: 'Default',
    color: '#4285F4',
    pages: 1234,
    size: '12 MB',
  },
  {
    id: 'firefox:default',
    label: 'Firefox',
    profile: null,
    color: '#FF6B35',
    size: '6 MB',
  },
]

function renderStatusBar(
  overrides: Partial<Parameters<typeof PKStatusBar>[0]> = {},
) {
  const props: Parameters<typeof PKStatusBar>[0] = {
    archiving: false,
    initialized: true,
    totalPages: 7777,
    totalSize: '18 MB',
    sinceLabel: 'since Mar 2024',
    lastArchivedLabel: 'last archived 09:30',
    sources,
    selectedSourceId: null,
    onSelectSource: vi.fn(),
    onManageSources: vi.fn(),
    epigraphIndex: 1,
    ...overrides,
  }
  return {
    ...render(
      <I18nProvider>
        <PKStatusBar {...props} />
      </I18nProvider>,
    ),
    props,
  }
}

afterEach(() => {
  vi.clearAllMocks()
})

describe('PKStatusBar', () => {
  test('clicking a source toggles selection through onSelectSource', async () => {
    const user = userEvent.setup()
    const { props } = renderStatusBar()
    await user.click(screen.getByTestId('pk-status-bar-source-trigger'))

    // Per-source button: select Chrome (line 220).
    await user.click(screen.getByRole('button', { name: /Chrome/ }))
    expect(props.onSelectSource).toHaveBeenLastCalledWith('chrome:default')
  })

  test('clicking All routes onSelectSource(null) (line 188)', async () => {
    const user = userEvent.setup()
    const { props } = renderStatusBar({ selectedSourceId: 'chrome:default' })
    await user.click(screen.getByTestId('pk-status-bar-source-trigger'))

    // The "All sources" row is the first popover button after the sources
    // header. Identify it by the aggregate-pages count text which only the
    // all-sources row carries.
    const popoverButtons = screen.getAllByRole('button')
    const allRow = popoverButtons.find((button) =>
      button.textContent?.includes(' pages'),
    )
    expect(allRow).toBeDefined()
    if (allRow) {
      await user.click(allRow)
      expect(props.onSelectSource).toHaveBeenLastCalledWith(null)
    }
  })

  test('toggles off the same source when clicked again', async () => {
    const user = userEvent.setup()
    const { props } = renderStatusBar({ selectedSourceId: 'chrome:default' })
    await user.click(screen.getByTestId('pk-status-bar-source-trigger'))
    // Two Chrome-bearing buttons exist (the now-active trigger + the per-source
    // popover row). Pick the one inside the popover by filtering on the
    // per-source size text (only the popover row carries the size string).
    const popoverRow = screen
      .getAllByRole('button')
      .find((button) => button.textContent?.includes('12 MB'))
    expect(popoverRow).toBeDefined()
    if (popoverRow) {
      await user.click(popoverRow)
      expect(props.onSelectSource).toHaveBeenLastCalledWith(null)
    }
  })

  test('renders the archive-not-initialized branch when initialized is false', () => {
    renderStatusBar({ initialized: false })
    expect(screen.getByTestId('pk-status-bar')).toBeInTheDocument()
  })

  test('renders the archiving pulse indicator when archiving', () => {
    renderStatusBar({ archiving: true })
    expect(screen.getByTestId('pk-status-bar')).toBeInTheDocument()
  })

  test('handles a selectedSourceId that does not match any source (?? null fallback)', () => {
    // activeSource lookup fails → fallback path runs on line 94.
    renderStatusBar({ selectedSourceId: 'unknown:profile' })
    expect(screen.getByTestId('pk-status-bar')).toBeInTheDocument()
  })

  test('routes Manage Sources to onManageSources', async () => {
    const user = userEvent.setup()
    const { props } = renderStatusBar()
    await user.click(screen.getByTestId('pk-status-bar-source-trigger'))
    const manage = screen.getByRole('button', { name: /Manage|管理/i })
    await user.click(manage)
    expect(props.onManageSources).toHaveBeenCalled()
  })
})
