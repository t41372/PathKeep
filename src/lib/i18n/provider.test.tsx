import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from './provider'
import { useI18n, useI18nContext } from './hooks'
import { i18nStorageKey, readStoredPreference } from './context'

function HookProbe() {
  const root = useI18n()
  const navigation = useI18n('navigation')
  const context = useI18nContext()

  return (
    <div>
      <div data-testid="language">{root.language}</div>
      <div data-testid="preference">{root.preference}</div>
      <div data-testid="nav-dashboard">{navigation.t('dashboardLabel')}</div>
      <div data-testid="context-language">{context.language}</div>
      <button
        type="button"
        onClick={() => root.setLanguagePreference('zh-CN', { persist: false })}
      >
        transient
      </button>
      <button type="button" onClick={() => root.setLanguagePreference('zh-TW')}>
        persist
      </button>
    </div>
  )
}

describe('i18n provider and hooks', () => {
  test('reads stored preferences and persists language changes when requested', async () => {
    const user = userEvent.setup()
    window.localStorage.setItem(i18nStorageKey, 'en')

    render(
      <I18nProvider>
        <HookProbe />
      </I18nProvider>,
    )

    expect(screen.getByTestId('language')).toHaveTextContent('en')
    expect(screen.getByTestId('preference')).toHaveTextContent('en')
    expect(screen.getByTestId('nav-dashboard')).toHaveTextContent('Dashboard')
    expect(screen.getByTestId('context-language')).toHaveTextContent('en')

    await user.click(screen.getByRole('button', { name: 'transient' }))
    expect(screen.getByTestId('language')).toHaveTextContent('zh-CN')
    expect(window.localStorage.getItem(i18nStorageKey)).toBe('en')

    await user.click(screen.getByRole('button', { name: 'persist' }))
    expect(screen.getByTestId('language')).toHaveTextContent('zh-TW')
    expect(window.localStorage.getItem(i18nStorageKey)).toBe('zh-TW')
  })

  test('falls back to system when storage is empty or invalid and hooks require a provider', () => {
    window.localStorage.removeItem(i18nStorageKey)
    expect(readStoredPreference()).toBe('system')

    window.localStorage.setItem(i18nStorageKey, 'invalid-value')
    expect(readStoredPreference()).toBe('system')

    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined)
    expect(() => render(<HookProbe />)).toThrow(
      'useI18nContext must be used inside I18nProvider',
    )
    consoleError.mockRestore()
  })
})
