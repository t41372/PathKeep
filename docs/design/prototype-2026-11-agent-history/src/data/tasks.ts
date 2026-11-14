/**
 * Background work, simulated: a backup that advances, an index build, a
 * queue behind them, and today's finished and failed runs. The real app
 * reads these from the job runner over IPC events.
 */
import { useSyncExternalStore } from 'react'
import { NOW } from './fixture'
import type { KindId } from '~/kinds'

export type TaskState = 'running' | 'queued' | 'paused' | 'done' | 'failed'

export interface Task {
  id: string
  /** Which data type it serves; `app` for archive-wide work (rekey, compaction). */
  space: KindId | 'app'
  title: string
  detail: string
  /** 0–1, or null while the total isn't known yet. */
  progress: number | null
  state: TaskState
  at: number
  /** Why it waits or failed. */
  note?: string
}

const initial = (): Task[] => [
  { id: 't1', space: 'agent', title: '備份 Agent 對話', detail: 'Claude Code · 412 / 1,160 段', progress: 0.36, state: 'running', at: NOW - 4 * 60_000 },
  { id: 't2', space: 'agent', title: '更新搜尋索引', detail: '新進的對話', progress: null, state: 'running', at: NOW - 60_000 },
  { id: 't3', space: 'agent', title: '重算洞察', detail: '等備份完成', progress: 0, state: 'queued', at: NOW },
  { id: 't4', space: 'browsing', title: '語意索引', detail: '2,140 頁', progress: 0.62, state: 'paused', at: NOW - 3600_000, note: '你暫停了' },
  { id: 't5', space: 'browsing', title: '備份瀏覽紀錄', detail: '5 個瀏覽器 · 新增 1,204 筆', progress: 1, state: 'done', at: NOW - 100 * 60_000 },
  { id: 't6', space: 'agent', title: '備份 Agent 對話', detail: 'Codex · 有 1 個檔案讀不到', progress: 1, state: 'failed', at: NOW - 160 * 60_000, note: '檔案在備份時正被寫入，下次會再試' },
]

let tasks = initial()
let enabled = true
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((l) => l())

let timer: ReturnType<typeof setInterval> | null = null
function tick() {
  tasks = tasks.map((t) => {
    if (t.state !== 'running') return t
    if (t.id === 't1') {
      const p = Math.min(1, (t.progress ?? 0) + 0.004)
      const done = Math.round(p * 1160)
      return p >= 1 ? { ...t, progress: 0.02, detail: 'Claude Code · 23 / 1,160 段' } : { ...t, progress: p, detail: `Claude Code · ${done.toLocaleString('zh-TW')} / 1,160 段` }
    }
    if (t.id === 't2') return { ...t, progress: t.progress === null ? (Math.random() < 0.05 ? 0.05 : null) : Math.min(0.98, t.progress + 0.01) }
    return t
  })
  emit()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  if (!timer) timer = setInterval(tick, 600)
  return () => {
    listeners.delete(listener)
    if (listeners.size === 0 && timer) {
      clearInterval(timer)
      timer = null
    }
  }
}

/** Prototype switch: pretend nothing is running. */
export function setTasksRunning(on: boolean) {
  enabled = on
  emit()
}

const idle: Task[] = initial().filter((t) => t.state === 'done' || t.state === 'failed' || t.state === 'paused')

export function useTasks(): Task[] {
  return useSyncExternalStore(subscribe, () => (enabled ? tasks : idle))
}

export function pauseTask(id: string) {
  tasks = tasks.map((t) => (t.id === id ? { ...t, state: t.state === 'paused' ? 'running' : 'paused', note: t.state === 'paused' ? undefined : '你暫停了' } : t))
  emit()
}
