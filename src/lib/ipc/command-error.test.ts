/**
 * @file command-error.test.ts
 * @description Contract tests for the structured desktop command error envelope and its classification helpers.
 * @module lib/ipc
 *
 * Why this file exists:
 * - `commandErrorCode` is the ONLY sanctioned classification channel; if it
 *   started trusting arbitrary `code`-bearing objects (DOM errors, plugin
 *   errors), unrelated failures would silently route into the Full Disk Access
 *   or unlock remediation surfaces.
 * - The message-marker fallbacks exist only for pre-envelope channels, so they
 *   must match the exact strings the backend emits — never localized copy.
 */

import { describe, expect, test } from 'vitest'
import {
  CommandInvokeError,
  ERROR_CODE_FULL_DISK_ACCESS,
  ERROR_CODE_LOCK_REQUIRED,
  commandErrorCode,
  isFullDiskAccessError,
  isFullDiskAccessIssueMessage,
  isLockRequiredError,
} from './command-error'

describe('CommandInvokeError', () => {
  test('carries the backend envelope fields alongside the diagnostic message', () => {
    const error = new CommandInvokeError('archive is locked', {
      code: ERROR_CODE_LOCK_REQUIRED,
      actionHint: 'Unlock PathKeep',
      retryHint: 'Retry after unlocking',
    })

    expect(error).toBeInstanceOf(Error)
    expect(error.name).toBe('CommandInvokeError')
    expect(error.message).toBe('archive is locked')
    expect(error.code).toBe('lock-required')
    expect(error.actionHint).toBe('Unlock PathKeep')
    expect(error.retryHint).toBe('Retry after unlocking')
  })

  test('defaults every structured field to null when the backend omits them', () => {
    const error = new CommandInvokeError('plain failure')

    expect(error.code).toBeNull()
    expect(error.actionHint).toBeNull()
    expect(error.retryHint).toBeNull()
  })

  test('normalizes explicitly undefined envelope fields to null', () => {
    const error = new CommandInvokeError('plain failure', {
      code: undefined,
      actionHint: undefined,
      retryHint: undefined,
    })

    expect(error.code).toBeNull()
    expect(error.actionHint).toBeNull()
    expect(error.retryHint).toBeNull()
  })
})

describe('commandErrorCode', () => {
  test('reads the code from a bridge-thrown CommandInvokeError', () => {
    expect(
      commandErrorCode(
        new CommandInvokeError('denied', {
          code: ERROR_CODE_FULL_DISK_ACCESS,
        }),
      ),
    ).toBe('full-disk-access')
  })

  test('returns null for a CommandInvokeError the backend did not classify', () => {
    expect(commandErrorCode(new CommandInvokeError('denied'))).toBeNull()
  })

  test('reads the code from a raw backend envelope with a `message`', () => {
    expect(
      commandErrorCode({ message: 'archive is locked', code: 'lock-required' }),
    ).toBe('lock-required')
  })

  test('reads the code from a raw backend envelope that only has `error`', () => {
    expect(
      commandErrorCode({ error: 'archive is locked', code: 'lock-required' }),
    ).toBe('lock-required')
  })

  test('refuses to treat an arbitrary code-bearing object as an envelope', () => {
    // A DOM/plugin error with a numeric-ish code and no message/error text is
    // NOT a backend envelope — trusting it would misroute unrelated failures
    // into the remediation surfaces.
    expect(commandErrorCode({ code: '19' })).toBeNull()
    expect(commandErrorCode({ code: 'full-disk-access' })).toBeNull()
  })

  test('ignores a non-string code even on a well-formed envelope', () => {
    expect(commandErrorCode({ message: 'boom', code: 500 })).toBeNull()
  })

  test('returns null for plain errors, strings, and empty values', () => {
    expect(commandErrorCode(new Error('boom'))).toBeNull()
    expect(commandErrorCode('boom')).toBeNull()
    expect(commandErrorCode(null)).toBeNull()
    expect(commandErrorCode(undefined)).toBeNull()
  })
})

describe('isFullDiskAccessIssueMessage', () => {
  test('matches only the two raw backend markers', () => {
    expect(
      isFullDiskAccessIssueMessage(
        'Chrome history is unreadable until PathKeep is granted Full Disk Access.',
      ),
    ).toBe(true)
    expect(
      isFullDiskAccessIssueMessage(
        'Safari History.db is not readable yet; skipped this profile.',
      ),
    ).toBe(true)
    expect(isFullDiskAccessIssueMessage('permission denied (os error 1)')).toBe(
      false,
    )
  })
})

describe('isFullDiskAccessError', () => {
  test('trusts the structured code', () => {
    expect(
      isFullDiskAccessError(
        new CommandInvokeError('permission denied', {
          code: ERROR_CODE_FULL_DISK_ACCESS,
        }),
      ),
    ).toBe(true)
    expect(
      isFullDiskAccessError({
        message: 'permission denied',
        code: ERROR_CODE_FULL_DISK_ACCESS,
      }),
    ).toBe(true)
  })

  test('a classified non-FDA error is never reclassified by its message text', () => {
    // The code wins: a lock-required failure whose diagnostic chain happens to
    // mention Full Disk Access must NOT open the FDA remediation affordance.
    expect(
      isFullDiskAccessError(
        new CommandInvokeError(
          'PathKeep is currently locked (grant Full Disk Access later)',
          { code: ERROR_CODE_LOCK_REQUIRED },
        ),
      ),
    ).toBe(false)
  })

  test('falls back to the raw backend markers for pre-envelope channels', () => {
    expect(
      isFullDiskAccessError(
        new Error('PathKeep needs Full Disk Access to read Chrome history.'),
      ),
    ).toBe(true)
    expect(
      isFullDiskAccessError(new Error('Safari History.db is not readable yet')),
    ).toBe(true)
    expect(isFullDiskAccessError(new Error('disk offline'))).toBe(false)
  })

  test('returns false for non-Error values with no envelope', () => {
    expect(
      isFullDiskAccessError('PathKeep needs Full Disk Access to continue.'),
    ).toBe(false)
    expect(isFullDiskAccessError(null)).toBe(false)
  })
})

describe('isLockRequiredError', () => {
  test('trusts the structured code', () => {
    expect(
      isLockRequiredError(
        new CommandInvokeError('archive is encrypted', {
          code: ERROR_CODE_LOCK_REQUIRED,
        }),
      ),
    ).toBe(true)
    expect(
      isLockRequiredError({
        error: 'archive is encrypted',
        code: ERROR_CODE_LOCK_REQUIRED,
      }),
    ).toBe(true)
  })

  test('a classified non-lock error is never reclassified by its message text', () => {
    expect(
      isLockRequiredError(
        new CommandInvokeError(
          'database key is required, but Full Disk Access is the real problem',
          { code: ERROR_CODE_FULL_DISK_ACCESS },
        ),
      ),
    ).toBe(false)
  })

  test('falls back to the two raw backend markers for pre-envelope channels', () => {
    expect(
      isLockRequiredError(
        new Error('database key is required for encrypted archives'),
      ),
    ).toBe(true)
    expect(
      isLockRequiredError(new Error('PathKeep is currently locked (app lock)')),
    ).toBe(true)
    expect(isLockRequiredError(new Error('archive is busy'))).toBe(false)
  })

  test('returns false for non-Error values with no envelope', () => {
    expect(isLockRequiredError('database key is required')).toBe(false)
    expect(isLockRequiredError(undefined)).toBe(false)
  })
})
