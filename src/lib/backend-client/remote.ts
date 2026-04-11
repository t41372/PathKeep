import type {
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
  S3CredentialInput,
} from '../types'
import { call } from './shared'

export const remoteClient = {
  storeCredentials: (credentials: S3CredentialInput) =>
    call<void>('store_s3_credentials', { credentials }),
  clearCredentials: () => call<void>('clear_s3_credentials'),
  previewBackup: () => call<RemoteBackupPreview>('preview_remote_backup'),
  runBackup: () => call<RemoteBackupResult>('run_remote_backup'),
  verifyBackup: (bundlePath: string) =>
    call<RemoteBackupVerification>('verify_remote_backup', { bundlePath }),
}
