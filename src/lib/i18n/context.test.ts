/**
 * This test file protects the front-end helper and contract logic in Context.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

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
