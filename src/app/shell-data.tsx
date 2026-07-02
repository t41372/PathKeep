/**
 * This module owns the shell-level front-end read model, bootstrap hydration, and busy-state orchestration.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `ShellDataProvider`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import {
  useCallback,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { backend } from '../lib/backend-client'
import {
  preloadAllTimeIntelligenceOverview,
  preloadIntelligenceOverviews,
} from '../lib/core-intelligence'
import { describeError } from '../lib/errors'
import { subscribeToImportProgress } from '../lib/ipc/import-progress'
import { useI18nContext } from '../lib/i18n'
import { useProfileScope } from '../lib/profile-scope-context'
import { waitForNextPaint } from '../lib/wait-for-next-paint'
import type {
  AppBuildInfo,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  ArchiveRecoveryReport,
  ArchiveUpgradeAssessment,
  BackupProgressEvent,
  BackupReport,
  ImportProgressEvent,
  DashboardSnapshot,
  TakeoutInspection,
} from '../lib/types'
import {
  type BusyOverlayState,
  type ShellErrorKind,
  type ShellImportTaskRequest,
  ShellDataContext,
} from './shell-data-context'
import {
  createShellDataActions,
  isFullDiskAccessIssueMessage,
} from './shell-data-actions'
import {
  archiveNeedsLaunchRecovery,
  buildUninitializedDashboardFallback,
  countActiveRuntimeJobs,
  isAppLockError,
  parseArchiveRecoveryRequired,
  shouldAttemptKeyringAutoUnlock,
} from './shell-data-helpers'
import { useShellRuntimeStatus } from './shell-runtime-status'
import {
  addShellNotification,
  applyBackupProgressToTask,
  applyImportProgressToTask,
  completeBackupTask,
  completeImportTask,
  createShellTask,
  dismissShellNotification,
  failShellTask,
  findActiveArchiveTask,
  markShellNotificationsRead,
  shellNotificationLimit,
  upsertShellTask,
  type ShellNotification,
  type ShellTask,
} from './shell-tasks'

const notificationStorageKey = 'pathkeep.shellNotifications.v1'

/**
 * Provides the front-end shell read model and shell-level actions to the rest
 * of the app.
 *
 * This provider is where PathKeep turns desktop command responses, progress
 * events, app-lock state, and bootstrap errors into a single route-friendly
 * context instead of making every page talk to the backend on its own.
 */
