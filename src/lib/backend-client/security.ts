import type {
  AppSnapshot,
  KeyringStatusReport,
  RekeyPreview,
  RekeyRequest,
  SecurityStatus,
} from '../types'
import { call } from './shared'

export const securityClient = {
  getStatus: () => call<SecurityStatus>('security_status'),
  previewRekey: (request: RekeyRequest) =>
    call<RekeyPreview>('preview_rekey_archive', { request }),
  executeRekey: (request: RekeyRequest) =>
    call<AppSnapshot>('rekey_archive', { request }),
  getKeyringStatus: () => call<KeyringStatusReport>('keyring_status'),
  getDatabaseKey: () => call<string | null>('keyring_get_database_key'),
  storeDatabaseKey: (value: string) =>
    call<KeyringStatusReport>('keyring_store_database_key', { value }),
  clearDatabaseKey: () =>
    call<KeyringStatusReport>('keyring_clear_database_key'),
  resetLocalSecretVault: () => call<void>('reset_local_secret_vault'),
}
