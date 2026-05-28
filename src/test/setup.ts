/**
 * This module implements the Setup front-end surface.
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

import '@testing-library/jest-dom/vitest'

import { afterEach, beforeAll, vi } from 'vitest'
import { cleanup, configure } from '@testing-library/react'

// testing-library's default asyncUtilTimeout is ~1s. Under the full
// coverage:js sweep on slower hosts (Linux dev VMs, v8 coverage
// instrumentation, ~250 files in parallel), heavy shell tests like the
// settings/maintenance route can take 2-4 s just to hydrate the App.
// 5 s is well within vitest's 15 s testTimeout and large enough that
// findBy* won't mask real failures with spurious timeouts.
configure({ asyncUtilTimeout: 5000 })

beforeAll(() => {
  const storage = (() => {
    const values = new Map<string, string>()
    return {
      clear: () => values.clear(),
      getItem: (key: string) => values.get(key) ?? null,
      key: (index: number) => Array.from(values.keys())[index] ?? null,
      removeItem: (key: string) => values.delete(key),
      setItem: (key: string, value: string) => {
        values.set(key, value)
      },
      get length() {
        return values.size
      },
    }
  })()

  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    writable: true,
    value: storage,
  })

  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  })

  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    writable: true,
    value: {
      writeText: vi.fn().mockResolvedValue(undefined),
    },
  })

  // cmdk (search palette) needs ResizeObserver; jsdom doesn't ship one.
  // Inert noop is enough for the library's internal layout bookkeeping.
  if (typeof window.ResizeObserver === 'undefined') {
    window.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as typeof ResizeObserver
  }

  // Radix dialog primitives call element.scrollIntoView in autofocus paths.
  if (typeof Element.prototype.scrollIntoView !== 'function') {
    Element.prototype.scrollIntoView = function () {}
  }
})

afterEach(() => {
  cleanup()
  window.localStorage?.clear?.()
  vi.clearAllMocks()
})
