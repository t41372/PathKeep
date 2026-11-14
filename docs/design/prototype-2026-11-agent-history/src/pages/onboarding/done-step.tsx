/**
 * 完成: today's step (src/features/onboarding/done-step.tsx) with §3.3's
 * title, body and summary rows: a summary of every choice, then the stages
 * that carry them out, each with its state, errors in place with retry and
 * skip, and the result. The one change in the run: the first backup shows a
 * line of progress per chosen type (ProgressCard showed one).
 */
import { Bot, Check, Earth, Folder, Globe, Layers, Lock, Minus, RefreshCw, Sparkles, X, type LucideIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/cn'
import { num } from '~/data/format'
import { InlineError } from './inline-error'
import { StepHeading } from './steps'
import { skippable, type FinishState, type Stage, type TypeProgress } from './use-finish'

export interface SummaryRow {
  icon: LucideIcon
  label: string
  value: string
}

/** Prototype-only: how the first backup turned out when it did not fail. */
export type ResultCase = 'all' | 'safari' | 'partial'

export const summaryIcons = { kinds: Layers, browsing: Globe, agent: Bot, location: Folder, encryption: Lock, schedule: RefreshCw, ai: Sparkles, online: Earth }

const stageLabels: Record<Stage, string> = { archive: '建立存檔', keychain: '把密碼存到鑰匙圈', schedule: '安裝排程', backup: '第一次備份', ai: '設定 AI' }
const failedTitles: Record<Stage, string> = { archive: '無法建立存檔', keychain: '無法把密碼存到鑰匙圈', schedule: '無法安裝排程', backup: '第一次備份失敗', ai: '無法設定 AI' }
const skipLabels: Record<Exclude<Stage, 'archive'>, string> = {
  keychain: '不用鑰匙圈，繼續',
  schedule: '跳過，之後在「備份」裡設定',
  backup: '跳過，之後再備份',
  ai: '跳過，之後在「設定」裡設定',
}
const stateNames = { pending: '未開始', running: '進行中', done: '已完成', failed: '失敗', skipped: '已略過' }

export function DoneStep({
  summary,
  stages,
  finish,
  onRetry,
  onSkip,
  localAi,
  result,
}: {
  summary: SummaryRow[]
  stages: Stage[]
  finish: FinishState
  onRetry: () => void
  onSkip: () => void
  localAi: boolean
  result: ResultCase
}) {
  const results = finish.progress
  return (
    <>
      <StepHeading title="都設定好了" body="接下來會做第一次完整備份，之後每次只追加新的記錄。" />
      <dl className="overflow-hidden rounded-xl border bg-card shadow-card">
        {summary.map(({ icon: Icon, label, value }) => (
          <div key={label} className="flex items-center gap-3 border-b px-4 py-3 text-[13px] last:border-b-0">
            <Icon className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} aria-hidden />
            <dt className="w-28 shrink-0 text-muted-foreground">{label}</dt>
            <dd className="min-w-0 flex-1 truncate font-medium" title={value}>
              {value}
            </dd>
          </div>
        ))}
      </dl>
      {finish.status !== 'idle' && (
        <ol className="flex animate-rise flex-col gap-2.5" aria-label="設定進度">
          {stages.map((stage) => (
            <StageRow key={stage} stage={stage} finish={finish} />
          ))}
        </ol>
      )}
      {finish.current === 'backup' && finish.status === 'running' && <BackupProgress progress={finish.progress} />}
      {finish.status === 'failed' && finish.error && (
        <InlineError
          title={failedTitles[finish.error.stage]}
          detail={finish.error.message}
          actions={
            <>
              <Button size="sm" onClick={onRetry}>
                再試一次
              </Button>
              {skippable.has(finish.error.stage) && (
                <Button size="sm" variant="outline" onClick={onSkip}>
                  {skipLabels[finish.error.stage as Exclude<Stage, 'archive'>]}
                </Button>
              )}
            </>
          }
        />
      )}
      {finish.status === 'done' && (
        <div role="status" className="flex animate-rise flex-col gap-1.5 rounded-xl bg-green/10 px-4 py-3.5 text-[13px]">
          <span className="flex items-center gap-2 font-medium">
            <Check className="size-4 text-green" strokeWidth={2.5} aria-hidden />
            {finish.saved === null ? '設定已完成。準備好了就點「立即備份」。' : results.length === 1 && results[0].id === 'browsing' ? `完成，共保存 ${num(results[0].records)} 條記錄。` : '完成。'}
          </span>
          {/* Each type counts in its own unit; the numbers are never added together. */}
          {finish.saved !== null &&
            !(results.length === 1 && results[0].id === 'browsing') &&
            results.map((t) => (
              <span key={t.id} className="text-muted-foreground">
                {t.label}：{t.id === 'browsing' ? `新增 ${num(t.records)} 條記錄` : t.id === 'agent' ? `存了 ${num(t.records)} 段對話` : `存了 ${num(t.records)} 筆`}
              </span>
            ))}
          {finish.saved !== null && result === 'safari' && <span className="text-muted-foreground">Safari 已略過：PathKeep 還沒有「完整磁碟取用權限」。</span>}
          {finish.saved !== null && result === 'partial' && <span className="text-muted-foreground">部分所選瀏覽器讀取失敗。讀得到的都已儲存，是哪些、為什麼，請看「備份」。</span>}
          {localAi && finish.completed.includes('ai') && !finish.skipped.includes('ai') && <span className="text-muted-foreground">語意搜尋模型正在背景下載，可以在「設定 → AI」查看進度。</span>}
        </div>
      )}
    </>
  )
}

