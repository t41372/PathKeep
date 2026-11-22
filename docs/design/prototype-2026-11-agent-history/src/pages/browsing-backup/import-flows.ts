/**
 * The two import flows as the app runs them (`use-takeout-import.ts`,
 * `use-browser-import.ts`): idle → inspecting → preview → importing (with
 * stop) → done, or back to idle with the error. Here with timers and fake
 * results, plus a way for the prototype switcher to jump to any step.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  EDGE_FOLDER,
  edgeNothingNew,
  FIREFOX_PATH,
  firefoxDone,
  firefoxPreview,
  firefoxStopped,
  TAKEOUT_PATH,
  takeoutContinued,
  takeoutDone,
  takeoutFound,
  takeoutMyActivity,
  takeoutNothing,
  takeoutStopped,
  type ImportProgress,
  type Inspection,
} from './import-data'

/** Where the file comes from. "detect" lets the backend read the file's format. */
export type BrowserSource = 'detect' | 'chrome' | 'edge' | 'firefox' | 'safari' | 'atlas' | 'comet'

export const browserSources: BrowserSource[] = ['detect', 'chrome', 'edge', 'firefox', 'safari', 'atlas', 'comet']

export type ImportState =
  | { step: 'idle'; error?: string }
  | { step: 'inspecting'; path: string }
  | { step: 'preview'; path: string; source: BrowserSource; inspection: Inspection }
  | { step: 'importing'; path: string; progress: ImportProgress | null; stopping: boolean }
  | { step: 'done'; path: string; result: Inspection }

/** Visits an import adds that count. */
export const newVisits = (i: Inspection) => Math.max(0, i.candidateItems - i.duplicateItems - syncCopies(i))
/** Firefox Sync copies saved beside the visits they copy, counted once. */
export const syncCopies = (i: Inspection) => i.sameVisitItems ?? 0
/** What the import writes; with none, there is nothing to import. */
export const toSave = (i: Inspection) => newVisits(i) + syncCopies(i)

export type TakeoutPreset = 'idle' | 'error' | 'inspecting' | 'found' | 'nothing' | 'myActivity' | 'importing' | 'stopping' | 'failed' | 'done' | 'continued' | 'stopped'
export type BrowserPreset = 'idle' | 'error' | 'inspecting' | 'preview' | 'nothingNew' | 'importing' | 'stopping' | 'failed' | 'done' | 'stopped'

/** `describeError` passes on the backend's own message. */
const READ_ERROR = 'No such file or directory (os error 2)'
const WRITE_ERROR = 'disk I/O error: No space left on device (os error 28)'

/** A failed import: back to the start with the error under the drop zone, and a toast. */
function failed(setState: (s: ImportState) => void) {
  setState({ step: 'idle', error: WRITE_ERROR })
  toast.error('匯入失敗', { description: WRITE_ERROR })
}

