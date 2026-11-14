/**
 * 最近記錄: recent runs, each opening its audit detail in a sheet. The open
 * run lives in the URL (`?run=`) so other screens and a finished backup's
 * toast can link to it. Mirrors `src/features/backup/runs-card.tsx`, with the
 * design change that only runs that did something for browsing history are
 * listed, each with its browsing part (`runs-data.ts`).
 */
import { useSearchParams } from 'react-router-dom'
import { dayAndTime, number, runDuration } from './format'
import { RunIcon } from './run-icon'
import { RunSheet } from './run-sheet'
import { allRuns, finishedRun, runningRun, type Run } from './runs-data'
import { useScene } from './scene'

const typeLabel: Record<string, string> = {
  import: '匯入',
  rekey: '加密變更',
  doctor: '健康檢查',
  archive_restore: '存檔還原',
  snapshot_restore: '快照還原',
  retention_prune: '清理',
  rollback: '復原匯入',
  restore: '還原匯入',
  'same-visit': '同一次造訪只算一次',
}

/** The kinds whose `newVisits` counts visits added; an undo's counts the ones it hid. */
const addsVisits = ['import', 'snapshot_restore']

function summary(run: Run) {
  if (run.status === 'failed') return run.errorMessage || '備份失敗'
  const type = run.runType in typeLabel ? run.runType : 'backup'
  const stopped = run.status === 'interrupted'
  const visits = `新增 ${number(run.newVisits)} 筆`
  if (type === 'backup') {
    const text = `${run.profilesProcessed} 個來源 · ${visits}`
    if (run.status === 'partial') return `備份未完成 · ${text}`
    return stopped ? `中途停止 · ${text} · 下次備份會接著完成` : text
  }
  const label = typeLabel[type]
  if (stopped) return `${label} · 中途停止`
  return addsVisits.includes(type) && run.newVisits > 0 ? `${label} · ${visits}` : label
}

export function RunsCard() {
  const scene = useScene()
  const latest = scene.running ? [runningRun] : scene.finished ? [finishedRun] : []
  const browsing = [...latest, ...allRuns.filter((run) => run.space === 'browsing')]
  const runs = scene.runsEmpty ? [] : browsing
  const [params, setParams] = useSearchParams()
  const open = Number(params.get('run')) || null
  const setOpen = (runId: number | null) => setParams(runId === null ? {} : { run: String(runId) }, { replace: runId === null })

  return (
    <section className="overflow-hidden rounded-xl border bg-card shadow-card">
      <h2 className="px-5 py-3.5 text-sm font-semibold">最近記錄</h2>
      {runs.length === 0 ? (
        <p className="border-t px-5 py-4 text-[13px] text-muted-foreground">還沒有備份。執行一次，記錄就會出現在這裡。</p>
      ) : (
        <ul className="border-t">
          {runs.map((run) => (
            <li key={run.id} className="border-t first:border-t-0">
              <button
                type="button"
                onClick={() => setOpen(run.id)}
                className="flex w-full items-center gap-3.5 px-5 py-2.5 text-left text-[13px] transition-colors hover:bg-accent/60"
              >
                <RunIcon status={run.status} />
                <span className="w-32 shrink-0 font-mono text-xs text-muted-foreground tabular">{dayAndTime(run.startedAt)}</span>
                <span className="min-w-0 flex-1 truncate">{summary(run)}</span>
                <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">{runDuration(run) ?? '—'}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <RunSheet run={browsing.find((run) => run.id === open) ?? null} onClose={() => setOpen(null)} />
    </section>
  )
}
