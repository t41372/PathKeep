/**
 * @file titlebar-dragstrip.test.tsx
 * @description Regression guard for the single, app-wide macOS window-drag strip.
 * @module app/index-tests
 *
 * ## What this suite covers
 * - The global drag strip (`data-testid="app-titlebar-dragstrip"`) renders on
 *   EVERY AppBody branch — recovery gate, upgrade gate, and the router
 *   (main-shell) — so every screen can drag the frameless macOS window, not just
 *   the main shell. This is the "no per-screen chrome afterthought" guard.
 * - The strip carries `data-tauri-drag-region` (Tauri v2 only starts a drag from
 *   a mousedown on an element that itself carries the attribute) and
 *   `data-titlebar-overlay="true"` (so its 28px height resolves from the single
 *   source-of-truth token rule).
 * - Exactly ONE strip exists in the document (no double strip now that the shell
 *   no longer renders its own copy).
 * - Off the macOS overlay platform (browser/Windows/Linux) NO strip renders.
 */

import { render, screen } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { createMemoryRouter } from 'react-router-dom'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../shell-data-context'
import type {
  AppConfig,
  ArchiveRecoveryReport,
  ArchiveUpgradeAssessment,
} from '../../lib/types'
import * as runtime from '../../lib/runtime'
import { AppBody } from '../index'

// ──────────────────────────────────────────────────────────────────────────────
// Mock the two gate screens so the test focuses purely on strip presence per
// branch, without booting their real IPC-driven internals.
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../components/archive-recovery-screen', () => ({
  ArchiveRecoveryScreen: () => <div data-testid="recovery-branch" />,
}))
vi.mock('../../components/archive-upgrade-screen', () => ({
  ArchiveUpgradeScreen: () => <div data-testid="upgrade-branch" />,
}))

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

function makeShellValue(
  overrides: Partial<ShellDataContextValue> = {},
): ShellDataContextValue {
  return {
    buildInfo: null,
    appLockStatus: null,
    snapshot: null,
    dashboard: null,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    rawError: null,
    notice: null,
    refreshKey: 0,
    errorKind: null,
    recovery: null,
    archiveUpgrade: null,
    finishArchiveUpgrade: vi.fn().mockResolvedValue(undefined),
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue({}),
    saveConfig: vi.fn().mockResolvedValue({}),
    initializeArchive: vi.fn(),
    runBackup: vi.fn().mockResolvedValue({}),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    lockAppSession: vi.fn().mockResolvedValue({}),
    unlockAppSession: vi.fn().mockResolvedValue({}),
    clearNotice: vi.fn(),
    clearError: vi.fn(),
    runFullArchiveRestore: vi.fn().mockResolvedValue({}),
    ...overrides,
  } as ShellDataContextValue
}

function makeRecovery(): ArchiveRecoveryReport {
  return {
    kind: 'atRestDriftUnresolved',
    configMode: 'Plaintext',
    availableSnapshots: ['/snap.sqlite'],
    recoverySnapshots: [],
    detail: 'archive drift',
  }
}

function makeUpgrade(): {
  assessment: ArchiveUpgradeAssessment
  config: AppConfig
} {
  return {
    assessment: {
      pending: true,
      currentSchemaVersion: 14,
      targetSchemaVersion: 16,
      phases: [],
    },
    config: { initialized: true } as AppConfig,
  }
}

function renderAppBody(overrides: Partial<ShellDataContextValue> = {}) {
  const router = createMemoryRouter([
    { path: '*', element: <div data-testid="router-content" /> },
  ])
  return render(
    <ShellDataContext.Provider value={makeShellValue(overrides)}>
      <AppBody router={router} />
    </ShellDataContext.Provider>,
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('AppBody global titlebar drag strip', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('renders the global drag strip on the recovery branch under the macOS overlay', () => {
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderAppBody({ recovery: makeRecovery() })
    // The recovery gate owns the screen, yet the window is still draggable.
    expect(screen.getByTestId('recovery-branch')).toBeInTheDocument()
    expect(screen.getByTestId('app-titlebar-dragstrip')).toBeInTheDocument()
  })

  test('renders the global drag strip on the upgrade branch under the macOS overlay', () => {
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderAppBody({ archiveUpgrade: makeUpgrade() })
    expect(screen.getByTestId('upgrade-branch')).toBeInTheDocument()
    expect(screen.getByTestId('app-titlebar-dragstrip')).toBeInTheDocument()
  })

  test('renders the global drag strip on the router (main-shell) branch under the macOS overlay', () => {
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderAppBody({ recovery: null, archiveUpgrade: null })
    expect(screen.getByTestId('router-content')).toBeInTheDocument()
    expect(screen.getByTestId('app-titlebar-dragstrip')).toBeInTheDocument()
  })

  test('renders the drag strip with data-tauri-drag-region and exactly once (no double strip)', () => {
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(true)
    renderAppBody()
    const strip = screen.getByTestId('app-titlebar-dragstrip')
    expect(strip).toHaveAttribute('data-tauri-drag-region')
    expect(strip).toHaveAttribute('data-titlebar-overlay', 'true')
    expect(document.querySelectorAll('.pk-titlebar-dragstrip')).toHaveLength(1)
  })

  test('renders NO drag strip off the macOS overlay platform', () => {
    vi.spyOn(runtime, 'hasMacOverlayTitlebar').mockReturnValue(false)
    renderAppBody()
    expect(
      screen.queryByTestId('app-titlebar-dragstrip'),
    ).not.toBeInTheDocument()
    expect(document.querySelector('.pk-titlebar-dragstrip')).toBeNull()
  })
})