export function ShellDataProvider({ children }: { children: ReactNode }) {
  const { setLanguagePreference, t } = useI18nContext()
  const { activeProfileId } = useProfileScope()
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [appLockStatus, setAppLockStatus] = useState<AppLockStatus | null>(null)
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null)
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null)
  const [dashboardLoading, setDashboardLoading] = useState(false)
  const [loading, setLoading] = useState(true)
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [busyOverlay, setBusyOverlay] = useState<BusyOverlayState | null>(null)
  const [error, setErrorMessage] = useState<string | null>(null)
  const [errorKind, setErrorKind] = useState<ShellErrorKind>(null)
  const errorKindRef = useRef<ShellErrorKind>(null)
  const [rawError, setRawError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [archiveTasks, setArchiveTasks] = useState<ShellTask[]>([])
  const [notifications, setNotifications] = useState<ShellNotification[]>(() =>
    readStoredNotifications(),
  )
  const [refreshKey, setRefreshKey] = useState(0)
  const [recovery, setRecovery] = useState<ArchiveRecoveryReport | null>(null)
  const [archiveUpgrade, setArchiveUpgrade] = useState<{
    assessment: ArchiveUpgradeAssessment
    config: AppConfig
  } | null>(null)
  // Latches once the cheap upgrade pre-check has confirmed the archive is at the
  // target schema, so a healthy shell never re-assesses on every refresh.
  const upgradeResolvedRef = useRef(false)
  const idleTimerRef = useRef<number | null>(null)
  const archiveTasksRef = useRef<ShellTask[]>([])
  const attemptedKeyringAutoUnlockRef = useRef(false)
  const surfacedCrashReportPathRef = useRef<string | null>(null)
  // Guards the once-per-session macOS schedule health probe so the post-upgrade
  // notification doesn't re-fire every render. The ref stores the snapshot
  // build sha we last probed against so a fresh boot of a newer build can
  // retry — useful when a user updates to a build that adds new health
  // signals.
  const surfacedScheduleHealthRef = useRef<string | null>(null)
  const dashboardRefreshTokenRef = useRef(0)
  const activeRuntimeJobsRef = useRef(0)
  const activeArchiveTask = findActiveArchiveTask(archiveTasks) ?? null
  // A stable boolean, NOT the task object: the object identity changes on every
  // progress tick, so depending on it would needlessly re-run the idle effect
  // each tick. The boolean only flips at the running↔idle transition.
  const archiveTaskActive = activeArchiveTask !== null
  const latestArchiveTask = archiveTasks[0] ?? null
  const unreadNotificationCount = notifications.filter(
    (notification) => !notification.read,
  ).length
  const { runtimeStatus, refreshRuntimeStatus, resetRuntimeStatus } =
    useShellRuntimeStatus({
      snapshot,
      refreshKey,
      t,
    })

  /**
   * Sets the shell error message and ALWAYS resets the locale-independent
   * `errorKind` classification back to `null`. Centralizing it here means every
   * `setError` call site — including the action factory — can never leave a
   * stale Full Disk Access classification attached to an unrelated error. The
   * FDA path re-asserts the kind via `setErrorKind` immediately after calling
   * this, so `error` and `errorKind` stay consistent. Stable identity (empty
   * deps) keeps the action factory memo honest.
   */
  const setError = useCallback(
    (value: string | null | ((current: string | null) => string | null)) => {
      setErrorMessage(value)
      setErrorKind(null)
      errorKindRef.current = null
      // Drop any stale raw-error detail attached to a previous failure. The
      // backup action re-sets `rawError` immediately after this call (batched
      // in the same tick), so the failure surface always shows the detail that
      // matches the message it is displaying — and unrelated errors carry none.
      setRawError(null)
    },
    [],
  )

  /**
   * Shows the shell-wide busy overlay for a long-running action.
   *
   * Centralizing this here keeps backup, initialize, lock, and config flows on
   * the same loading grammar instead of each action inventing its own spinner.
   */
  function showBusyOverlay(next: BusyOverlayState) {
    setBusyAction(next.label)
    setBusyOverlay(next)
  }

  /**
   * Clears the shell-wide busy overlay after a long-running action settles.
   */
  function clearBusyOverlay() {
    setBusyAction(null)
    setBusyOverlay(null)
  }

  function nextShellTimestamp() {
    return new Date().toISOString()
  }

  function publishNotification(input: {
    title: string
    body: string
    tone: ShellNotification['tone']
    taskId?: string | null
    href?: string | null
  }) {
    const timestamp = nextShellTimestamp()
    setNotifications((current) =>
      addShellNotification(current, {
        id: `notification:${timestamp}:${current.length}`,
        timestamp,
        title: input.title,
        body: input.body,
        tone: input.tone,
        taskId: input.taskId,
        href: input.href,
      }),
    )
  }

  function setArchiveTask(nextTask: ShellTask) {
    const nextTasks = upsertShellTask(archiveTasksRef.current, nextTask)
    archiveTasksRef.current = nextTasks
    setArchiveTasks(nextTasks)
  }

  function readArchiveTask(taskId: string) {
    /* v8 ignore next -- callers use task ids issued by beginArchiveTask; null is defensive against stale external events. */
    return archiveTasksRef.current.find((task) => task.id === taskId) ?? null
  }

  function beginArchiveTask(input: {
    kind: 'import' | 'backup'
    title: string
    detail: string
    sourceLabel?: string | null
    profileLabel?: string | null
  }): { task: ShellTask } | { blockedBy: ShellTask } {
    const activeTask = findActiveArchiveTask(archiveTasksRef.current)
    if (activeTask) {
      publishNotification({
        title: t('jobs.archiveTaskAlreadyRunningTitle'),
        body: t('jobs.archiveTaskAlreadyRunningBody', {
          task: activeTask.title,
        }),
        tone: 'warning',
        taskId: activeTask.id,
        href: '/jobs',
      })
      return { blockedBy: activeTask }
    }

    const timestamp = nextShellTimestamp()
    const task = createShellTask({
      id: `archive-${input.kind}-${Date.now()}`,
      kind: input.kind,
      title: input.title,
      detail: input.detail,
      sourceLabel: input.sourceLabel,
      profileLabel: input.profileLabel,
      timestamp,
    })
    setArchiveTask(task)
    publishNotification({
      title: t(
        input.kind === 'import'
          ? 'jobs.importTaskStartedTitle'
          : 'jobs.backupTaskStartedTitle',
      ),
      body: input.detail,
      tone: 'info',
      taskId: task.id,
      href: '/jobs',
    })
    return { task }
  }

  function updateBackupTask(taskId: string, progress: BackupProgressEvent) {
    const task = readArchiveTask(taskId)
    /* v8 ignore next -- backup progress events are subscribed only after creating this task id. */
    if (!task) return
    setArchiveTask(
      applyBackupProgressToTask(task, progress, nextShellTimestamp()),
    )
  }

  function updateImportTask(taskId: string, progress: ImportProgressEvent) {
    const task = readArchiveTask(taskId)
    /* v8 ignore next -- import progress events are subscribed only after creating this task id. */
    if (!task) return
    setArchiveTask(
      applyImportProgressToTask(task, progress, nextShellTimestamp()),
    )
  }

  function finishBackupTask(taskId: string, report: BackupReport) {
    const task = readArchiveTask(taskId)
    /* v8 ignore next -- completion is only called for task ids created by beginArchiveTask. */
    if (!task) return
    const message = backupCompletionNoticeForTask(report)
    const nextTask = completeBackupTask(
      task,
      report,
      nextShellTimestamp(),
      message,
    )
    setArchiveTask(nextTask)
    publishNotification({
      title: t('jobs.backupTaskCompleteTitle'),
      body: message,
      tone: report.warnings.length > 0 ? 'warning' : 'success',
      taskId,
      href: nextTask.resultLink!,
    })
  }

  function finishImportTask(taskId: string, result: TakeoutInspection) {
    const task = readArchiveTask(taskId)
    /* v8 ignore next -- completion is only called for task ids created by beginArchiveTask. */
    if (!task) return
    const message = t('jobs.importTaskCompleteBody', {
      imported: result.importedItems.toLocaleString(),
      duplicates: result.duplicateItems.toLocaleString(),
    })
    const nextTask = completeImportTask(
      task,
      result,
      nextShellTimestamp(),
      message,
    )
    setArchiveTask(nextTask)
    publishNotification({
      title: t('jobs.importTaskCompleteTitle'),
      body: message,
      tone: 'success',
      taskId,
      href: nextTask.resultLink!,
    })
  }

  function failArchiveTask(
    taskId: string,
    message: string,
    options?: { silent?: boolean },
  ) {
    const task = readArchiveTask(taskId)
    /* v8 ignore next -- failure is only called for task ids created by beginArchiveTask. */
    if (!task) return
    setArchiveTask(failShellTask(task, nextShellTimestamp(), message))
    // `silent` records the failure in the ledger without a danger bell — the
    // lock-required backup case relies on the unlock gate to remediate instead.
    if (options?.silent) return
    publishNotification({
      title: t('jobs.archiveTaskFailedTitle'),
      body: message,
      tone: 'danger',
      taskId,
      href: '/jobs',
    })
  }

  function backupCompletionNoticeForTask(report: BackupReport) {
    if (report.dueSkipped) {
      return report.reason ?? t('shell.manualBackupDueWindow')
    }
    if (report.run) {
      return report.warnings.some(isFullDiskAccessIssueMessage)
        ? t('shell.safariFullDiskAccessBackupWarning', {
            runId: report.run.id,
          })
        : t('shell.manualBackupFinished', { runId: report.run.id })
    }
    return t('common.complete')
  }

  // Stryker disable ArrayDeclaration: this callback only touches stable refs and browser timer APIs.
  const clearIdleTimer = useCallback(() => {
    // Stryker disable next-line ConditionalExpression: ShellDataProvider only runs in the browser; the window guard is defensive for non-DOM execution.
    if (typeof window === 'undefined' || idleTimerRef.current === null) {
      return
    }

    window.clearTimeout(idleTimerRef.current)
    idleTimerRef.current = null
  }, [])
  // Stryker restore ArrayDeclaration

  // Stryker disable ArrayDeclaration: resetRuntimeStatus is stable, so any constant dependency array preserves this callback contract.
  const clearLoadedState = useCallback(() => {
    setSnapshot(null)
    setDashboard(null)
    setDashboardLoading(false)
    resetRuntimeStatus()
  }, [resetRuntimeStatus])
  // Stryker restore ArrayDeclaration

  useEffect(() => {
    archiveTasksRef.current = archiveTasks
  }, [archiveTasks])

  useEffect(() => {
    storeNotifications(notifications)
  }, [notifications])

  const refreshDashboardSnapshot = useCallback(
    async (
      nextSnapshot: AppSnapshot,
      options: { surfaceErrors?: boolean } = {},
    ) => {
      const { surfaceErrors = false } = options
      // Stryker disable next-line ArithmeticOperator: the token only needs a new unique value; increasing or decreasing is equivalent for equality checks.
      const refreshToken = dashboardRefreshTokenRef.current + 1
      dashboardRefreshTokenRef.current = refreshToken
      setDashboardLoading(true)

      try {
        const nextDashboard = await backend.loadDashboardSnapshot()
        if (dashboardRefreshTokenRef.current !== refreshToken) {
          return
        }
        setDashboard(nextDashboard)
      } catch (dashboardError) {
        if (dashboardRefreshTokenRef.current !== refreshToken) {
          return
        }

        // Only surface a dashboard error when no backup failure is in flight.
        // A fire-and-forget dashboard refresh can resolve after the backup
        // action's finally block re-asserted setError/setRawError — clobbering
        // the backup failure with an unrelated dashboard error. The ref
        // captures the latest errorKind without adding a dependency that would
        // change the callback identity (and thus re-create refreshAppData)
        // on every error state change.
        const backupErrorInFlight =
          errorKindRef.current === 'backup' ||
          errorKindRef.current === 'full-disk-access'
        if (
          nextSnapshot.config.initialized &&
          surfaceErrors &&
          !backupErrorInFlight
        ) {
          setError(describeError(dashboardError, 'load_dashboard_snapshot'))
          return
        }

        setDashboard(buildUninitializedDashboardFallback(nextSnapshot))
      } finally {
        if (dashboardRefreshTokenRef.current === refreshToken) {
          setDashboardLoading(false)
        }
      }
    },
    [setError],
  )

  const refreshAppData = useCallback(
    async (showSpinner = true) => {
      if (showSpinner) {
        setLoading(true)
        await waitForNextPaint()
      }
      setError(null)
      setRecovery(null)
      setArchiveUpgrade(null)

      try {
        const [nextLockStatus, nextBuildInfo] = await Promise.all([
          backend.loadAppLockStatus(),
          backend.getAppBuildInfo(),
        ])
        setAppLockStatus(nextLockStatus)
        setBuildInfo(nextBuildInfo)

        if (nextLockStatus.locked) {
          clearLoadedState()
          setNotice(null)
          setRefreshKey((value) => value + 1)
          return
        }

        let nextSnapshot = await backend.getAppSnapshot()
        if (
          !attemptedKeyringAutoUnlockRef.current &&
          shouldAttemptKeyringAutoUnlock(nextSnapshot)
        ) {
          attemptedKeyringAutoUnlockRef.current = true
          try {
            const key = await backend.keyringGetDatabaseKey()
            if (key) {
              await backend.setSessionDatabaseKey(key)
              // Best-effort reconcile: self-heals any drifted encryption state
              // right after auto-unlock so the next backup doesn't fail on a
              // stale mode mismatch. Fire-and-forget; swallow all errors.
              void backend.reconcileArchiveEncryption().catch(() => undefined)
              nextSnapshot = await backend.getAppSnapshot()
            }
          } catch {
            // Auto-unlock is best-effort; keep the locked snapshot and let the
            // user fall back to the explicit unlock controls if keychain access
            // is denied or unavailable for this session.
          }
        }

        if (archiveNeedsLaunchRecovery(nextSnapshot)) {
          try {
            nextSnapshot = await backend.initializeArchive(nextSnapshot.config)
          } catch (recoveryError) {
            const report = parseArchiveRecoveryRequired(recoveryError)
            if (report) {
              setRecovery(report)
              setSnapshot(nextSnapshot)
              setAppLockStatus(nextSnapshot.appLockStatus)
              setBuildInfo(nextBuildInfo)
              setRefreshKey((value) => value + 1)
              return
            }
            throw recoveryError
          }
        }

        // Best-effort one-time upgrade gate. `assess_archive_upgrade` is cheap
        // (COUNTs only); when it reports a pending v0.2.0 → v0.3.0 migration we
        // hand off to the blocking `ArchiveUpgradeScreen` (which drives
        // `initialize_archive` with live progress) instead of freezing behind an
        // opaque busy overlay. Latched via `upgradeResolvedRef` so a healthy
        // shell never re-assesses on every refresh.
        if (
          !upgradeResolvedRef.current &&
          nextSnapshot.config.initialized &&
          nextSnapshot.archiveStatus.unlocked
        ) {
          let assessment: ArchiveUpgradeAssessment | null = null
          try {
            assessment = await backend.assessArchiveUpgrade()
          } catch {
            // Best-effort: an assess failure (e.g. the browser-preview fixture,
            // which throws on unknown commands) must never block launch — treat
            // as not-pending and continue the normal flow.
          }
          if (assessment && assessment.pending) {
            setLanguagePreference(nextSnapshot.config.preferredLanguage, {
              persist: false,
            })
            setArchiveUpgrade({ assessment, config: nextSnapshot.config })
            setBuildInfo(nextBuildInfo)
            setAppLockStatus(nextSnapshot.appLockStatus)
            setSnapshot(nextSnapshot)
            setRefreshKey((value) => value + 1)
            return
          }
          upgradeResolvedRef.current = true
        }

        setLanguagePreference(nextSnapshot.config.preferredLanguage, {
          persist: false,
        })
        setSnapshot(nextSnapshot)
        setBuildInfo(nextBuildInfo)
        setAppLockStatus(nextSnapshot.appLockStatus)
        setRefreshKey((value) => value + 1)
        void refreshDashboardSnapshot(nextSnapshot, { surfaceErrors: true })
      } catch (nextError) {
        if (isAppLockError(nextError)) {
          try {
            const nextLockStatus = await backend.loadAppLockStatus()
            if (nextLockStatus.locked) {
              setAppLockStatus(nextLockStatus)
              clearLoadedState()
              setNotice(null)
              setRefreshKey((value) => value + 1)
              return
            }
          } catch {
            // Fall back to the generic error path below if the lock refresh fails.
          }
        }
        setError(describeError(nextError, 'refresh_shell_snapshot'))
        throw nextError
      } finally {
        if (showSpinner) {
          setLoading(false)
        }
      }
    },
    [
      clearLoadedState,
      refreshDashboardSnapshot,
      setLanguagePreference,
      setError,
    ],
  )

  const armIdleDeadline = useEffectEvent((idleTimeoutMinutes: number) => {
    clearIdleTimer()

    idleTimerRef.current = window.setTimeout(() => {
      void backend
        .lockAppSession('idle-timeout')
        .then((nextStatus) => {
          setAppLockStatus(nextStatus)
          clearLoadedState()
          setNotice(null)
          setError(null)
          setRefreshKey((value) => value + 1)
        })
        .catch((nextError) => {
          setError(describeError(nextError, 'lock_app_session'))
        })
    }, idleTimeoutMinutes * 60_000)
  })

  useEffect(() => {
    void refreshAppData().catch(() => undefined)
  }, [refreshAppData])

  useEffect(() => {
    const crashReportPath =
      snapshot?.runtimeDiagnostics.latestCrashReport?.path ?? null
    if (
      !crashReportPath ||
      surfacedCrashReportPathRef.current === crashReportPath ||
      notice !== null
    ) {
      return
    }
    surfacedCrashReportPathRef.current = crashReportPath
    setNotice(t('shell.runtimeCrashNotice'))
  }, [notice, snapshot?.runtimeDiagnostics.latestCrashReport?.path, t])

  // Stale-plist startup probe. The macOS 26 LaunchService changes broke any
  // v0.2.0 schedule that launched the binary directly; users upgrading into
  // this build need to re-apply once so the new plist (which routes through
  // `/usr/bin/open`) takes over. The Schedule status command already returns
  // `installState: "mismatch"` for that case; we surface it proactively as
  // a shell notification so users don't have to discover it themselves by
  // wondering why backups stopped firing.
  //
  // `useEffectEvent` wraps the publish call so the effect doesn't need to
  // re-run every render just to capture a fresh `publishNotification`
  // closure — which would also defeat the ref guard.
  const publishStaleScheduleNotice = useEffectEvent(() => {
    publishNotification({
      title: t('shell.scheduleStaleAfterUpgradeTitle'),
      body: t('shell.scheduleStaleAfterUpgradeBody'),
      tone: 'warning',
      href: '/settings#schedule',
    })
  })
  useEffect(() => {
    if (!snapshot?.config.initialized || !snapshot.archiveStatus.unlocked) {
      return
    }
    const guardKey = snapshot.archiveStatus.databasePath
    if (surfacedScheduleHealthRef.current === guardKey) return
    surfacedScheduleHealthRef.current = guardKey
    const cancelToken = { cancelled: false }
    void runScheduleHealthProbe(cancelToken, publishStaleScheduleNotice)
    return () => {
      cancelToken.cancelled = true
    }
  }, [
    snapshot?.archiveStatus.databasePath,
    snapshot?.archiveStatus.unlocked,
    snapshot?.config.initialized,
  ])

  useEffect(() => {
    // Suppress the idle timer while a blocking busy action runs OR while an
    // archive task (import/backup) is in flight. `runImport` never sets
    // busyAction, so without `archiveTaskActive` a long import could hit the
    // idle timeout and lock the archive out from under an in-flight encrypted
    // write. This is a DEFER, not a cancel: the effect re-arms once the task
    // clears (archiveTaskActive is a dependency), so we still lock promptly if
    // the user stays idle after the write settles.
    if (
      !appLockStatus?.enabled ||
      appLockStatus.locked ||
      busyAction !== null ||
      archiveTaskActive
    ) {
      clearIdleTimer()
      return
    }

    const scheduleIdleReset = () => {
      armIdleDeadline(appLockStatus.idleTimeoutMinutes)
    }

    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        armIdleDeadline(appLockStatus.idleTimeoutMinutes)
      }
    }

    for (const eventName of ['pointerdown', 'keydown', 'mousemove', 'focus']) {
      window.addEventListener(eventName, scheduleIdleReset, { passive: true })
    }
    document.addEventListener('visibilitychange', handleVisibility)
    armIdleDeadline(appLockStatus.idleTimeoutMinutes)

    return () => {
      clearIdleTimer()
      for (const eventName of [
        'pointerdown',
        'keydown',
        'mousemove',
        'focus',
      ]) {
        window.removeEventListener(eventName, scheduleIdleReset)
      }
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [
    appLockStatus?.enabled,
    appLockStatus?.idleTimeoutMinutes,
    appLockStatus?.locked,
    busyAction,
    archiveTaskActive,
    clearIdleTimer,
  ])

  useEffect(() => {
    const activeRuntimeJobs = countActiveRuntimeJobs(runtimeStatus)
    const hadActiveRuntimeJobs = activeRuntimeJobsRef.current > 0
    activeRuntimeJobsRef.current = activeRuntimeJobs

    if (
      !hadActiveRuntimeJobs ||
      activeRuntimeJobs > 0 ||
      !snapshot?.config.initialized ||
      !snapshot.archiveStatus.unlocked
    ) {
      return
    }

    setRefreshKey((value) => value + 1)
    void refreshDashboardSnapshot(snapshot, { surfaceErrors: true })
  }, [refreshDashboardSnapshot, runtimeStatus, snapshot])

  useEffect(() => {
    if (!snapshot?.config.initialized || !snapshot.archiveStatus.unlocked) {
      return
    }
    // Warm the `/intelligence` overviews so the route paints from cache. The
    // route requests the active profile scope (archive-wide when null), so a
    // returning scoped user needs that scope warmed — warming only `null` would
    // seed a key the page never reads. For the active scope we warm all-time
    // (the default, snapshot-backed) first and then idle-stagger the bounded
    // presets (month, quarter, year) the user can switch to; those bounded warms
    // are skipped for the archive-wide companion below so their snapshot-less
    // cold recompute is paid at most once per scope. When a specific profile is
    // active we still warm the archive-wide all-time default (cheap) so clearing
    // the scope stays instant. `refreshKey` bumps after a rebuild drains, so this
    // re-warms whenever the archive changed (or the scope switches); the cleanup
    // cancels every pending warm if a dependency changes first.
    const cancelActive = preloadIntelligenceOverviews(activeProfileId)
    const cancelArchiveWide =
      activeProfileId === null ? null : preloadAllTimeIntelligenceOverview(null)
    return () => {
      cancelActive()
      cancelArchiveWide?.()
    }
  }, [
    activeProfileId,
    refreshKey,
    snapshot?.archiveStatus.unlocked,
    snapshot?.config.initialized,
  ])

  async function runImport(
    request: ShellImportTaskRequest,
  ): Promise<TakeoutInspection | ShellTask> {
    const sourceLabel =
      request.sourceLabel ??
      (request.method === 'browser'
        ? (request.request.browserName ?? request.request.profileName ?? null)
        : request.request.sourcePath)
    const started = beginArchiveTask({
      kind: 'import',
      title:
        request.method === 'browser'
          ? t('jobs.importBrowserTaskTitle')
          : t('jobs.importTakeoutTaskTitle'),
      detail: t('jobs.importTaskStartedBody', {
        source: sourceLabel ?? request.request.sourcePath,
      }),
      sourceLabel,
      profileLabel:
        request.method === 'browser'
          ? (request.request.profileName ?? request.request.profileId ?? null)
          : null,
    })
    if ('blockedBy' in started) {
      return started.blockedBy
    }

    let unsubscribe: (() => void) | null = null
    try {
      await waitForNextPaint()
      unsubscribe = await subscribeToImportProgress((progress) => {
        updateImportTask(started.task.id, progress)
      })
      const result =
        request.method === 'takeout'
          ? await backend.importTakeout(request.request)
          : await backend.importBrowserHistory(request.request)
      finishImportTask(started.task.id, result)
      return result
    } catch (nextError) {
      const message = describeError(nextError, 'import_history')
      failArchiveTask(started.task.id, message)
      throw nextError
    } finally {
      /* v8 ignore next -- failed progress subscription leaves no listener to unsubscribe. */
      unsubscribe?.()
    }
  }

  const {
    saveConfig,
    initializeArchive,
    runBackup,
    setAppLockPasscode,
    clearAppLockPasscode,
    lockAppSession,
    unlockAppSession,
  } = createShellDataActions({
    t,
    setLanguagePreference,
    refreshDashboardSnapshot,
    refreshAppData,
    clearLoadedState,
    showBusyOverlay,
    clearBusyOverlay,
    setNotice,
    setError,
    setErrorKind: (value: ShellErrorKind) => {
      setErrorKind(value)
      errorKindRef.current = value
    },
    setRawError,
    setSnapshot,
    setAppLockStatus,
    setRefreshKey,
    archiveTasks: {
      beginBackupTask: () =>
        beginArchiveTask({
          kind: 'backup',
          title: t('jobs.backupTaskTitle'),
          detail: t('jobs.backupTaskStartedBody'),
        }),
      updateBackupTask,
      finishBackupTask,
      failBackupTask: failArchiveTask,
    },
  })

  const finishArchiveUpgrade = useCallback(async () => {
    // refreshAppData clears archiveUpgrade at its start, re-assesses (now not
    // pending → migration done), and drives the normal shell.
    await refreshAppData(false)
  }, [refreshAppData])

  const runFullArchiveRestore = useCallback(
    async (snapshotPath: string, key?: string | null) => {
      const report = await backend.runFullArchiveRestore(
        { snapshotPath },
        key ?? null,
      )
      // Refresh first so a failure keeps the recovery screen visible (setRecovery
      // inside refreshAppData also clears it on success, so the explicit call below
      // is redundant on the happy path but ensures cleanup on a partial failure).
      await refreshAppData(false)
      setRecovery(null)
      return report
    },
    [refreshAppData],
  )

  return (
    <ShellDataContext.Provider
      value={{
        buildInfo,
        appLockStatus,
        snapshot,
        dashboard,
        dashboardLoading,
        runtimeStatus,
        loading,
        busyAction,
        busyOverlay,
        error,
        errorKind,
        rawError,
        notice,
        archiveTasks,
        activeArchiveTask,
        latestArchiveTask,
        notifications,
        unreadNotificationCount,
        refreshKey,
        refreshAppData: (showSpinner) => refreshAppData(showSpinner),
        refreshRuntimeStatus: () => refreshRuntimeStatus(),
        saveConfig,
        initializeArchive,
        runBackup,
        runImport,
        setAppLockPasscode,
        clearAppLockPasscode,
        lockAppSession,
        unlockAppSession,
        recovery,
        archiveUpgrade,
        finishArchiveUpgrade,
        runFullArchiveRestore,
        clearNotice: () => setNotice(null),
        clearError: () => setError(null),
        markNotificationsRead: () =>
          setNotifications((current) => markShellNotificationsRead(current)),
        dismissNotification: (id: string) =>
          setNotifications((current) => dismissShellNotification(current, id)),
      }}
    >
      {children}
    </ShellDataContext.Provider>
  )
}

