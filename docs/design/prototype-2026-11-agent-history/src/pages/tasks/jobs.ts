/**
 * Fake job history of the two background queues, already in the one shape
 * the job list reads (`JobView`, `src/features/settings/background/job-view.ts`):
 * the Insights queue (`list_intelligence_jobs`) and the search-index /
 * assistant queue (`list_ai_jobs`). Names, the actions each job allows and
 * the paused notes follow job-view's rules; the actions change the store the
 * way the backend would.
 */
import { useSyncExternalStore } from 'react'
import { toast } from 'sonner'
import { NOW } from '~/data/fixture'

export type JobState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'paused' | 'stale' | 'unknown'

export interface JobView {
  key: string
  id: number
  name: string
  state: JobState
  rawState: string
  at: number | null
  progress: { percent: number | null; current?: number; total?: number } | null
  error: string | null
  note: { text: string; problem: boolean } | null
  canRetry: boolean
  resumes: boolean
  canCancel: boolean
  canPause: boolean
}

const M = 60_000
const H = 3600_000

const jobNames: Record<string, string> = {
  'visit-derive': '網頁細節',
  'daily-rollup': '每日統計',
  'structural-rebuild': '瀏覽段落與搜尋',
  'full-rebuild': '全部',
  'enrichment-plugin': '整理網頁標題',
  'content-fetch': '網頁摘要',
  'index-build': '建立搜尋索引',
  'index-clear': '清除搜尋索引',
  assistant: '助理回答',
}
const typeName = (type: string) => jobNames[type] ?? '背景工作'

/** `fromRuntime`: page jobs carry the page title; the rest the browser profile (or 所有瀏覽器). */
function runtimeJob(id: number, type: string, state: JobState, at: number, extra: { profile?: string; title?: string; progress?: [number, number]; error?: string; raw?: string } = {}): JobView {
  const page = type === 'enrichment-plugin' || type === 'content-fetch'
  const name = page ? `${typeName(type)}: ${extra.title ?? ''}` : `${extra.profile ?? '所有瀏覽器'} · ${typeName(type)}`
  return {
    key: `runtime-${id}`,
    id,
    name,
    state,
    rawState: extra.raw ?? state,
    at,
    progress: state === 'running' ? (extra.progress ? { percent: (extra.progress[0] / extra.progress[1]) * 100, current: extra.progress[0], total: extra.progress[1] } : { percent: null }) : null,
    error: state === 'failed' ? (extra.error ?? null) : null,
    note: null,
    canRetry: ['failed', 'cancelled', 'stale'].includes(state),
    resumes: false,
    canCancel: ['queued', 'running'].includes(state),
    canPause: false,
  }
}

type PausedBy = 'user' | 'queue' | 'service'
const pausedNotes: Record<PausedBy, (message: string) => string> = {
  queue: () => '隨所有背景工作一起暫停。',
  user: () => '你暫停了它。',
  service: (message) => `服務沒能執行模型，已暫停：${message}`,
}

/** `fromAiQueue`, with the `replay_ai_job` / `cancel_ai_job` / `pause_ai_job` state rules. */
function aiJob(id: number, type: string, state: JobState, at: number, extra: { embedded?: [number, number]; pausedBy?: PausedBy; message?: string; retired?: boolean; error?: string } = {}): JobView {
  const build = type === 'index-build'
  const pausedBy = state === 'paused' ? (extra.pausedBy ?? 'user') : null
  const target = extra.embedded?.[1] ?? 0
  return {
    key: `ai-${id}`,
    id,
    name: typeName(type),
    state,
    rawState: state,
    at,
    progress:
      state === 'running' || (build && state === 'paused')
        ? target > 0
          ? { percent: (extra.embedded![0] / target) * 100, current: extra.embedded![0], total: target }
          : { percent: null }
        : null,
    error: extra.retired ? 'Ask 已經不在背景回答問題，所以這個問題沒有被回答。請在 Ask 裡再問一次。' : state === 'failed' ? (extra.error ?? null) : null,
    note: pausedBy ? { text: pausedNotes[pausedBy](extra.message ?? ''), problem: pausedBy === 'service' } : null,
    canRetry: type !== 'assistant' && pausedBy !== 'queue' && ['failed', 'cancelled', 'stale', 'paused'].includes(state),
    resumes: build && state === 'paused',
    canCancel: ['queued', 'paused', 'stale', 'running'].includes(state),
    canPause: build && ['queued', 'stale', 'running'].includes(state),
  }
}

function insightsHistory(): JobView[] {
  const jobs: JobView[] = [
    runtimeJob(240, 'enrichment-plugin', 'queued', NOW - 2 * M, { title: 'Announcing Rust 1.92 | Rust Blog' }),
    runtimeJob(239, 'content-fetch', 'failed', NOW - 26 * M, { title: 'SQLite FTS5 Extension', error: 'HTTP 429 Too Many Requests' }),
    runtimeJob(238, 'daily-rollup', 'succeeded', NOW - 78 * M, { profile: 'Chrome 個人' }),
    runtimeJob(237, 'visit-derive', 'succeeded', NOW - 79 * M, { profile: 'Chrome 個人' }),
    runtimeJob(236, 'structural-rebuild', 'stale', NOW - 3 * H, { profile: 'Safari 預設' }),
    runtimeJob(235, 'content-fetch', 'cancelled', NOW - 5 * H, { title: 'Taipei Metro route map' }),
    runtimeJob(234, 'visit-derive', 'succeeded', NOW - 7 * H, { profile: 'Chrome 工作' }),
    runtimeJob(233, 'garbage-collect', 'succeeded', NOW - 9 * H),
  ]
  for (let i = 0; i < 30; i++) {
    const types = ['visit-derive', 'daily-rollup', 'structural-rebuild', 'content-fetch']
    const type = types[i % types.length]
    jobs.push(runtimeJob(232 - i, type, 'succeeded', NOW - (10 + i * 3) * H, { profile: ['Chrome 個人', 'Chrome 工作', 'Safari 預設', 'Arc 預設'][i % 4], title: `網頁 ${232 - i}` }))
  }
  return jobs
}

