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
  AppBuildInfo,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  BackupReport,
  DashboardSnapshot,
  SetAppLockPasscodeRequest,
  UnlockAppSessionRequest,
} from '../lib/types'

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
  loading: boolean
  busyAction: string | null
  busyOverlay: BusyOverlayState | null
  error: string | null
  notice: string | null
  refreshKey: number
  refreshAppData: () => Promise<void>
  saveConfig: (config: AppConfig) => Promise<AppSnapshot>
  initializeArchive: (
    config: AppConfig,
    databaseKey?: string | null,
  ) => Promise<AppSnapshot>
  runBackup: () => Promise<BackupReport>
  setAppLockPasscode: (
    request: SetAppLockPasscodeRequest,
  ) => Promise<AppLockStatus>
  clearAppLockPasscode: () => Promise<AppLockStatus>
  lockAppSession: (reason?: string | null) => Promise<AppLockStatus>
  unlockAppSession: (request: UnlockAppSessionRequest) => Promise<AppLockStatus>
  clearNotice: () => void
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
