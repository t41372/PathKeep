/**
 * Browser Direct import as a small state machine: inspect a history file or
 * profile folder (a read-only preview that counts new and already-archived
 * visits), import on confirmation, then refresh everything that depends on
 * the archive. A running import can be stopped. Mirrors
 * `use-takeout-import.ts`, and like it runs once for the whole app
 * (`import-runner.tsx`).
 */
import { useCallback, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { refreshAfterArchiveChange } from '@/app/backup-runner'
import { importClient } from '@/lib/backend-client/import'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { subscribeToImportProgress } from '@/lib/ipc/import-progress'
import type {
  BrowserHistoryImportRequest,
  ImportProgressEvent,
  TakeoutInspection,
} from '@/lib/types'

/** Where the file comes from. "detect" lets the backend read the file's format. */
export type BrowserSource =
  | 'detect'
  | 'chrome'
  | 'edge'
  | 'firefox'
  | 'safari'
  | 'atlas'
  | 'comet'

export const browserSources: BrowserSource[] = [
  'detect',
  'chrome',
  'edge',
  'firefox',
  'safari',
  'atlas',
  'comet',
]

/**
 * Edge, Atlas and Comet use Chrome's format; naming them keeps the product
 * on the imported profile instead of a generic "Google Chrome".
 */
const sourceRequest: Record<
  Exclude<BrowserSource, 'detect'>,
  Pick<BrowserHistoryImportRequest, 'browserFamily' | 'browserName'>
> = {
  chrome: { browserFamily: 'chromium', browserName: 'Google Chrome' },
  edge: { browserFamily: 'chromium', browserName: 'Microsoft Edge' },
  firefox: { browserFamily: 'firefox', browserName: 'Firefox' },
  safari: { browserFamily: 'safari', browserName: 'Safari' },
  atlas: { browserFamily: 'chromium', browserName: 'ChatGPT Atlas' },
  comet: { browserFamily: 'chromium', browserName: 'Perplexity Comet' },
}

function request(
  sourcePath: string,
  source: BrowserSource,
  dryRun: boolean,
): BrowserHistoryImportRequest {
  return {
    sourcePath,
    dryRun,
    ...(source === 'detect' ? {} : sourceRequest[source]),
  }
}

/** Visits an import of this preview would add. */
export function newVisits(inspection: TakeoutInspection) {
  return Math.max(0, inspection.candidateItems - inspection.duplicateItems)
}

export type BrowserImportState =
  | { step: 'idle'; error?: string }
  | { step: 'inspecting'; path: string }
  | {
      step: 'preview'
      path: string
      source: BrowserSource
      inspection: TakeoutInspection
    }
  | {
      step: 'importing'
      path: string
      /** Names the run for `stopImport`. */
      importId: string
      progress: ImportProgressEvent | null
      stopping: boolean
    }
  | { step: 'done'; path: string; result: TakeoutInspection }

export function useBrowserImportRun() {
  const { t } = useI18n()
  const [state, setState] = useState<BrowserImportState>({ step: 'idle' })
  const busy = useRef(false)

  const inspect = useCallback(
    async (rawPath: string, source: BrowserSource) => {
      const path = rawPath.trim()
      if (!path || busy.current) return
      busy.current = true
      setState({ step: 'inspecting', path })
      try {
        const inspection = await importClient.inspectBrowserHistory(
          request(path, source, true),
        )
        setState({ step: 'preview', path, source, inspection })
      } catch (error) {
        setState({
          step: 'idle',
          error: describeError(error, 'inspect_browser_history'),
        })
      } finally {
        busy.current = false
      }
    },
    [],
  )

  const confirm = useCallback(async () => {
    if (state.step !== 'preview' || busy.current) return
    const { path, source } = state
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
      const result = await importClient.importBrowserHistory(
        request(path, source, false),
        importId,
      )
      setState({ step: 'done', path, result })
      if (result.stopped) toast.success(t('backup.import.stoppedToast'))
      else
        toast.success(
          t('backupImport.browser.done', { count: result.importedItems }),
        )
    } catch (error) {
      const description = describeError(error, 'import_browser_history')
      setState({ step: 'idle', error: description })
      toast.error(t('backupImport.browser.failed'), { description })
    } finally {
      unsubscribe?.()
      busy.current = false
    }
    // After the run, not part of it: the next file can be inspected while the
    // pages read the archive again, and a refresh that fails does not undo an
    // import that finished. A failed import may have saved part of the file.
    await refreshAfterArchiveChange().catch(() => undefined)
  }, [state, t])

  /** Asks the import to stop after the batch it is writing, as for Takeout. */
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
