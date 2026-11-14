/**
 * Live progress while a backup runs. Mirrors `src/features/backup/progress-card.tsx`:
 * 「正在準備…」 until the first progress event, then which source of how many,
 * the source's name, records read and a bar.
 */
import { number } from './format'
import { useScene } from './scene'

export function ProgressCard() {
  const { running, progress } = useScene()
  if (!running) return null

  const total = progress?.totalProfiles ?? 0
  const percent = progress?.progressPercent ?? (total > 0 ? (progress!.completedProfiles / total) * 100 : null)
  const processed = progress?.processedRecords
  const records = progress?.totalRecords

  return (
    <div className="flex animate-rise flex-col gap-2.5 rounded-xl border bg-card px-[18px] py-3.5 shadow-card">
      <div className="flex items-baseline justify-between gap-4 text-sm">
        <span className="truncate">
          {total > 0 ? `正在備份第 ${Math.min(progress!.completedProfiles + 1, total)} / ${total} 個來源` : '正在準備…'}
          {progress?.sourceLabel && <span className="text-muted-foreground"> · {progress.sourceLabel}</span>}
        </span>
        {processed != null && records != null && records > 0 && (
          <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">
            {number(processed)} / {number(records)}
          </span>
        )}
      </div>
      <div
        role="progressbar"
        aria-label="備份進度"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent == null ? undefined : Math.round(percent)}
        className="h-1 overflow-hidden rounded-full bg-muted"
      >
        <div
          className={percent == null ? 'h-full w-1/3 animate-indeterminate rounded-full bg-brand' : 'h-full rounded-full bg-brand transition-[width] duration-300'}
          style={percent == null ? undefined : { width: `${Math.min(100, Math.max(2, percent))}%` }}
        />
      </div>
    </div>
  )
}