function useImportFlow(kind: 'takeout' | 'browser') {
  const [state, setState] = useState<ImportState>({ step: 'idle' })
  const timers = useRef<number[]>([])
  const clear = () => {
    timers.current.forEach((id) => window.clearTimeout(id))
    timers.current = []
  }
  const later = (ms: number, fn: () => void) => timers.current.push(window.setTimeout(fn, ms))
  useEffect(() => clear, [])

  const finish = useCallback(
    (path: string, result: Inspection) => {
      setState({ step: 'done', path, result })
      if (result.stopped) toast.success('匯入已停止')
      else toast.success(`已匯入 ${result.importedItems.toLocaleString('zh-TW')} 筆新的瀏覽記錄`)
    },
    [],
  )

  const inspect = useCallback(
    (path: string, source: BrowserSource = 'detect') => {
      clear()
      setState({ step: 'inspecting', path })
      const inspection = kind === 'takeout' ? takeoutFound : path === EDGE_FOLDER ? edgeNothingNew : firefoxPreview
      later(900, () => setState({ step: 'preview', path, source, inspection }))
    },
    [kind],
  )

  const confirm = useCallback(() => {
    if (state.step !== 'preview') return
    const { path, inspection } = state
    const total = inspection.candidateItems
    setState({ step: 'importing', path, progress: null, stopping: false })
    // The first event may come before the total is known.
    later(500, () =>
      setState((s) => (s.step === 'importing' ? { ...s, progress: { processedRecords: 1_800, totalRecords: null, progressPercent: null } } : s)),
    )
    for (let i = 1; i <= 10; i += 1)
      later(500 + i * 450, () =>
        setState((s) =>
          s.step === 'importing' && !s.stopping
            ? { ...s, progress: { processedRecords: Math.round((total * i) / 10), totalRecords: total, progressPercent: i * 10 } }
            : s,
        ),
      )
    later(500 + 11 * 450, () => finish(path, kind === 'takeout' ? takeoutDone : firefoxDone))
  }, [state, kind, finish])

  const stop = useCallback(() => {
    if (state.step !== 'importing' || state.stopping) return
    const { path } = state
    clear()
    setState({ ...state, stopping: true })
    later(900, () => finish(path, kind === 'takeout' ? takeoutStopped : firefoxStopped))
  }, [state, kind, finish])

  const reset = useCallback(() => {
    clear()
    setState({ step: 'idle' })
  }, [])

  return { state, setState, inspect, confirm, stop, reset, clear }
}

const midway: ImportProgress = { processedRecords: 9_240, totalRecords: 23_412, progressPercent: 39 }

export function useTakeoutImport() {
  const flow = useImportFlow('takeout')
  const show = (preset: TakeoutPreset) => {
    flow.clear()
    const path = TAKEOUT_PATH
    const preview = (inspection: Inspection) => flow.setState({ step: 'preview', path, source: 'detect', inspection })
    if (preset === 'idle') flow.setState({ step: 'idle' })
    if (preset === 'error') flow.setState({ step: 'idle', error: READ_ERROR })
    if (preset === 'inspecting') flow.setState({ step: 'inspecting', path })
    if (preset === 'found') preview(takeoutFound)
    if (preset === 'nothing') preview(takeoutNothing)
    if (preset === 'myActivity') preview(takeoutMyActivity)
    if (preset === 'importing') flow.setState({ step: 'importing', path, progress: { ...midway, totalRecords: 61_842, processedRecords: 24_100 }, stopping: false })
    if (preset === 'stopping') flow.setState({ step: 'importing', path, progress: { ...midway, totalRecords: 61_842, processedRecords: 24_100 }, stopping: true })
    if (preset === 'failed') failed(flow.setState)
    if (preset === 'done') flow.setState({ step: 'done', path, result: takeoutDone })
    if (preset === 'continued') flow.setState({ step: 'done', path, result: takeoutContinued })
    if (preset === 'stopped') flow.setState({ step: 'done', path, result: takeoutStopped })
  }
  return { ...flow, show }
}

export function useBrowserImport() {
  const flow = useImportFlow('browser')
  const show = (preset: BrowserPreset) => {
    flow.clear()
    const path = FIREFOX_PATH
    if (preset === 'idle') flow.setState({ step: 'idle' })
    if (preset === 'error') flow.setState({ step: 'idle', error: READ_ERROR })
    if (preset === 'inspecting') flow.setState({ step: 'inspecting', path })
    if (preset === 'preview') flow.setState({ step: 'preview', path, source: 'detect', inspection: firefoxPreview })
    if (preset === 'nothingNew') flow.setState({ step: 'preview', path: EDGE_FOLDER, source: 'detect', inspection: edgeNothingNew })
    if (preset === 'importing') flow.setState({ step: 'importing', path, progress: midway, stopping: false })
    if (preset === 'stopping') flow.setState({ step: 'importing', path, progress: midway, stopping: true })
    if (preset === 'failed') failed(flow.setState)
    if (preset === 'done') flow.setState({ step: 'done', path, result: firefoxDone })
    if (preset === 'stopped') flow.setState({ step: 'done', path, result: firefoxStopped })
  }
  return { ...flow, show }
}
