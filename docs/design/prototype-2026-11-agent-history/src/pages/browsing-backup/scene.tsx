/**
 * Prototype-only: the states a reviewer can switch this page into, and the
 * stand-in for the app's shared backup runner (`app/backup-runner`), which
 * every card reads to know whether a backup is running.
 */
import { createContext, useContext } from 'react'

export type SourcesScene = 'normal' | 'fullDiskAccess' | 'discoveryError' | 'empty' | 'statsLoading' | 'statsFailed'

export interface BackupProgress {
  completedProfiles: number
  totalProfiles: number
  sourceLabel: string | null
  processedRecords: number | null
  totalRecords: number | null
  progressPercent: number | null
}

export interface Scene {
  /** `useBackupRunner()`: one backup at a time for the whole app. */
  running: boolean
  /** Null between the start and the first progress event. */
  progress: BackupProgress | null
  sources: SourcesScene
  scheduleOn: boolean
  /** The desktop app has native pickers; a plain browser types a path instead. */
  nativeFiles: boolean
  /** A file is being dragged over the window. */
  hovering: boolean
  /** The backup started from this page has ended (the run is listed as finished). */
  finished: boolean
  runsEmpty: boolean
  importsEmpty: boolean
}

export const initialScene: Scene = {
  running: false,
  progress: null,
  sources: 'normal',
  scheduleOn: true,
  nativeFiles: true,
  hovering: false,
  finished: false,
  runsEmpty: false,
  importsEmpty: false,
}

export const SceneContext = createContext<Scene>(initialScene)

export const useScene = () => useContext(SceneContext)
