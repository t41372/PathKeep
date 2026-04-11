export interface ScheduleGeneratedFile {
  relativePath: string
  absolutePath?: string | null
  purpose: string
  contents: string
}

export interface SchedulePlan {
  platform: string
  label: string
  executablePath: string
  generatedFiles: ScheduleGeneratedFile[]
  manualSteps: string[]
  applyCommands: string[][]
  rollbackCommands: string[][]
  applySupported: boolean
}

export interface ApplyResult {
  applied: boolean
  platform: string
  files: string[]
  auditPath?: string | null
  message: string
}

export interface ScheduleStatus {
  platform: string
  label: string
  dueAfterHours: number
  checkIntervalHours: number
  applySupported: boolean
  installState: string
  detectedFiles: string[]
  manualSteps: string[]
  auditPath?: string | null
  lastSuccessfulBackupAt?: string | null
  warnings: string[]
}
