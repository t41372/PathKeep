/**
 * The Takeout import flow as a small state machine: inspect (dry run), show
 * what was found, import on confirmation, then refresh everything that
 * depends on the archive. A running import can be stopped; it then reports
 * what it saved. One run for the whole app (`import-runner.tsx`), so leaving
 * Backup keeps it.
 */
import { useCallback, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { refreshAfterArchiveChange } from '@/app/backup-runner'
import { importClient } from '@/lib/backend-client/import'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { subscribeToImportProgress } from '@/lib/ipc/import-progress'
import type { ImportProgressEvent, TakeoutInspection } from '@/lib/types'

export type ImportState =
  | { step: 'idle'; error?: string }
  | { step: 'inspecting'; path: string }
  | { step: 'preview'; path: string; inspection: TakeoutInspection }
  | {
      step: 'importing'
      path: string
      /** Names the run for `stopImport`. */
      importId: string
      progress: ImportProgressEvent | null
      stopping: boolean
    }
  | { step: 'done'; path: string; result: TakeoutInspection }

export function useTakeoutImportRun() {
  const { t } = useI18n()
  const [state, setState] = useState<ImportState>({ step: 'idle' })
  const busy = useRef(false)

  const inspect = useCallback(async (rawPath: string) => {
    const path = rawPath.trim()
    if (!path || busy.current) return
    busy.current = true
    setState({ step: 'inspecting', path })
    try {
      const inspection = await importClient.inspectTakeout({
        sourcePath: path,
        dryRun: true,
      })
      setState({ step: 'preview', path, inspection })
    } catch (error) {
      setState({ step: 'idle', error: describeError(error, 'inspect_takeout') })
    } finally {
      busy.current = false
    }
  }, [])

  const confirm = useCallback(async () => {
    if (state.step !== 'preview' || busy.current) return
    const { path } = state
    const importId = crypto.randomUUID()
    busy.current = true
    setState({
      step: 'importing',
      path,
      importId,
      progress: null,
      stopping: false,
    })
    const unsubscribe = await subscribeToImportProgress((progress) =>
      setState((current) =>
        current.step === 'importing' ? { ...current, progress } : current,
      ),
    )
    try {
      const result = await importClient.importTakeout(
        { sourcePath: path, dryRun: false },
        importId,
      )
      setState({ step: 'done', path, result })
      if (result.stopped) toast.success(t('backup.import.stoppedToast'))
      else
        toast.success(t('backup.import.done', { count: result.importedItems }))
    } catch (error) {
      const description = describeError(error, 'import_takeout')
      setState({ step: 'idle', error: description })
      toast.error(t('backup.import.failed'), { description })
    } finally {
      unsubscribe?.()
      busy.current = false
    }
    // After the run, not part of it: the next export can be inspected while
    // the pages read the archive again, and a refresh that fails does not undo
    // an import that finished. A failed import may have saved part of it.
    await refreshAfterArchiveChange().catch(() => undefined)
  }, [state, t])

  /**
   * Asks the import to stop after the batch it is writing. It then finishes
   * the way `confirm` waits for, with `stopped` set, or as done if it was
   * already past its last batch.
   */
  const stop = useCallback(async () => {
    if (state.step !== 'importing' || state.stopping) return
    const { importId } = state
    const marked = (stopping: boolean) =>
      setState((current) =>
        current.step === 'importing' && current.importId === importId
          ? { ...current, stopping }
          : current,
      )
    marked(true)
    try {
      await importClient.stopImport(importId)
    } catch (error) {
      marked(false)
      toast.error(t('backup.import.stopFailed'), {
        description: describeError(error, 'stop_import'),
      })
    }
  }, [state, t])

  const reset = useCallback(() => setState({ step: 'idle' }), [])

  return useMemo(
    () => ({ state, inspect, confirm, stop, reset }),
    [state, inspect, confirm, stop, reset],
  )
}
