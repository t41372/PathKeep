
export interface RemoteBackupConfig {
  enabled: boolean
  bucket: string
  region: string
  endpoint?: string | null
  prefix: string
  pathStyle: boolean
  uploadAfterBackup: boolean
  credentialsSaved: boolean
  lastUploadedAt?: string | null
  lastUploadedObjectKey?: string | null
  lastError?: string | null
}
export interface S3CredentialInput {
  accessKeyId: string
  secretAccessKey: string
}

export interface RemoteBackupPreview {
  bundlePath: string
  objectKey: string
  uploadUrl: string
  previewCommand: string
  manualSteps: string[]
  warnings: string[]
}

export interface RemoteBackupResult {
  uploaded: boolean
  bundlePath: string
  objectKey: string
  uploadUrl: string
  message: string
}

export interface RemoteBackupVerificationCheck {
  name: string
  status: string
  message: string
}

export interface RemoteBackupVerificationFile {
  relativePath: string
  sha256: string
  sizeBytes: number
}

export interface RemoteBackupVerification {
  bundlePath: string
  bundleVersion: string
  appVersion: string
  createdAt: string
  archiveMode: string
  objectKey: string
  restoreReady: boolean
  checks: RemoteBackupVerificationCheck[]
  warnings: string[]
  restoreSteps: string[]
  manifestFiles: RemoteBackupVerificationFile[]
}
