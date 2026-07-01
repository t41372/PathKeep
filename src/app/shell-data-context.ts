/**
 * This module defines the context contract that exposes shell bootstrap state and shell actions to route components.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `BusyOverlayState`
 * - `ShellDataContextValue`
 * - `ShellDataContext`
 * - `useShellData`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import { createContext, useContext } from 'react'
import type {
  AiQueueStatus,
  AppBuildInfo,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  ArchiveRecoveryReport,
  ArchiveUpgradeAssessment,
  BackupReport,
  BrowserHistoryImportRequest,
  DashboardSnapshot,
  FullArchiveRestoreReport,
  IntelligenceRuntimeSnapshot,
  SetAppLockPasscodeRequest,
  TakeoutInspection,
  TakeoutRequest,
  UnlockAppSessionRequest,
} from '../lib/types'
import type { ShellNotification, ShellTask } from './shell-tasks'

/**
 * Captures the state shape used by `BusyOverlay`.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export interface BusyOverlayState {
  label: string
  detail?: string | null
  progressLabel?: string | null
  progressValue?: number | null
  steps?: string[]
  activeStep?: number
  logLines?: string[]
  /**
   * When `true`, the shell should render the progress as an unobtrusive
   * bottom-bar strip instead of a full-screen blocking overlay. Set this
   * for long-running but user-deferrable actions (manual backup, scheduled
   * scans) so the rest of the app stays usable. Blocking actions (archive
   * unlock, irreversible derived rebuilds) leave it `false` / unset.
   */
  background?: boolean
}

export interface ShellRuntimeStatus {
  aiQueue: AiQueueStatus | null
  intelligence: IntelligenceRuntimeSnapshot | null
  loading: boolean
  error: string | null
}

/**
 * Locale-independent classification of the current shell error.
 *
 * This exists so the shell can decide whether to render remediation
 * affordances (e.g. the macOS Full Disk Access deep-link) WITHOUT re-parsing the
 * already-translated `error` string. The kind is assigned at the point the error
 * is classified from the RAW backend message, and cleared whenever the error is
 * cleared or replaced — keeping `error` and `errorKind` from drifting apart.
 */
export type ShellErrorKind =
  | 'full-disk-access'
  | 'backup'
  | 'lock-required'
  | null

/**
 * Describes the shell-owned import request that routes can start without owning progress state.
 *
 * Import execution writes archive records, so it runs through the shell task store instead
 * of remaining local to the Import route.
 */
export type ShellImportTaskRequest =
  | {
      method: 'takeout'
      request: TakeoutRequest
      expectedRecords?: number | null
      sourceLabel?: string | null
    }
  | {
      method: 'browser'
      request: BrowserHistoryImportRequest
      expectedRecords?: number | null
      sourceLabel?: string | null
    }

/**
 * Defines the value exposed through the `ShellDataContext` context.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export interface ShellDataContextValue {
  buildInfo: AppBuildInfo | null
  appLockStatus: AppLockStatus | null
  snapshot: AppSnapshot | null
  dashboard: DashboardSnapshot | null
  dashboardLoading?: boolean
  runtimeStatus?: ShellRuntimeStatus
  loading: boolean
  busyAction: string | null
  busyOverlay: BusyOverlayState | null
  error: string | null
  /**
   * Locale-independent classification of `error`. The shell gates remediation
   * affordances (e.g. the Full Disk Access deep-link) on this flag rather than
   * re-parsing the translated `error` text, so the affordance appears for every
   * shipped locale. `null` whenever there is no error or the error is unclassified.
   */
  errorKind: ShellErrorKind
  /**
   * The RAW, untranslated backend error behind `error`, preserved so the
   * failure surface can offer a copy-able diagnostic report (the displayed
   * `error` is sometimes a localized message, e.g. the Full Disk Access copy).
   * `null` whenever there is no error or no raw detail was captured.
   */
  rawError?: string | null
  notice: string | null
  archiveTasks?: ShellTask[]
  activeArchiveTask?: ShellTask | null
  latestArchiveTask?: ShellTask | null
  notifications?: ShellNotification[]
  unreadNotificationCount?: number
  refreshKey: number
  /**
   * Re-reads lock status + the app snapshot. `showSpinner` defaults to `true`
   * (the full-page loading flash for user-initiated refreshes); pass `false` for
   * background refreshes that must not freeze the current view — e.g. the
   * Explorer's one-shot snapshot refresh when a semantic-index build drains, so
   * the Smart-index callout flips to its honest "N pages indexed" ready state
   * without a jarring spinner (H-3).
   */
  refreshAppData: (showSpinner?: boolean) => Promise<void>
  refreshRuntimeStatus: () => Promise<ShellRuntimeStatus>
  saveConfig: (
    config: AppConfig,
    options?: { quiet?: boolean },
  ) => Promise<AppSnapshot>
  initializeArchive: (
    config: AppConfig,
    databaseKey?: string | null,
  ) => Promise<AppSnapshot>
  runBackup: () => Promise<BackupReport>
  runImport?: (
    request: ShellImportTaskRequest,
  ) => Promise<TakeoutInspection | ShellTask>
  setAppLockPasscode: (
    request: SetAppLockPasscodeRequest,
  ) => Promise<AppLockStatus>
  clearAppLockPasscode: () => Promise<AppLockStatus>
  lockAppSession: (reason?: string | null) => Promise<AppLockStatus>
  unlockAppSession: (request: UnlockAppSessionRequest) => Promise<AppLockStatus>
  /**
   * The structured recovery report surfaced when `initialize_archive` detects
   * an unresolvable archive drift or corruption at launch. Set to `null` when
   * the archive is healthy or after a successful restore.
   */
  recovery: ArchiveRecoveryReport | null
  /**
   * Set when a healthy archive is version-behind and the one-time upgrade
   * migration is pending. Null otherwise. Seeds the blocking
   * `ArchiveUpgradeScreen` with the cheap pre-check breakdown + the config that
   * drives `initialize_archive`.
   */
  archiveUpgrade: {
    assessment: ArchiveUpgradeAssessment
    config: AppConfig
  } | null
  /**
   * Runs after the upgrade screen's `initialize_archive` resolves: clears the
   * gate and re-bootstraps the shell (which re-assesses as not-pending and
   * drives the normal flow).
   */
  finishArchiveUpgrade: () => Promise<void>
  /**
   * Replaces the live archive with a verified safety snapshot, quarantining the
   * broken state (moved, not deleted). Clears `recovery` and re-bootstraps the
   * shell on success.
   */
  runFullArchiveRestore: (
    snapshotPath: string,
    key?: string | null,
  ) => Promise<FullArchiveRestoreReport>
  clearNotice: () => void
  clearError: () => void
  markNotificationsRead?: () => void
  dismissNotification?: (id: string) => void
}

/**
 * Holds the React context used to share shell data across the shell.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export const ShellDataContext = createContext<ShellDataContextValue | null>(
  null,
)

/**
 * Provides the `useShellData` hook.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export function useShellData() {
  const value = useContext(ShellDataContext)

  if (!value) {
    throw new Error('useShellData must be used inside ShellDataProvider')
  }

  return value
}
