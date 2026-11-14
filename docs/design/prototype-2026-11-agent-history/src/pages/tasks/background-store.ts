/**
 * The background-work settings and states both pages read: the settings page
 * (run / pause, jobs at once, link-preview choices) writes them, the 背景任務
 * page shows what they mean for the queues. Stands in for
 * `config.ai.jobQueuePaused` / `jobQueueConcurrency`, `config.ogImage`, and
 * the feature switches in today's General (in this prototype in Settings ›
 * 瀏覽紀錄). The `proto*` fields are only the reviewer's switches.
 */
import { useSyncExternalStore } from 'react'

export type KeepChoice = 'off' | 'days30' | 'days90' | 'size100' | 'size500' | 'custom'
export type PreviewMode = 'on_demand' | 'background'

export interface BackgroundState {
  paused: boolean
  concurrency: number
  /** `ai.enabled && (semanticIndexEnabled || assistantEnabled)`. */
  aiOn: boolean
  summariesOn: boolean
  previewsOn: boolean
  previewMode: PreviewMode
  keep: KeepChoice
  /** Prototype: an Insights rebuild is running, so clearing is refused. */
  protoRebuilding: boolean
  /** Prototype: the queues are still on their first read. */
  protoLoading: boolean
  /** Prototype: the queues have no jobs at all. */
  protoEmpty: boolean
}

let state: BackgroundState = {
  paused: false,
  concurrency: 2,
  aiOn: true,
  summariesOn: true,
  previewsOn: true,
  previewMode: 'background',
  keep: 'days90',
  protoRebuilding: false,
  protoLoading: false,
  protoEmpty: false,
}
const listeners = new Set<() => void>()

export function setBackground(next: Partial<BackgroundState>) {
  state = { ...state, ...next }
  listeners.forEach((l) => l())
}

export function useBackground() {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => state,
  )
}

export const keepLabels: Record<KeepChoice, string> = {
  custom: '自訂',
  off: '直到清除',
  days30: '30 天',
  days90: '90 天',
  size100: '最多 100 MB',
  size500: '最多 500 MB',
}
export const modeLabels: Record<PreviewMode, string> = { on_demand: '顯示時', background: '每次備份後' }