function StageRow({ stage, finish }: { stage: Stage; finish: FinishState }) {
  const state = finish.skipped.includes(stage)
    ? 'skipped'
    : finish.completed.includes(stage)
      ? 'done'
      : finish.current === stage
        ? finish.status === 'failed'
          ? 'failed'
          : 'running'
        : 'pending'
  return (
    <li className={cn('flex items-center gap-2.5 text-[13px] transition-colors duration-200', (state === 'pending' || state === 'skipped') && 'text-muted-foreground')}>
      <span className="flex size-[18px] shrink-0 items-center justify-center">
        {state === 'done' ? (
          <span className="flex size-[18px] animate-rise items-center justify-center rounded-full bg-green text-white">
            <Check className="size-3" strokeWidth={3} aria-hidden />
          </span>
        ) : state === 'running' ? (
          <Spinner aria-hidden role={undefined} className="text-brand" />
        ) : state === 'failed' ? (
          <span className="flex size-[18px] items-center justify-center rounded-full bg-destructive text-white">
            <X className="size-3" strokeWidth={3} aria-hidden />
          </span>
        ) : state === 'skipped' ? (
          <Minus className="size-3.5" strokeWidth={2.5} aria-hidden />
        ) : (
          <span className="size-3 rounded-full border-[1.5px] border-border" />
        )}
      </span>
      <span className={cn(state === 'skipped' && 'line-through')}>{stageLabels[stage]}</span>
      <span className="sr-only">{stateNames[state]}</span>
    </li>
  )
}

/** The §3.3 change: one progress line per chosen type, each its own task, in the app's ProgressCard style. */
function BackupProgress({ progress }: { progress: TypeProgress[] }) {
  if (progress.length === 0) {
    return (
      <div className="flex animate-rise flex-col gap-2.5 rounded-xl border bg-card px-[18px] py-3.5 shadow-card">
        <span className="text-sm">正在準備…</span>
        <div role="progressbar" aria-label="備份進度" className="h-1 overflow-hidden rounded-full bg-muted">
          <div className="h-full w-1/3 animate-indeterminate rounded-full bg-brand" />
        </div>
      </div>
    )
  }
  return (
    <div className="flex animate-rise flex-col gap-3.5 rounded-xl border bg-card px-[18px] py-3.5 shadow-card">
      {progress.map((type) => {
        const total = type.sources.length
        const finished = type.completed >= total
        const percent = type.records > 0 ? (type.processed / type.records) * 100 : 0
        return (
          <div key={type.id} className="flex flex-col gap-2">
            <div className="flex items-baseline justify-between gap-4 text-sm">
              <span className="truncate">
                <span className="font-medium">{type.label}</span>
                <span className="text-muted-foreground"> · </span>
                {finished ? (
                  <Check className="inline size-3.5 text-green" strokeWidth={2.5} aria-label="已完成" />
                ) : (
                  <>
                    正在備份第 {Math.min(type.completed + 1, total)} / {total} 個來源
                    <span className="text-muted-foreground"> · {type.sources[Math.min(type.completed, total - 1)]}</span>
                  </>
                )}
              </span>
              <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">
                {num(type.processed)} / {num(type.records)}
              </span>
            </div>
            <div role="progressbar" aria-label={`備份進度 · ${type.label}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(percent)} className="h-1 overflow-hidden rounded-full bg-muted">
              <div className="h-full rounded-full bg-brand transition-[width] duration-300" style={{ width: `${Math.min(100, Math.max(2, percent))}%` }} />
            </div>
          </div>
        )
      })}
    </div>
  )
}
