/**
 * Browser Direct import as a small state machine: inspect a history file or
 * profile folder (a read-only preview that counts new and already-archived
 * visits), import on confirmation, then refresh everything that depends on
 * the archive. Mirrors `use-takeout-import.ts`.
 */
import { useCallback, useRef, useState } from 'react'
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
  | { step: 'importing'; path: string; progress: ImportProgressEvent | null }
  | { step: 'done'; path: string; result: TakeoutInspection }

export function useBrowserImport() {
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
    busy.current = true
    setState({ step: 'importing', path, progress: null })
    const unsubscribe = await subscribeToImportProgress((progress) =>
      setState((current) =>
        current.step === 'importing' ? { ...current, progress } : current,
      ),
    )
    try {
      const result = await importClient.importBrowserHistory(
        request(path, source, false),
      )
      setState({ step: 'done', path, result })
      toast.success(
        t('backupImport.browser.done', { count: result.importedItems }),
      )
      await refreshAfterArchiveChange()
    } catch (error) {
      const description = describeError(error, 'import_browser_history')
      setState({ step: 'idle', error: description })
      toast.error(t('backupImport.browser.failed'), { description })
      await refreshAfterArchiveChange().catch(() => undefined)
    } finally {
      unsubscribe?.()
      busy.current = false
    }
  }, [state, t])

  const reset = useCallback(() => setState({ step: 'idle' }), [])

  return { state, inspect, confirm, reset }
}