function readStoredNotifications(): ShellNotification[] {
  /* v8 ignore next -- shell-data runs in a browser window; this is defensive for non-DOM imports. */
  if (typeof window === 'undefined') {
    return []
  }

  try {
    const raw = window.localStorage.getItem(notificationStorageKey)
    if (!raw) {
      return []
    }
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) {
      return []
    }
    return parsed.filter(isShellNotification).slice(0, shellNotificationLimit)
  } catch {
    return []
  }
}

function storeNotifications(notifications: readonly ShellNotification[]) {
  /* v8 ignore next -- shell-data runs in a browser window; this is defensive for non-DOM imports. */
  if (typeof window === 'undefined') {
    return
  }

  try {
    window.localStorage.setItem(
      notificationStorageKey,
      JSON.stringify(notifications.slice(0, shellNotificationLimit)),
    )
  } catch {
    // localStorage can be blocked by privacy settings; notifications still work in memory.
  }
}

function isShellNotification(value: unknown): value is ShellNotification {
  if (!value || typeof value !== 'object') {
    return false
  }
  const candidate = value as Partial<ShellNotification>
  return (
    typeof candidate.id === 'string' &&
    typeof candidate.timestamp === 'string' &&
    typeof candidate.title === 'string' &&
    typeof candidate.body === 'string' &&
    typeof candidate.tone === 'string' &&
    typeof candidate.read === 'boolean'
  )
}

/**
 * Runs the macOS schedule-status probe and fires the "re-apply schedule"
 * notification when the installed plist no longer matches the current PathKeep
 * build's plan. Pulled out of the inline IIFE so v8 coverage can see each
 * branch (the inline `async () => {}` form was reporting the whole body as
 * uncovered even when exercised by tests).
 */
async function runScheduleHealthProbe(
  cancelToken: { cancelled: boolean },
  publishStaleScheduleNotice: () => void,
) {
  try {
    const status = await backend.scheduleStatus('macos')
    if (cancelToken.cancelled) return
    if (status.installState !== 'mismatch') return
    publishStaleScheduleNotice()
  } catch {
    // Schedule probes are best-effort — non-macOS hosts and devices without
    // a configured schedule are expected to no-op here. We don't want a
    // startup failure path that blocks the rest of the shell.
  }
}
