/**
 * This test file protects the front-end helper and contract logic in Runtime.
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

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import {
  DEV_IPC_URL_ENV,
  hasDesktopCommandTransport,
  hasMacOverlayTitlebar,
  hasTauriGuestApi,
  resolveAppRuntime,
  resolveDevIpcBridgeUrl,
} from './runtime'

const { isTauriMock } = vi.hoisted(() => ({
  isTauriMock: vi.fn(() => false),
}))

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: isTauriMock,
}))

describe('runtime detection', () => {
  beforeEach(() => {
    isTauriMock.mockReturnValue(false)
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
  })

  test('prefers the tauri runtime when the guest api is available', () => {
    isTauriMock.mockReturnValue(true)
    vi.stubEnv(DEV_IPC_URL_ENV, 'http://127.0.0.1:43117/')

    expect(resolveAppRuntime()).toBe('tauri')
    expect(hasDesktopCommandTransport()).toBe(true)
    expect(hasTauriGuestApi()).toBe(true)
  })

  test('treats injected tauri internals as a real desktop runtime even without global isTauri', () => {
    vi.stubEnv(DEV_IPC_URL_ENV, 'http://127.0.0.1:43117/')
    vi.stubGlobal('__TAURI_INTERNALS__', {
      invoke: vi.fn(),
    })

    expect(resolveAppRuntime()).toBe('tauri')
    expect(hasDesktopCommandTransport()).toBe(true)
    expect(hasTauriGuestApi()).toBe(true)
  })

  test('treats the tauri protocol as a real desktop runtime even without helper globals', () => {
    vi.stubEnv(DEV_IPC_URL_ENV, 'http://127.0.0.1:43117/')
    vi.stubGlobal('location', {
      protocol: 'tauri:',
    })

    expect(resolveAppRuntime()).toBe('tauri')
    expect(hasDesktopCommandTransport()).toBe(true)
    expect(hasTauriGuestApi()).toBe(true)
  })

  test('normalizes the browser desktop bridge url', () => {
    vi.stubEnv(DEV_IPC_URL_ENV, ' http://127.0.0.1:43117/ ')

    expect(resolveDevIpcBridgeUrl()).toBe('http://127.0.0.1:43117')
    expect(resolveAppRuntime()).toBe('browser-desktop-bridge')
    expect(hasDesktopCommandTransport()).toBe(true)
    expect(hasTauriGuestApi()).toBe(false)
  })

  test('falls back to browser preview when no desktop bridge is configured', () => {
    vi.stubEnv(DEV_IPC_URL_ENV, '   ')

    expect(resolveDevIpcBridgeUrl()).toBeNull()
    expect(resolveAppRuntime()).toBe('browser-preview')
    expect(hasDesktopCommandTransport()).toBe(false)
  })
})

describe('macOS overlay title bar', () => {
  beforeEach(() => {
    isTauriMock.mockReturnValue(false)
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
  })

  afterEach(() => {
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
  })

  test('is active on a real desktop window hosted on macOS', () => {
    isTauriMock.mockReturnValue(true)
    vi.stubGlobal('navigator', { platform: 'MacIntel', userAgent: '' })

    expect(hasMacOverlayTitlebar()).toBe(true)
  })

  test('detects macOS from the user agent when platform is empty', () => {
    isTauriMock.mockReturnValue(true)
    vi.stubGlobal('navigator', {
      platform: '',
      userAgent:
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15',
    })

    expect(hasMacOverlayTitlebar()).toBe(true)
  })

  test('is inactive on a desktop window hosted off macOS (Windows/Linux)', () => {
    isTauriMock.mockReturnValue(true)
    vi.stubGlobal('navigator', { platform: 'Win32', userAgent: '' })

    expect(hasMacOverlayTitlebar()).toBe(false)
  })

  test('is inactive in the browser preview even on macOS', () => {
    vi.stubEnv(DEV_IPC_URL_ENV, '   ')
    vi.stubGlobal('navigator', { platform: 'MacIntel', userAgent: '' })

    expect(resolveAppRuntime()).toBe('browser-preview')
    expect(hasMacOverlayTitlebar()).toBe(false)
  })

  test('is inactive when navigator is unavailable', () => {
    isTauriMock.mockReturnValue(true)
    vi.stubGlobal('navigator', undefined)

    expect(hasMacOverlayTitlebar()).toBe(false)
  })
})
