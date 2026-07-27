/**
 * @file helpers.test.ts
 * @description Protects the pure helper contract behind the split Security route.
 * @module pages/security
 *
 * ## Responsibilities
 * - Verify each backend warning code maps to shipped localized Security copy.
 * - Verify uncoded and unknown-coded warnings still pass their prose through unchanged.
 * - Verify localization survives backend prose drift, which is the regression the code channel exists to stop.
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
    if (key === 'security.rekeyArchiveLockedWarning') {
      return 'The archive is locked; unlock it before applying this change.'
    }
    if (key === 'security.rekeyNewKeyRequiredWarning') {
      return 'A new password is needed before the encrypted change can be applied.'
    }
    if (key === 'security.rekeySameModeRewriteWarning') {
      return 'The mode is unchanged, so this rewrite acts as a password rotation.'
    }
    return key
  }

  test('localizes every shipped backend security warning code', () => {
    expect(
      localizeSecurityWarning(
        'encrypted-needs-password',
        'database key is required for encrypted archives',
        t,
      ),
    ).toBe(
      'Unlock this encrypted archive with the current password before reviewing history or audit data.',
    )
    expect(
      localizeSecurityWarning(
        'remember-key-no-keyring',
        'Archive is configured to remember the database key, but no native keyring backend is available on this machine.',
        t,
      ),
    ).toBe(
      'PathKeep cannot remember the archive key on this machine until a native keyring backend is available.',
    )
    expect(
      localizeSecurityWarning(
        'remembered-key-missing',
        'Archive is encrypted, but the database key is not currently stored in the system keyring.',
        t,
      ),
    ).toBe(
      'PathKeep expected the archive key in the system keyring, but it is missing right now.',
    )
  })

  test('localizes every shipped rekey preview warning code', () => {
    expect(
      localizeSecurityWarning(
        'archive-locked',
        'The archive is currently locked. Unlock it before executing the rekey.',
        t,
      ),
    ).toBe('The archive is locked; unlock it before applying this change.')
    expect(
      localizeSecurityWarning(
        'new-key-required',
        'Encrypted rekey requires a new database key before execute can run.',
        t,
      ),
    ).toBe(
      'A new password is needed before the encrypted change can be applied.',
    )
    expect(
      localizeSecurityWarning(
        'same-mode-rewrite',
        'The archive will still be rewritten because the target mode matches the current mode, which makes this a key rotation or validation pass rather than a mode switch.',
        t,
      ),
    ).toBe(
      'The mode is unchanged, so this rewrite acts as a password rotation.',
    )
  })

  test('keeps localizing when the backend prose drifts away from the shipped sentence', () => {
    expect(
      localizeSecurityWarning(
        'encrypted-needs-password',
        'open archive: a database key is required for encrypted archives!',
        t,
      ),
    ).toBe(
      'Unlock this encrypted archive with the current password before reviewing history or audit data.',
    )
  })

  test('preserves uncoded and unknown-coded warnings verbatim', () => {
    expect(localizeSecurityWarning('', 'disk is read-only', t)).toBe(
      'disk is read-only',
    )
    expect(localizeSecurityWarning(undefined, 'unknown warning', t)).toBe(
      'unknown warning',
    )
    expect(
      localizeSecurityWarning('some-future-code', 'a newer warning', t),
    ).toBe('a newer warning')
  })
})
