/**
 * The Takeout import flow as a small state machine: inspect (dry run), show
 * what was found, import on confirmation, then refresh everything that
 * depends on the archive.
 */
import { useCallback, useRef, useState } from 'react'
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
  | { step: 'importing'; path: string; progress: ImportProgressEvent | null }
  | { step: 'done'; path: string; result: TakeoutInspection }

export function useTakeoutImport() {
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
    busy.current = true
    setState({ step: 'importing', path, progress: null })
    const unsubscribe = await subscribeToImportProgress((progress) =>
      setState((current) =>
        current.step === 'importing' ? { ...current, progress } : current,
      ),
    )
    try {
      const result = await importClient.importTakeout({
        sourcePath: path,
        dryRun: false,
      })
      setState({ step: 'done', path, result })
      toast.success(t('backup.import.done', { count: result.importedItems }))
      await refreshAfterArchiveChange()
    } catch (error) {
      const description = describeError(error, 'import_takeout')
      setState({ step: 'idle', error: description })
      toast.error(t('backup.import.failed'), { description })
      await refreshAfterArchiveChange().catch(() => undefined)
    } finally {
      unsubscribe?.()
      busy.current = false
    }
  }, [state, t])

  const reset = useCallback(() => setState({ step: 'idle' }), [])

  return { state, inspect, confirm, reset }
}
