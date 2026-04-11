import type { ArchiveMode } from './archive'

export interface KeyringStatusReport {
  available: boolean
  backend: string
  storedSecret: boolean
  message?: string | null
}
export interface RekeyRequest {
  newMode: ArchiveMode
  newKey?: string | null
}

export interface SecurityStatus {
  initialized: boolean
  mode: string
  encrypted: boolean
  unlocked: boolean
  databasePath: string
  strongholdPath: string
  rememberDatabaseKeyInKeyring: boolean
  lastSuccessfulBackupAt?: string | null
  lastRekeyAt?: string | null
  lastRekeyRunId?: number | null
  lastRekeySnapshotPath?: string | null
  keyringStatus: KeyringStatusReport
  warnings: string[]
}

export interface RekeyPreview {
  currentMode: ArchiveMode
  nextMode: ArchiveMode
  requiresNewKey: boolean
  snapshotPath: string
  tempDatabasePath: string
  steps: string[]
  warnings: string[]
}
