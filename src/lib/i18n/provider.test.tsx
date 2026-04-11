/**
 * This test file protects the front-end helper and contract logic in Provider.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - `HookProbe`
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from './provider'
import { useI18n, useI18nContext } from './hooks'
import { i18nStorageKey, readStoredPreference } from './context'

/**
 * Explains how hook probe works.
 *
 * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
 */
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
    expect(document.documentElement.lang).toBe('en-US')

    await user.click(screen.getByRole('button', { name: 'transient' }))
    expect(screen.getByTestId('language')).toHaveTextContent('zh-CN')
    expect(document.documentElement.lang).toBe('zh-CN')
    expect(window.localStorage.getItem(i18nStorageKey)).toBe('en')

    await user.click(screen.getByRole('button', { name: 'persist' }))
    expect(screen.getByTestId('language')).toHaveTextContent('zh-TW')
    expect(document.documentElement.lang).toBe('zh-TW')
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

  test('skips document lang writes when document is unavailable during hydration', () => {
    const originalDocument = globalThis.document
    const container = originalDocument.createElement('div')
    originalDocument.body.appendChild(container)
    vi.stubGlobal('document', undefined)

    try {
      expect(() =>
        render(
          <I18nProvider>
            <div>server-safe</div>
          </I18nProvider>,
          { container },
        ),
      ).not.toThrow()
    } finally {
      vi.stubGlobal('document', originalDocument)
      container.remove()
    }
  })
})
