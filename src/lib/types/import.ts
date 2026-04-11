export interface TakeoutRequest {
  sourcePath: string
  dryRun: boolean
}

export interface TakeoutFileReport {
  path: string
  kind: string
  status: string
  records: number
}

export interface TakeoutPreviewEntry {
  sourcePath: string
  url: string
  title?: string | null
  visitedAt: string
  sourceVisitId: number
  status: string
}

export interface ImportBatchOverview {
  id: number
  sourceKind: string
  sourcePath: string
  profileId: string
  createdAt: string
  importedAt?: string | null
  revertedAt?: string | null
  status: string
  candidateItems: number
  importedItems: number
  duplicateItems: number
  visibleItems: number
  auditPath?: string | null
  gitCommit?: string | null
}

export interface ImportBatchDetail {
  batch: ImportBatchOverview
  previewEntries: TakeoutPreviewEntry[]
  recognizedFiles: TakeoutFileReport[]
  quarantinedFiles: TakeoutFileReport[]
  notes: string[]
}

export interface TakeoutInspection {
  dryRun: boolean
  sourcePath: string
  recognizedFiles: TakeoutFileReport[]
  quarantinedFiles: TakeoutFileReport[]
  previewEntries: TakeoutPreviewEntry[]
  candidateItems: number
  importedItems: number
  duplicateItems: number
  notes: string[]
  importBatch?: ImportBatchOverview | null
}
