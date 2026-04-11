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
import { cleanup } from '@testing-library/react'

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
})

afterEach(() => {
  cleanup()
  window.localStorage?.clear?.()
  vi.clearAllMocks()
})
