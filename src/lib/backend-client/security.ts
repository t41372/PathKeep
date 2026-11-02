/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `securityClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import type {
  AppSnapshot,
  KeyringStatusReport,
  RekeyPreview,
  RekeyRequest,
} from '../types'
import { call } from './shared'

/**
 * Exposes the focused client surface for security commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export const securityClient = {
  previewRekey: (request: RekeyRequest) =>
    call<RekeyPreview>('preview_rekey_archive', { request }),
  executeRekey: (request: RekeyRequest) =>
    call<AppSnapshot>('rekey_archive', { request }),
  getKeyringStatus: () => call<KeyringStatusReport>('keyring_status'),
  storeDatabaseKey: (value: string) =>
    call<KeyringStatusReport>('keyring_store_database_key', { value }),
  clearDatabaseKey: () =>
    call<KeyringStatusReport>('keyring_clear_database_key'),
}
