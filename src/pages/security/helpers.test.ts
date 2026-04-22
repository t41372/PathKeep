/**
 * @file helpers.test.ts
 * @description Protects the pure helper contract behind the split Security route.
 * @module pages/security
 *
 * ## Responsibilities
 * - Verify known backend warning strings map to shipped localized Security copy.
 * - Verify unknown warnings still pass through unchanged.
 *
 * ## Not responsible for
 * - Rendering the Security route
 * - Verifying unlock, keyring, or rekey side effects
 *
 * ## Dependencies
 * - Depends only on `helpers.ts`.
 *
 * ## Performance notes
 * - Pure helper coverage keeps the route split verifiable without mounting the page shell.
 */

import { describe, expect, test } from 'vitest'
import { localizeSecurityWarning } from './helpers'

describe('security helpers', () => {
  const t = (key: string) => {
    if (key === 'security.encryptedArchiveNeedsPasswordWarning') {
      return 'Unlock this encrypted archive with the current password before reviewing history or audit data.'
    }
    if (key === 'security.rememberKeyNeedsKeychainWarning') {
      return 'PathKeep cannot remember the archive key on this machine until a native keyring backend is available.'
    }
    if (key === 'security.rememberedKeyMissingWarning') {
      return 'PathKeep expected the archive key in the system keyring, but it is missing right now.'
    }
    return key
  }

  test('localizes exact known backend security warnings', () => {
    expect(
      localizeSecurityWarning(
        'database key is required for encrypted archives',
        t,
      ),
    ).toBe(
      'Unlock this encrypted archive with the current password before reviewing history or audit data.',
    )
    expect(
      localizeSecurityWarning(
        'Archive is configured to remember the database key, but no native keyring backend is available on this machine.',
        t,
      ),
    ).toBe(
      'PathKeep cannot remember the archive key on this machine until a native keyring backend is available.',
    )
    expect(
      localizeSecurityWarning(
        'Archive is encrypted, but the database key is not currently stored in the system keyring.',
        t,
      ),
    ).toBe(
      'PathKeep expected the archive key in the system keyring, but it is missing right now.',
    )
  })

  test('localizes substring matches without requiring an exact backend payload', () => {
    expect(
      localizeSecurityWarning(
        'warning: database key is required for encrypted archives (session locked)',
        t,
      ),
    ).toBe(
      'Unlock this encrypted archive with the current password before reviewing history or audit data.',
    )
  })

  test('preserves unknown warnings verbatim', () => {
    expect(localizeSecurityWarning('unknown warning', t)).toBe(
      'unknown warning',
    )
  })
})
