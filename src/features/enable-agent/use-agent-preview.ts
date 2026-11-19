/**
 * One run of the Agent tools preview, as a state a dialog draws (the enable
 * dialog's, and the backup page's rescan and add-a-folder): walking (with the running count and each folder as it is
 * counted), finished (with the report, and whether it has gone stale),
 * stopped by the person, or failed.
 *
 * Responsibilities: starting the walk while the dialog is open, ending it
 * when the dialog closes, unmounts (the app locked) or the person cancels,
 * and starting a fresh one on request. The walk runs in the backend; this
 * only applies its frames, about ten a second at most.
 * Not responsible for: drawing (`preview-step.tsx`) or the counts' meaning
 * (`preview-model.ts`).
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import {
  dataKindsClient,
  type AgentPreviewRequest,
  type PreviewFrame,
  type PreviewReport,
  type RootInventory,
} from '@/lib/backend-client/data-kinds'
import type { AgentPreviewStream } from '@/lib/ipc/agent-preview-stream'
import { commandErrorCode } from '@/lib/ipc/command-error'

export type PreviewState =
  | {
      phase: 'walking'
      /** Files and folders looked at so far; `null` before the first count. */
      checked: number | null
      /** Folders counted so far, as they finish. */
      roots: RootInventory[]
    }
  | { phase: 'finished'; report: PreviewReport; expired: boolean }
  | { phase: 'cancelled' }
  | { phase: 'failed'; code: string | null; error: unknown }

const WALKING: PreviewState = { phase: 'walking', checked: null, roots: [] }

function applyFrame(state: PreviewState, frame: PreviewFrame): PreviewState {
  switch (frame.type) {
    case 'progress':
      return state.phase === 'walking'
        ? { ...state, checked: Number(frame.payload.done) || 0 }
        : state
    case 'root':
      return state.phase === 'walking'
        ? { ...state, roots: [...state.roots, frame.payload] }
        : state
    case 'finished':
      return {
        phase: 'finished',
        report: frame.payload,
        expired: frame.payload.expiresAt <= Date.now(),
      }
    case 'cancelled':
      return { phase: 'cancelled' }
    case 'error':
      return { phase: 'failed', code: frame.payload.code, error: null }
  }
}

const ENABLE: AgentPreviewRequest = { purpose: 'enable' }

/** `request` is read when a walk starts; a new walk starts when its contents change. */
export function useAgentPreview(
  active: boolean,
  request: AgentPreviewRequest = ENABLE,
) {
  const [run, setRun] = useState(0)
  const requestKey = JSON.stringify(request)
  const [state, setState] = useState<PreviewState>(WALKING)
  const stream = useRef<AgentPreviewStream | null>(null)
  const stopped = useRef(false)

  useEffect(() => {
    if (!active) return
    let live = true
    stopped.current = false
    const onFrame = (frame: PreviewFrame) => {
      if (live && !stopped.current) {
        setState((current) => applyFrame(current, frame))
      }
    }
    // Started a moment later, so a mount that is undone at once (React's
    // development double mount, a dialog closed as it opens) walks nothing.
    const start = setTimeout(() => {
      if (!live || stopped.current) return
      const asked = JSON.parse(requestKey) as AgentPreviewRequest
      dataKindsClient.previewAgentSources(asked, onFrame).then(
        (started) => {
          // Closed or cancelled before the backend answered: stop it at once.
          if (!live || stopped.current) started.close()
          else stream.current = started
        },
        (error: unknown) => {
          if (live && !stopped.current) {
            setState({ phase: 'failed', code: commandErrorCode(error), error })
          }
        },
      )
    }, 0)
    return () => {
      live = false
      clearTimeout(start)
      stream.current?.close()
      stream.current = null
    }
  }, [active, run, requestKey])

  // A finished preview's ticket lapses (five minutes); after that the
  // folders have to be looked at again before anything is saved.
  const expiresAt = state.phase === 'finished' ? state.report.expiresAt : null
  useEffect(() => {
    if (expiresAt === null) return
    const timer = setTimeout(
      () =>
        setState((current) =>
          current.phase === 'finished' && !current.expired
            ? { ...current, expired: true }
            : current,
        ),
      Math.max(0, expiresAt - Date.now()),
    )
    return () => clearTimeout(timer)
  }, [expiresAt])

  /** Stops the walk; what was counted so far is let go. */
  const cancel = useCallback(() => {
    stopped.current = true
    stream.current?.close()
    stream.current = null
    setState({ phase: 'cancelled' })
  }, [])

  /** A fresh walk, from nothing. */
  const rerun = useCallback(() => {
    stream.current?.close()
    stream.current = null
    setState(WALKING)
    setRun((count) => count + 1)
  }, [])

  return { state, cancel, rerun }
}