function aiHistory(paused: boolean): JobView[] {
  return [
    paused
      ? aiJob(91, 'index-build', 'paused', NOW - 20 * M, { embedded: [41_206, 96_118], pausedBy: 'queue' })
      : aiJob(91, 'index-build', 'running', NOW - 20 * M, { embedded: [41_206, 96_118] }),
    aiJob(90, 'index-build', 'paused', NOW - 2 * H, { embedded: [1_204, 8_400], pausedBy: 'service', message: 'LM Studio returned 503: model is loading' }),
    aiJob(89, 'index-build', 'paused', NOW - 4 * H, { embedded: [300, 2_140], pausedBy: 'user' }),
    aiJob(88, 'index-clear', 'succeeded', NOW - 26 * H),
    aiJob(87, 'assistant', 'failed', NOW - 3 * 24 * H, { retired: true }),
    aiJob(86, 'index-build', 'failed', NOW - 4 * 24 * H, { error: 'embedding request timed out after 120 s' }),
    aiJob(85, 'index-build', 'stale', NOW - 5 * 24 * H, { embedded: [0, 0] }),
    aiJob(84, 'index-build', 'queued', NOW - 5 * 24 * H),
  ]
}

type Queue = 'insights' | 'ai'
let lists: Record<Queue, JobView[]> = { insights: insightsHistory(), ai: aiHistory(false) }
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((l) => l())

export function useJobs(queue: Queue) {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => lists[queue],
  )
}

/** All background work paused or resumed: the index build follows (`pausedBy: queue`). */
export function followQueuePause(paused: boolean) {
  lists = { ...lists, ai: lists.ai.map((j) => (j.id === 91 && (j.state === 'running' || j.state === 'paused') ? aiHistory(paused)[0] : j)) }
  emit()
}

/** Prototype: an Insights rebuild is running, or not. */
export function setRebuildRunning(on: boolean) {
  const rest = lists.insights.filter((j) => j.id !== 300)
  lists = { ...lists, insights: on ? [runtimeJob(300, 'full-rebuild', 'running', NOW - 3 * M, { progress: [182_340, 612_118] }), ...rest] : rest }
  emit()
}

function update(queue: Queue, id: number, next: (j: JobView) => JobView) {
  lists = { ...lists, [queue]: lists[queue].map((j) => (j.id === id ? next(j) : j)) }
  emit()
}

export function retryJob(queue: Queue, job: JobView) {
  update(queue, job.id, (j) => (queue === 'ai' ? aiJob(j.id, j.name === '建立搜尋索引' ? 'index-build' : 'index-clear', 'queued', NOW) : { ...j, state: 'queued', rawState: 'queued', at: NOW, error: null, canRetry: false, canCancel: true }))
  toast.success('已重新排隊。')
}

export function cancelJob(queue: Queue, job: JobView) {
  update(queue, job.id, (j) => ({ ...j, state: 'cancelled', rawState: 'cancelled', at: NOW, progress: null, note: null, canCancel: false, canPause: false, canRetry: j.name !== '助理回答', resumes: false }))
  toast.success(job.state === 'running' ? '正在停止，到達安全的停止點就會結束。' : '已取消。')
}

export function pauseJob(job: JobView) {
  update('ai', job.id, (j) => aiJob(j.id, 'index-build', 'paused', NOW, { embedded: j.progress?.total ? [j.progress.current ?? 0, j.progress.total] : undefined, pausedBy: 'user' }))
  toast.success('正在暫停，正在讀取的這批造訪存好後就會停下。')
}

/** `useQueueRebuild`: a full rebuild joins the Insights queue. */
export function queueRebuild() {
  lists = { ...lists, insights: [runtimeJob(400 + lists.insights.length, 'full-rebuild', 'queued', NOW), ...lists.insights] }
  emit()
  toast.success('重建已加入佇列。')
}

/** "2 running · 3 waiting · 1 failed", or "Nothing running." */
export function countSummary(counts: { running: number; queued: number; failed: number }) {
  const parts = [counts.running > 0 && `${counts.running} 項執行中`, counts.queued > 0 && `${counts.queued} 項等待中`, counts.failed > 0 && `${counts.failed} 項失敗`].filter(Boolean)
  return parts.length > 0 ? parts.join(' · ') : '沒有正在執行的工作。'
}

export function countsOf(jobs: JobView[]) {
  return {
    running: jobs.filter((j) => j.state === 'running').length,
    queued: jobs.filter((j) => j.state === 'queued' || j.state === 'paused').length,
    failed: jobs.filter((j) => j.state === 'failed').length,
  }
}
