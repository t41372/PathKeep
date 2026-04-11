/**
 * This test file protects the front-end helper and contract logic in Tokens.
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

import { describe, expect, test } from 'vitest'
import { themes, tokens } from './tokens'

describe('design tokens', () => {
  test('exports both supported themes', () => {
    expect(themes).toEqual(['dark', 'light'])
  })

  test('maps token helpers to CSS variables', () => {
    expect(tokens.color.accent).toBe('var(--accent)')
    expect(tokens.font.ui).toBe('var(--font-ui)')
    expect(tokens.font.code).toBe('var(--font-code)')
    expect(tokens.space[4]).toBe('var(--space-4)')
    expect(tokens.density.sidebar).toBe('var(--layout-sidebar-width)')
  })
})
