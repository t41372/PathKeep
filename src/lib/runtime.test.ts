import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import {
  DEV_IPC_URL_ENV,
  hasDesktopCommandTransport,
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
  })

  afterEach(() => {
    vi.unstubAllEnvs()
  })

  test('prefers the tauri runtime when the guest api is available', () => {
    isTauriMock.mockReturnValue(true)
    vi.stubEnv(DEV_IPC_URL_ENV, 'http://127.0.0.1:43117/')

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
