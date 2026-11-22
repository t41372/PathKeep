/**
 * Progress and a Stop button for a long whole-archive operation (export,
 * import, encryption change, restore). Mirrors
 * `src/components/app/maintenance-progress.tsx`; instead of polling the
 * backend it walks through the phases the backend reports for that
 * operation, with invented counts, then calls `onDone`. Stop shows
 * 正在停止… for a moment, then `onStopped`: the operation ends with nothing changed.
 */
import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { count } from './format'

const phaseText = {
  exporting: '正在準備檔案',
  importing: '正在開啟檔案',
  converting: '正在複製存檔',
  rewrapping: '正在更換密碼',
  restoring: '正在還原',
  pruning: '正在刪除舊快照',
  copying: '正在複製存檔',
  snapshotting: '正在建立快照',
  verifying: '正在檢查副本',
  indexing: '正在建立搜尋索引',
  projecting: '正在連結頁面',
  publishing: '即將完成',
  merging: '正在合併重複項目',
} as const

export type Phase = keyof typeof phaseText

/** One phase as the backend reports it; `total` 0 means it gives no count. */
export interface Step {
  phase: Phase
  table?: string
  total: number
}

const TICK_MS = 140
const TICKS_PER_STEP = 9

/** The phases each operation reports (`vault-worker`, `vault-core/src/maintenance`). */
export const operationSteps = {
  encryptionChange: [
    { phase: 'copying', table: 'visits', total: 1_284_310 },
    { phase: 'verifying', table: 'visits', total: 1_284_310 },
    { phase: 'indexing', total: 0 },
    { phase: 'snapshotting', total: 0 },
  ],
  passwordChange: [{ phase: 'rewrapping', total: 0 }],
  export: [{ phase: 'exporting', total: 0 }],
  import: [{ phase: 'importing', total: 0 }],
  restore: [{ phase: 'restoring', total: 0 }],
} satisfies Record<string, Step[]>

export function MaintenanceProgress({ steps, onDone, onStop, onStopped }: { steps: Step[]; onDone: () => void; onStop?: () => void; onStopped: () => void }) {
  const [tick, setTick] = useState(0)
  const [stopping, setStopping] = useState(false)
  const end = steps.length * TICKS_PER_STEP
  const done = useRef({ onDone, onStopped })
  done.current = { onDone, onStopped }

  useEffect(() => {
    if (stopping) {
      const timer = setTimeout(() => done.current.onStopped(), 700)
      return () => clearTimeout(timer)
    }
    if (tick >= end) {
      done.current.onDone()
      return
    }
    const timer = setTimeout(() => setTick((value) => value + 1), TICK_MS)
    return () => clearTimeout(timer)
  }, [tick, end, stopping])

  const step = steps[Math.min(steps.length - 1, Math.floor(tick / TICKS_PER_STEP))]
  const within = (tick % TICKS_PER_STEP) / (TICKS_PER_STEP - 1)
  const processed = Math.round(step.total * Math.min(1, within))
  const percent = step.total > 0 ? Math.min(100, (processed / step.total) * 100) : undefined

  return (
    <div role="status" className="flex flex-col gap-1.5">
      <Progress value={percent} className="h-1.5" aria-label="進度" />
      <div className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
        <span className="min-w-0 truncate tabular">
          {phaseText[step.phase]}
          {step.table && ` · ${step.table}`}
          {step.total > 0 ? ` · ${count(processed)} / ${count(step.total)}` : ''}
        </span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={stopping}
          onClick={() => {
            setStopping(true)
            onStop?.()
          }}
        >
          {stopping ? '正在停止…' : '停止'}
        </Button>
      </div>
    </div>
  )
}

/** 已停止，沒有任何變動。, shown after a stop the user asked for. */
export function StoppedLine() {
  return (
    <p role="status" className="text-[13px] text-muted-foreground">
      已停止，沒有任何變動。
    </p>
  )
}
