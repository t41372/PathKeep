/**
 * Simulates what src/features/onboarding/use-finish-setup.ts carries out once
 * the user presses 開始第一次備份: create the archive, store the password,
 * install the schedule, run the first backup (one task per chosen type), then
 * set up AI. Same order, the same resume from the stage that failed, and the
 * same skip for every stage but the archive. A prototype switch makes one
 * stage fail the first time it runs.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { Draft } from './draft'
import type { StepKind } from './sources-data'

export type Stage = 'archive' | 'keychain' | 'schedule' | 'backup' | 'ai'

/** Stages the user may skip after a failure. The archive is the only must. */
export const skippable: ReadonlySet<Stage> = new Set(['keychain', 'schedule', 'backup', 'ai'])

export interface TypeProgress {
  id: string
  label: string
  sources: string[]
  /** Sources finished. */
  completed: number
  records: number
  processed: number
}

export interface FinishState {
  status: 'idle' | 'running' | 'failed' | 'done'
  current: Stage | null
  completed: Stage[]
  skipped: Stage[]
  error: { stage: Stage; message: string } | null
  /** Records saved by the first backup; null until it ran. */
  saved: number | null
  progress: TypeProgress[]
}

const idle: FinishState = { status: 'idle', current: null, completed: [], skipped: [], error: null, saved: null, progress: [] }

export function stagesFor(draft: Draft, options: { keychainAvailable: boolean; scheduleSupported: boolean }): Stage[] {
  const stages: Stage[] = ['archive']
  if (draft.encrypt && draft.keychain && options.keychainAvailable) stages.push('keychain')
  if (draft.frequency !== 'off' && options.scheduleSupported) stages.push('schedule')
  stages.push('backup')
  if (draft.ai !== 'off') stages.push('ai')
  return stages
}

/** What each stage's failure says: the app's own messages where it has them, a backend error otherwise. */
const failures: Record<Stage, string> = {
  archive: '存檔已建立，但沒能打開。',
  keychain: 'Platform secure storage failure: User interaction is not allowed.',
  schedule: 'macOS 沒有載入工作。',
  backup: 'Safari 需要「完整磁碟取用權限」才能備份。請在系統設定 → 隱私權與安全性中允許 PathKeep，然後再試一次。',
  ai: 'error sending request for url (http://localhost:11434/v1/models)',
}

/** Records per chosen source, as a person would count them. */
const recordsPerSource: Record<string, number> = { browsing: 151_216, agent: 1_604 }

function typeProgress(list: StepKind[], selection: Record<string, Set<string>>): TypeProgress[] {
  return list
    .map((kind) => {
      const chosen = kind.sources.filter((s) => selection[kind.id]?.has(s.id))
      return {
        id: kind.id,
        label: kind.label,
        sources: chosen.map((s) => (kind.id === 'browsing' ? `${s.name} (${s.detail})` : s.name)),
        completed: 0,
        records: chosen.length * (recordsPerSource[kind.id] ?? 3_200),
        processed: 0,
      }
    })
    .filter((t) => t.sources.length > 0)
}

const wait = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms))

export function useFinishSetup(stages: Stage[], failAt: Stage | null, list: StepKind[], selection: Record<string, Set<string>>) {
  const [state, setState] = useState<FinishState>(idle)
  const busy = useRef(false)
  const failedOnce = useRef(new Set<Stage>())
  const alive = useRef(true)
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
    }
  }, [])

  const runBackup = useCallback(async () => {
    let progress = typeProgress(list, selection)
    setState((s) => ({ ...s, progress }))
    await wait(500)
    // Each type is its own task; they move at their own pace.
    for (let tick = 1; tick <= 24 && alive.current; tick++) {
      progress = progress.map((t, i) => {
        const share = Math.min(1, (tick * (1.4 - i * 0.18)) / 20)
        return { ...t, processed: Math.round(t.records * share), completed: Math.min(t.sources.length, Math.floor(share * t.sources.length)) }
      })
      setState((s) => ({ ...s, progress }))
      await wait(220)
    }
    progress = progress.map((t) => ({ ...t, processed: t.records, completed: t.sources.length }))
    setState((s) => ({ ...s, progress, saved: progress.reduce((sum, t) => sum + t.records, 0) }))
  }, [list, selection])

  const runFrom = useCallback(
    async (completed: Stage[]) => {
      if (busy.current) return
      busy.current = true
      setState((s) => ({ ...s, status: 'running', error: null }))
      const done = [...completed]
      try {
        for (const stage of stages) {
          if (done.includes(stage)) continue
          setState((s) => ({ ...s, current: stage }))
          if (stage === 'backup') await runBackup()
          else await wait(stage === 'archive' ? 900 : 700)
          if (!alive.current) return
          if (stage === failAt && !failedOnce.current.has(stage)) {
            failedOnce.current.add(stage)
            setState((s) => ({ ...s, status: 'failed', progress: stage === 'backup' ? [] : s.progress, saved: stage === 'backup' ? null : s.saved, error: { stage, message: failures[stage] } }))
            return
          }
          done.push(stage)
          setState((s) => ({ ...s, completed: [...done] }))
        }
        setState((s) => ({ ...s, status: 'done', current: null }))
      } finally {
        busy.current = false
      }
    },
    [failAt, runBackup, stages],
  )

  const start = useCallback(() => void runFrom(state.completed), [runFrom, state.completed])

  /** Moves past a failed stage the user can live without. */
  const skip = useCallback(() => {
    const stage = state.error?.stage
    if (!stage || !skippable.has(stage)) return
    setState((s) => ({ ...s, skipped: [...s.skipped, stage] }))
    void runFrom([...state.completed, stage])
  }, [runFrom, state.completed, state.error])

  return { state, start, skip }
}
