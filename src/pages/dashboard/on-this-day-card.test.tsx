/**
 * Coverage test for the "On this day" dashboard card.
 *
 * ## Responsibilities
 * - Cover the populated-list branch with onOpenEntry click + onJumpToDate
 *   click (Codex P1 removed the fake-data path, so the entries-present path
 *   only fires when something is explicitly passed in).
 * - Cover the loading skeleton + error fallback branches.
 * - Cover the summary-present branch so the trailing fallback note hides.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nProvider } from '@/lib/i18n'
import { i18nStorageKey } from '@/lib/i18n/context'
import { DashboardOnThisDay } from './on-this-day-card'
import type { OnThisDayEntry } from '@/lib/core-intelligence/types'

const entry: OnThisDayEntry = {
  year: 2024,
  date: '2024-05-19',
  totalVisits: 12,
  deepDiveSessions: 2,
  topDomains: ['github.com', 'wikipedia.org', 'arxiv.org'],
  summary: 'Researched Rust async ecosystem',
}

const entryWithoutSummary: OnThisDayEntry = {
  ...entry,
  year: 2023,
  date: '2023-05-19',
  summary: '',
  topDomains: [],
}

function renderCard(
  overrides: Partial<Parameters<typeof DashboardOnThisDay>[0]> = {},
) {
  const props: Parameters<typeof DashboardOnThisDay>[0] = {
    entries: [],
    loading: false,
    error: null,
    onJumpToDate: vi.fn(),
    onOpenEntry: vi.fn(),
    ...overrides,
  }
  return {
    ...render(
      <I18nProvider>
        <DashboardOnThisDay {...props} />
      </I18nProvider>,
    ),
    props,
  }
}

describe('DashboardOnThisDay', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    window.localStorage.removeItem(i18nStorageKey)
  })

  test('renders the loading skeleton when loading', () => {
    const { container } = renderCard({ loading: true })
    expect(container.querySelector('[aria-hidden="true"]')).not.toBeNull()
  })

  test('renders the error fallback when error is set', () => {
    renderCard({ error: 'oops' })
    expect(screen.getByTestId('dashboard-on-this-day')).toBeInTheDocument()
  })

  test('renders entries and wires onOpenEntry + onJumpToDate', async () => {
    const user = userEvent.setup()
    const { props } = renderCard({ entries: [entry] })
    expect(screen.getByText('2024')).toBeInTheDocument()
    expect(
      screen.getByText('Researched Rust async ecosystem'),
    ).toBeInTheDocument()

    await user.click(screen.getByText('Researched Rust async ecosystem'))
    expect(props.onOpenEntry).toHaveBeenCalledWith(entry)

    const dateButton = screen
      .getAllByRole('button')
      .find((button) => button.textContent?.includes('→'))
    expect(dateButton).toBeDefined()
    if (dateButton) {
      await user.click(dateButton)
      expect(props.onJumpToDate).toHaveBeenCalled()
    }
  })

  test('falls back to count when entry has no summary', () => {
    renderCard({ entries: [entryWithoutSummary] })
    // count-based fallback contains the count number formatted via i18n
    expect(screen.getByText('2023')).toBeInTheDocument()
    expect(screen.getByText('2023-05-19')).toBeInTheDocument()
  })

  test('formats the target date with the resolved non-English locale', () => {
    const seenLocales: Intl.LocalesArgument[] = []
    vi.spyOn(Date.prototype, 'toLocaleDateString').mockImplementation(function (
      this: Date,
      locales?: Intl.LocalesArgument,
    ) {
      seenLocales.push(locales)
      return 'localized target'
    })
    window.localStorage.setItem(i18nStorageKey, 'zh-CN')

    renderCard()

    expect(
      screen.getByRole('button', { name: /localized target/i }),
    ).toBeInTheDocument()
    expect(seenLocales).toContain('zh-CN')
  })
})
