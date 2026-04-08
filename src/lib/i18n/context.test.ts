import { afterEach, describe, expect, test, vi } from 'vitest'
import { i18nStorageKey, readStoredPreference } from './context'

describe('i18n context helpers', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    window.localStorage.clear()
  })

  test('defaults to system when window is unavailable', () => {
    vi.stubGlobal('window', undefined)

    expect(readStoredPreference()).toBe('system')
  })

  test('reads stored language preferences and ignores invalid values', () => {
    window.localStorage.setItem(i18nStorageKey, 'zh-TW')
    expect(readStoredPreference()).toBe('zh-TW')

    window.localStorage.setItem(i18nStorageKey, 'invalid-language')
    expect(readStoredPreference()).toBe('system')
  })
})
