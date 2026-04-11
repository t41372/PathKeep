/**
 * This module implements the Main front-end surface.
 *
 * Why this file exists:
 * - It is part of the active `src/` tree and should explain its own role without forcing the next reader to scan unrelated files first.
 * - When this file changes, the surrounding comments should keep the intent, boundaries, and main declarations easy to see at a glance.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep the implementation aligned with the accepted product, design, and architecture documents.
 * - Prefer explicit structure over cleverness so the codebase stays navigable as the front-end keeps growing.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'

const { createRootMock, renderMock } = vi.hoisted(() => ({
  createRootMock: vi.fn(),
  renderMock: vi.fn(),
}))

const { installRuntimeDiagnosticsMock } = vi.hoisted(() => ({
  installRuntimeDiagnosticsMock: vi.fn().mockResolvedValue(undefined),
}))

const { resolveAppRuntimeMock } = vi.hoisted(() => ({
  resolveAppRuntimeMock: vi.fn(() => 'browser-preview'),
}))

vi.mock('react-dom/client', () => ({
  createRoot: createRootMock,
}))

vi.mock('./app', () => ({
  default: () => <div>App shell</div>,
}))

vi.mock('./lib/runtime-diagnostics', () => ({
  installRuntimeDiagnostics: installRuntimeDiagnosticsMock,
}))

vi.mock('./lib/runtime', () => ({
  resolveAppRuntime: resolveAppRuntimeMock,
}))

describe('main entrypoint', () => {
  beforeEach(() => {
    vi.resetModules()
    document.body.innerHTML = '<div id="root"></div>'
    document.documentElement.removeAttribute('data-theme')
    document.documentElement.removeAttribute('data-pathkeep-runtime')
    document.documentElement.lang = 'en'
    window.localStorage.clear()
    renderMock.mockReset()
    resolveAppRuntimeMock.mockReset().mockReturnValue('browser-preview')
    installRuntimeDiagnosticsMock.mockReset().mockResolvedValue(undefined)
    createRootMock.mockReset().mockReturnValue({
      render: renderMock,
    })
  })

  test('mounts the React application into the root element', async () => {
    await import('./main.tsx')

    expect(createRootMock).toHaveBeenCalledWith(document.getElementById('root'))
    expect(renderMock).toHaveBeenCalledTimes(1)
    expect(installRuntimeDiagnosticsMock).toHaveBeenCalledTimes(1)
    expect(document.documentElement.dataset.pathkeepRuntime).toBe(
      'browser-preview',
    )
  })

  test('restores a persisted theme preference before rendering', async () => {
    window.localStorage.setItem('pathkeep.theme', 'dark')

    await import('./main.tsx')

    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
    expect(document.documentElement.lang).toBe('en-US')
    expect(renderMock).toHaveBeenCalledTimes(1)
  })

  test('restores a persisted light theme preference before rendering', async () => {
    window.localStorage.setItem('pathkeep.theme', 'light')

    await import('./main.tsx')

    expect(document.documentElement.getAttribute('data-theme')).toBe('light')
    expect(document.documentElement.lang).toBe('en-US')
    expect(renderMock).toHaveBeenCalledTimes(1)
  })

  test('ignores unsupported persisted theme values', async () => {
    window.localStorage.setItem('pathkeep.theme', 'sepia')
    window.localStorage.setItem('pathkeep-language-preference', 'zh-TW')

    await import('./main.tsx')

    expect(document.documentElement.hasAttribute('data-theme')).toBe(false)
    expect(document.documentElement.lang).toBe('zh-TW')
    expect(renderMock).toHaveBeenCalledTimes(1)
  })

  test('tags the document with the active runtime for agent diagnostics', async () => {
    resolveAppRuntimeMock.mockReturnValue('browser-desktop-bridge')

    await import('./main.tsx')

    expect(document.documentElement.dataset.pathkeepRuntime).toBe(
      'browser-desktop-bridge',
    )
  })

  test('keeps rendering if reading theme preference throws', async () => {
    const getItemSpy = vi
      .spyOn(Storage.prototype, 'getItem')
      .mockImplementation(() => {
        throw new Error('storage unavailable')
      })

    await import('./main.tsx')

    expect(document.documentElement.hasAttribute('data-theme')).toBe(false)
    expect(document.documentElement.lang).toBe('en-US')
    expect(renderMock).toHaveBeenCalledTimes(1)

    getItemSpy.mockRestore()
  })

  test('restores the system language before rendering when no explicit locale is stored', async () => {
    Object.defineProperty(navigator, 'languages', {
      configurable: true,
      value: ['zh-Hant-TW'],
    })

    window.localStorage.setItem('pathkeep-language-preference', 'system')

    await import('./main.tsx')

    expect(document.documentElement.lang).toBe('zh-TW')
    expect(renderMock).toHaveBeenCalledTimes(1)
  })
})
