/**
 * Rewriting the archive with a new key: encrypt, change the password, or
 * decrypt. All three are one backend operation (`rekey_archive`) that first
 * keeps a safety copy, then swaps the rewritten file in.
 *
 * Also keeps the keychain in step: a saved password that no longer opens the
 * archive would break the next automatic unlock and every scheduled backup.
 * Not responsible for any rendering.
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'
import { appClient } from '@/lib/backend-client/app'
import { securityClient } from '@/lib/backend-client/security'
import { queryKeys } from '@/lib/query'
import type { RekeyRequest } from '@/lib/types'
import { waitForNextPaint } from '@/lib/wait-for-next-paint'

export type RekeyMode = 'encrypt' | 'change' | 'decrypt'

/** Same rule onboarding uses for a new archive password. */
export const ARCHIVE_PASSWORD_MIN_LENGTH = 10

export function rekeyRequest(mode: RekeyMode, password: string): RekeyRequest {
  return mode === 'decrypt'
    ? { newMode: 'Plaintext', newKey: null }
    : { newMode: 'Encrypted', newKey: password }
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
      password: string,
      keepInKeychain: boolean,
    ): Promise<RekeyOutcome> => {
      // Let the busy state paint before the long rewrite starts.
      await waitForNextPaint()
      const snapshot = await securityClient.executeRekey(
        rekeyRequest(mode, password),
      )
      client.setQueryData(queryKeys.snapshot, snapshot)

      let keychainFailed = false
      try {
        const remember = mode !== 'decrypt' && keepInKeychain
        if (remember) {
          await securityClient.storeDatabaseKey(password)
        } else if (snapshot.keyringStatus.storedSecret) {
          // The stored password is the old one; it would no longer open the archive.
          await securityClient.clearDatabaseKey()
        }
        if (snapshot.config.rememberDatabaseKeyInKeyring !== remember) {
          await appClient.saveConfig(
            { ...snapshot.config, rememberDatabaseKeyInKeyring: remember },
            snapshot.config,
          )
        }
      } catch {
        keychainFailed = true
      }
      client.setQueryData(queryKeys.snapshot, await appClient.getSnapshot())
      return { keychainFailed }
    },
    [client],
  )
}
