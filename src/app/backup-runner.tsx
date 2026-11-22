/**
 * Runs a manual backup and reports progress. Lives above the router so Home,
 * Backup, the command palette and onboarding share one run and one progress
 * state. A backup that skipped browsing because it is paused or off says
 * so, from here and from the menu bar.
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { toast } from 'sonner'
import { appClient } from '@/lib/backend-client/app'
import { archiveClient } from '@/lib/backend-client/archive'
import { countRunWarnings } from '@/features/backup/run-codes'
import { openRun } from '@/features/backup/run-link'
import { describeError } from '@/lib/errors'
import {
  commandErrorCode,
  isFullDiskAccessError,
} from '@/lib/ipc/command-error'
import { subscribeToBackupProgress } from '@/lib/ipc/backup-progress'
import { subscribeToBackupFinished } from '@/lib/ipc/desktop-events'
import { useI18n } from '@/lib/i18n'
import { kindSkipReason, kindSkipSentence } from './backup-skips'
import { queryClient, queryKeys } from '@/lib/query'
import type { BackupProgressEvent, BackupReport } from '@/lib/types'

/** What a run ended with, for callers that show the result in place. */
export type BackupOutcome =
  | { ok: true; report: BackupReport }
  | { ok: false; error: unknown; report?: BackupReport }

interface RunOptions {
  /** The browsing page's own button: it backs browsing up even while paused. */
  kindSpecific?: boolean
  /** Onboarding shows the result itself, so it turns the toasts off. */
  toast?: boolean
  /** The caller says itself that browsing was skipped ("Back up everything"). */
  quietSkip?: boolean
}

interface BackupRunnerValue {
  running: boolean
  progress: BackupProgressEvent | null
  lastReport: BackupReport | null
  /** Resolves to null when a run is already in progress. */
  run: (options?: RunOptions) => Promise<BackupOutcome | null>
}

const BackupRunnerContext = createContext<BackupRunnerValue | null>(null)

/** Refreshes everything that depends on archive contents after new data lands. */
export async function refreshAfterArchiveChange() {
  const snapshot = await appClient.getSnapshot()
  queryClient.setQueryData(queryKeys.snapshot, snapshot)
  await queryClient.invalidateQueries({ queryKey: queryKeys.archiveData })
  await queryClient.invalidateQueries({ queryKey: queryKeys.schedule })
}

export function BackupRunnerProvider({ children }: { children: ReactNode }) {
  const { t } = useI18n()
  const [running, setRunning] = useState(false)
  const [progress, setProgress] = useState<BackupProgressEvent | null>(null)
  const [lastReport, setLastReport] = useState<BackupReport | null>(null)
  const runningRef = useRef(false)

  const announceDone = useCallback(
    (report: BackupReport) => {
      const added = report.run?.newVisits ?? 0
      const incomplete = report.run?.status === 'partial'
      const title =
        added > 0
          ? t(incomplete ? 'shell.backup.partial' : 'shell.backup.done', {
              count: added,
            })
          : t(
              incomplete
                ? 'shell.backup.partialNothingNew'
                : 'shell.backup.doneNothingNew',
            )
      const warnings = countRunWarnings(report)
      const runId = report.run?.id
      if (!incomplete && (warnings === 0 || runId === undefined)) {
        toast.success(title)
        return
      }
      toast.warning(title, {
        description: t('shell.backup.warnings', { count: warnings }),
        duration: 10_000,
        action: {
          label: t('shell.backup.viewDetails'),
          onClick: () => {
            if (runId !== undefined) openRun(runId)
          },
        },
      })
    },
    [t],
  )
  /** A run skipped because browsing is paused or off: say so. */
  const announceSkipped = useCallback(
    (report: BackupReport) => {
      const reason = kindSkipReason(report.reasonCode)
      if (reason) toast(kindSkipSentence(t, { kind: 'browsing', reason }))
    },
    [t],
  )
  /** The translated sentence for a failed run's code, when it has one. */
  const failureCopy = useCallback(
    (error: unknown) =>
      commandErrorCode(error) === 'backup-no-profiles'
        ? t('shell.backup.errors.backup-no-profiles')
        : null,
    [t],
  )
  const announceFailed = useCallback(
    (error: unknown) => {
      toast.error(t('shell.backup.failed'), {
        description: isFullDiskAccessError(error)
          ? t('shell.backup.fullDiskAccess')
          : (failureCopy(error) ?? describeError(error, 'run_backup_now')),
        duration: 10_000,
      })
    },
    [failureCopy, t],
  )

  // Scheduled checks refresh silently. Menu-bar runs also announce their result.
  useEffect(() => {
    const unsubscribe = subscribeToBackupFinished((event) => {
      if (event.source === 'app') return
      if (event.source === 'menu-bar') {
        if (event.report) {
          setLastReport(event.report)
          if (event.report.dueSkipped) announceSkipped(event.report)
          else announceDone(event.report)
        } else {
          announceFailed(event.error)
        }
      }
      void refreshAfterArchiveChange().catch(() => undefined)
    })
    return () => void unsubscribe.then((stop) => stop())
  }, [announceDone, announceFailed, announceSkipped])

  const run = useCallback(
    async ({
      toast: notify = true,
      kindSpecific = false,
      quietSkip = false,
    }: RunOptions = {}) => {
      if (runningRef.current) return null
      runningRef.current = true
      setRunning(true)
      setProgress(null)
      const unsubscribe = await subscribeToBackupProgress(setProgress)
      try {
        const report = await archiveClient.runBackupNow(false, kindSpecific)
        setLastReport(report)
        if (notify && report.dueSkipped) {
          if (!quietSkip) announceSkipped(report)
        } else if (notify) announceDone(report)
        await refreshAfterArchiveChange()
        // A partial run still saved what it read; its warnings say what it left out.
        return { ok: true, report } as const
      } catch (error) {
        if (notify) announceFailed(error)
        await refreshAfterArchiveChange().catch(() => undefined)
        return { ok: false, error } as const
      } finally {
        unsubscribe?.()
        runningRef.current = false
        setRunning(false)
        setProgress(null)
      }
    },
    [announceDone, announceFailed, announceSkipped],
  )

  const value = useMemo(
    () => ({ running, progress, lastReport, run }),
    [running, progress, lastReport, run],
  )
  return (
    <BackupRunnerContext.Provider value={value}>
      {children}
    </BackupRunnerContext.Provider>
  )
}

export function useBackupRunner() {
  const value = useContext(BackupRunnerContext)
  if (!value) {
    throw new Error('useBackupRunner must be used inside BackupRunnerProvider')
  }
  return value
}
