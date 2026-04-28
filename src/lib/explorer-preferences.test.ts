/**
 * @file explorer-preferences.test.ts
 * @description Bounds coverage for the shared Explorer prefetch preference helper.
 * @module lib/explorer-preferences
 */

import { describe, expect, test } from 'vitest'
import {
  defaultExplorerBackgroundPrefetchPages,
  explorerBackgroundPrefetchPageOptions,
  maxExplorerBackgroundPrefetchPages,
  normalizeExplorerBackgroundPrefetchPages,
} from './explorer-preferences'

describe('Explorer background prefetch preferences', () => {
  test('normalizes invalid, low, high, and fractional values', () => {
    expect(normalizeExplorerBackgroundPrefetchPages(null)).toBe(
      defaultExplorerBackgroundPrefetchPages,
    )
    expect(normalizeExplorerBackgroundPrefetchPages(Number.NaN)).toBe(
      defaultExplorerBackgroundPrefetchPages,
    )
    expect(normalizeExplorerBackgroundPrefetchPages(-2)).toBe(0)
    expect(normalizeExplorerBackgroundPrefetchPages(99)).toBe(
      maxExplorerBackgroundPrefetchPages,
    )
    expect(normalizeExplorerBackgroundPrefetchPages(3.9)).toBe(3)
  })

  test('exposes one option for every allowed prefetch count', () => {
    expect(explorerBackgroundPrefetchPageOptions).toEqual([
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    ])
  })
})
