import type { BackupRunOverview } from './archive'

export interface AuditArtifact {
  kind: string
  path: string
  checksum?: string | null
  sizeBytes?: number | null
  createdAt: string
  reason?: string | null
}

export interface AuditRunDetail {
  run: BackupRunOverview
  trigger: string
  timezone?: string | null
  dueOnly: boolean
  profileScope: string[]
  warnings: string[]
  errorMessage?: string | null
  stats: Record<string, unknown>
  manifestPath?: string | null
  manifestHash?: string | null
  artifacts: AuditArtifact[]
}
export interface HealthCheck {
  name: string
  status: string
  message: string
}

export interface HealthReport {
  generatedAt: string
  checks: HealthCheck[]
}

export interface HealthRepairReport {
  runId?: number | null
  repairedImportAudits: number
  repairedVisibilityRows: number
  clearedDerivedRows: number
  notes: string[]
}
