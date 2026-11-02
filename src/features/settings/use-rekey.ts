/**
 * Rewriting the archive with a new key: encrypt, change the password, or
 * decrypt. All three are one backend operation (`rekey_archive`) that first
 * keeps a safety copy, then swaps the rewritten file in.
 *
 * The keychain and its setting go with the request: the backend applies them
 * under the hold that lets the window ask before quitting, so they cannot be
 * left a step behind the archive. A saved key that no longer matches would
 * break the next automatic unlock and every scheduled backup.
 * Not responsible for any rendering.
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'
import { securityClient } from '@/lib/backend-client/security'
import { queryKeys } from '@/lib/query'
import type { RekeyRequest } from '@/lib/types'
import { waitForNextPaint } from '@/lib/wait-for-next-paint'

export type RekeyMode = 'encrypt' | 'change' | 'decrypt'

/** Same rule onboarding uses for a new archive password. */
export const ARCHIVE_PASSWORD_MIN_LENGTH = 10

/** What the user typed: the new password, and the current one unless encrypting. */
export interface RekeySecrets {
  password: string
  current: string
}

/** Only changing and decrypting send the current password; a plaintext archive has none. */
export const needsCurrentPassword = (mode: RekeyMode) => mode !== 'encrypt'

export function rekeyRequest(
  mode: RekeyMode,
  { password, current }: RekeySecrets,
): RekeyRequest {
  const currentKey = needsCurrentPassword(mode) ? current : null
  return mode === 'decrypt'
    ? { newMode: 'Plaintext', newKey: null, currentKey }
    : { newMode: 'Encrypted', newKey: password, currentKey }
}

export interface RekeyOutcome {
  /** The archive changed, but the keychain could not be updated to match. */
  keychainFailed: boolean
}

export function useRekey() {
  const client = useQueryClient()

  return useCallback(
    async (
      mode: RekeyMode,
      secrets: RekeySecrets,
      keepInKeychain: boolean,
    ): Promise<RekeyOutcome> => {
      const remember = mode !== 'decrypt' && keepInKeychain
      // Let the busy state paint before the long rewrite starts.
      await waitForNextPaint()
      const snapshot = await securityClient.executeRekey({
        ...rekeyRequest(mode, secrets),
        rememberInKeychain: remember,
      })
      client.setQueryData(queryKeys.snapshot, snapshot)
      return {
        keychainFailed: snapshot.keyringStatus.storedSecret !== remember,
      }
    },
    [client],
  )
}
