/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `remoteClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import type {
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
  S3CredentialInput,
} from '../types'
import { call } from './shared'

/**
 * Exposes the focused client surface for remote commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export const remoteClient = {
  storeCredentials: (credentials: S3CredentialInput) =>
    call<void>('store_s3_credentials', { credentials }),
  clearCredentials: () => call<void>('clear_s3_credentials'),
  previewBackup: () => call<RemoteBackupPreview>('preview_remote_backup'),
  runBackup: () => call<RemoteBackupResult>('run_remote_backup'),
  verifyBackup: (bundlePath: string) =>
    call<RemoteBackupVerification>('verify_remote_backup', { bundlePath }),
}
