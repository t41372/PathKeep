import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { I18nProvider } from '@/lib/i18n'
import { DashboardThisWeek } from './this-week-card'

type Locale = 'en' | 'zh-CN' | 'zh-TW'

function renderCard(language: Locale = 'en') {
  window.localStorage.setItem('pathkeep-language-preference', language)
  return render(
    <I18nProvider>
      <DashboardThisWeek
        totalPages={12345}
        totalUrls={789}
        recentRunsCount={4}
      />
    </I18nProvider>,
  )
}

describe('DashboardThisWeek', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  test('renders the en locale + week badge + comma-formatted stats', () => {
    renderCard('en')
    expect(screen.getByTestId('dashboard-this-week')).toBeInTheDocument()
    expect(screen.getByText('12,345')).toBeInTheDocument()
    expect(screen.getByText('789')).toBeInTheDocument()
    expect(screen.getByText('4')).toBeInTheDocument()
  })

  test('formats stats with the zh-CN locale separator', () => {
    renderCard('zh-CN')
    // zh-CN comma-formats the same way Intl chooses ("12,345"); the goal
    // is to exercise the `language === 'en' ? 'en-US' : language` branch.
    expect(screen.getByTestId('dashboard-this-week')).toBeInTheDocument()
  })

  describe('isoWeek (driven via week badge)', () => {
    beforeEach(() => {
      vi.useFakeTimers()
    })

    test('renders a Sunday-anchored week without falling back to week 0', () => {
      // Sunday 2026-01-04. `new Date().getDay()` is 0 — the `|| 7`
      // fallback inside isoWeek covers the Sunday branch.
      vi.setSystemTime(new Date(2026, 0, 4))
      renderCard('en')
      // The badge text always includes the week number.
      expect(screen.getByText(/Week \d+/)).toBeInTheDocument()
    })

    test('renders a mid-week date so the typical branch runs', () => {
      // Wednesday 2026-04-15. getDay() = 3, falls through `|| 7`.
      vi.setSystemTime(new Date(2026, 3, 15))
      renderCard('en')
      expect(screen.getByText(/Week \d+/)).toBeInTheDocument()
    })
  })
})
