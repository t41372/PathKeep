/**
 * Runs a manual backup and reports progress. Lives above the router so Home,
 * Backup and the command palette share one run and one progress state.
 */
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { toast } from 'sonner'
import { appClient } from '@/lib/backend-client/app'
import { archiveClient } from '@/lib/backend-client/archive'
import { describeError } from '@/lib/errors'
import { isFullDiskAccessError } from '@/lib/ipc/command-error'
import { subscribeToBackupProgress } from '@/lib/ipc/backup-progress'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import type { BackupProgressEvent, BackupReport } from '@/lib/types'

interface BackupRunnerValue {
  running: boolean
  progress: BackupProgressEvent | null
  lastReport: BackupReport | null
  run: () => Promise<void>
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

  const run = useCallback(async () => {
    if (runningRef.current) return
    runningRef.current = true
    setRunning(true)
    setProgress(null)
    const unsubscribe = await subscribeToBackupProgress(setProgress)
    try {
      const report = await archiveClient.runBackupNow(false)
      setLastReport(report)
      const added = report.run?.newVisits ?? 0
      toast.success(
        added > 0
          ? t('shell.backup.done', { count: added })
          : t('shell.backup.doneNothingNew'),
      )
      await refreshAfterArchiveChange()
    } catch (error) {
      toast.error(t('shell.backup.failed'), {
        description: isFullDiskAccessError(error)
          ? t('shell.backup.fullDiskAccess')
          : describeError(error, 'run_backup_now'),
        duration: 10_000,
      })
      await refreshAfterArchiveChange().catch(() => undefined)
    } finally {
      unsubscribe?.()
      runningRef.current = false
      setRunning(false)
      setProgress(null)
    }
  }, [t])

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
