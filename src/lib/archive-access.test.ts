/**
 * This test file protects the shared archive-access helper contract.
 *
 * Why this file exists:
 * - Shell routes and chrome rely on these helpers to choose between a recoverable unlock flow and a generic failure state.
 * - If backend refusal wording changes, this test makes the mismatch obvious before the UI drifts.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep these assertions aligned with the archive-layer error strings emitted by the Rust backend.
 * - Prefer a small, explicit matrix here over repeating regex expectations across many route tests.
 */

import { describe, expect, test } from 'vitest'
import {
  isArchiveUnlockRequiredError,
  isArchiveUnlockRequiredMessage,
} from './archive-access'

describe('archive-access helpers', () => {
  test('detect missing encrypted archive keys from message strings', () => {
    expect(
      isArchiveUnlockRequiredMessage(
        'database key is required for encrypted archives',
      ),
    ).toBe(true)
    expect(
      isArchiveUnlockRequiredMessage(
        'Encrypted archive requires an active session key',
      ),
    ).toBe(true)
    expect(
      isArchiveUnlockRequiredMessage(
        'PathKeep is currently locked. Unlock the app before requesting archive data.',
      ),
    ).toBe(false)
    expect(isArchiveUnlockRequiredMessage(null)).toBe(false)
  })

  test('detect missing encrypted archive keys from Error instances', () => {
    expect(
      isArchiveUnlockRequiredError(
        new Error('database key is required for encrypted archives'),
      ),
    ).toBe(true)
    expect(isArchiveUnlockRequiredError(new Error('archive unavailable'))).toBe(
      false,
    )
    expect(isArchiveUnlockRequiredError('database key is required')).toBe(false)
  })
})
