/**
 * This test file protects the dev-only gate for the paper QA overlay mounts:
 * dev builds honor `?layout=paper`, production builds ignore it entirely.
 */

import { describe, expect, test } from 'vitest'
import { isPaperQaLayoutEnabled } from './paper-qa-layout'

describe('isPaperQaLayoutEnabled', () => {
  test('enables the QA overlay in dev builds when layout=paper is present', () => {
    expect(
      isPaperQaLayoutEnabled(new URLSearchParams('layout=paper'), true),
    ).toBe(true)
  })

  test('stays disabled in dev builds without the layout=paper param', () => {
    expect(isPaperQaLayoutEnabled(new URLSearchParams(), true)).toBe(false)
    expect(
      isPaperQaLayoutEnabled(new URLSearchParams('layout=classic'), true),
    ).toBe(false)
  })

  test('ignores layout=paper entirely in production builds', () => {
    expect(
      isPaperQaLayoutEnabled(new URLSearchParams('layout=paper'), false),
    ).toBe(false)
  })

  test('defaults devBuild to import.meta.env.DEV (true under vitest)', () => {
    // Vitest runs with DEV=true, so the default-parameter branch must behave
    // exactly like the explicit devBuild=true branch here.
    expect(isPaperQaLayoutEnabled(new URLSearchParams('layout=paper'))).toBe(
      true,
    )
    expect(isPaperQaLayoutEnabled(new URLSearchParams())).toBe(false)
  })
})
