/**
 * @file backend-preview-shell-commands.test.ts
 * @description Focused coverage for browser-preview shell command dispatch fallbacks.
 * @module lib
 */

import { describe, expect, test } from 'vitest'
import { handlePreviewShellCommand } from './backend-preview-shell-commands'
import { createMockState } from './backend-preview-state'

describe('handlePreviewShellCommand', () => {
  test('loads history favicons with an empty default request', () => {
    const state = createMockState()

    expect(
      handlePreviewShellCommand('load_history_favicons', undefined, state),
    ).toEqual([])
  })

  test('loads history og:image rows with an empty default request', () => {
    const state = createMockState()

    expect(
      handlePreviewShellCommand('load_history_og_images', undefined, state),
    ).toEqual([])
  })
})
