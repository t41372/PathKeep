/**
 * The step heading every onboarding step shows (with the "03 / 08" counter
 * from src/features/onboarding/onboarding-page.tsx), the welcome step and the
 * new "要保存什麼" step. The other steps have their own files; "來源" is
 * sources-step.tsx.
 */
import { Check, Copy, HardDrive, Search, type LucideIcon } from 'lucide-react'
import { createContext, useContext, type ReactNode } from 'react'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/cn'
import arcIcon from '@/assets/browser-icons/arc.png'
import chromeIcon from '@/assets/browser-icons/chrome.png'
import firefoxIcon from '@/assets/browser-icons/firefox.svg'
import safariIcon from '@/assets/browser-icons/safari.png'
import darkMark from '@/assets/pathkeep-mark-dark.svg'
import lightMark from '@/assets/pathkeep-mark.svg'
import { HarnessIcon } from '~/components/bits'
import { kinds, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'

/** Which step is showing, for the counter above each heading. */
export const StepPosition = createContext<{ index: number; total: number } | null>(null)

const pad = (value: number) => String(value).padStart(2, '0')

export function StepHeading({ title, body }: { title: string; body?: ReactNode }) {
  const position = useContext(StepPosition)
  return (
    <header className="flex flex-col gap-2.5">
      {position && (
        <span className="font-mono text-xs font-medium text-brand">
          {pad(position.index + 1)} / {pad(position.total)}
        </span>
      )}
      <h1 data-step-heading tabIndex={-1} className="text-[32px] leading-[1.15] font-semibold tracking-[-0.025em] text-pretty outline-none">
        {title}
      </h1>
      {body && <p className="text-[15px] leading-relaxed text-pretty text-muted-foreground">{body}</p>}
    </header>
  )
}

const retention: { name: string; icon: ReactNode; keep: string; width: string }[] = [
  { name: 'Claude Code', icon: <HarnessIcon id="claude" className="size-5" />, keep: '預設 30 天', width: '3%' },
  { name: 'Chrome', icon: <img src={chromeIcon} alt="" className="size-5" />, keep: '約 90 天', width: '8%' },
  { name: 'Arc', icon: <img src={arcIcon} alt="" className="size-5" />, keep: '約 90 天', width: '8%' },
  { name: 'Safari', icon: <img src={safariIcon} alt="" className="size-5" />, keep: '1 年', width: '32%' },
  { name: 'Firefox', icon: <img src={firefoxIcon} alt="" className="size-5" />, keep: '看容量', width: '55%' },
  { name: 'Codex', icon: <HarnessIcon id="codex" className="size-5" />, keep: '不自動刪，但常被手動清掉', width: '64%' },
]

const promises: { icon: LucideIcon; title: string; body: string }[] = [
  { icon: HardDrive, title: '只在本機', body: '不上傳，不需要帳號' },
  { icon: Copy, title: '不碰原檔', body: '先複製，再增量追加' },
  { icon: Search, title: '可以搜尋', body: '全文、正則、時間線' },
]

/** Today's welcome step with §3.3's copy and rows; the PathKeep "永久" row is today's. */
export function WelcomeStep() {
  const { theme } = useProto()
  return (
    <>
      <StepHeading title="留住會被自動刪掉的東西" body="瀏覽紀錄、跟 Agent 的對話，很多工具過一陣子就自己清掉了。PathKeep 定時把它們複製進你電腦上的存檔，之後隨時能找、能看。" />
      <section className="flex flex-col gap-3.5 rounded-xl border bg-card p-5 shadow-card">
        <h2 className="text-[13px] font-normal text-muted-foreground">各家自己會留多久</h2>
        <ul className="flex flex-col gap-3.5">
          {retention.map((row) => (
            <li key={row.name} className="flex items-center gap-3">
              <span className="flex size-5 shrink-0 items-center justify-center">{row.icon}</span>
              <span className="w-24 shrink-0 text-[13px]">{row.name}</span>
              <span className="h-2 flex-1 overflow-hidden rounded bg-muted">
                <span className="block h-full rounded bg-foreground/75" style={{ width: row.width }} />
              </span>
              <span className="w-[168px] shrink-0 text-right font-mono text-xs text-muted-foreground">{row.keep}</span>
            </li>
          ))}
          <li className="flex items-center gap-3 border-t pt-3">
            <img src={theme === 'dark' ? darkMark : lightMark} alt="" className="size-5" />
            <span className="w-24 shrink-0 text-[13px] font-semibold">PathKeep</span>
            <span className="h-2 flex-1 rounded bg-brand" />
            <span className="w-[168px] shrink-0 text-right font-mono text-xs font-medium text-brand">永久</span>
          </li>
        </ul>
      </section>
      <ul className="grid grid-cols-3 gap-2.5">
        {promises.map(({ icon: Icon, title, body }) => (
          <li key={title} className="flex flex-col gap-1.5 rounded-[10px] bg-muted p-3.5">
            <Icon className="size-[18px]" strokeWidth={1.75} aria-hidden />
            <span className="text-[13px] font-medium">{title}</span>
            <span className="text-xs text-muted-foreground">{body}</span>
          </li>
        ))}
      </ul>
    </>
  )
}

export function KindsStep({ selected, onToggle, pickAll, notFound }: { selected: Set<KindId>; onToggle: (id: KindId) => void; pickAll?: boolean; /** Types with no source on this computer: not ticked to begin with. */ notFound: Set<KindId> }) {
  return (
    <>
      <StepHeading title="要保存什麼？" body="每一種之後都能在設定裡加減。關掉不會刪掉已經存下的資料。" />
      <div className="grid grid-cols-2 gap-3 max-[760px]:grid-cols-1">
        {kinds.map((registered) => {
          // The stress set treats the planned types as ready, to see many chosen at once.
          const kind = pickAll ? { ...registered, real: true } : registered
          const on = selected.has(kind.id)
          const Icon = kind.icon
          return (
            <button
              key={kind.id}
              type="button"
              role="checkbox"
              aria-checked={on}
              disabled={!kind.real}
              onClick={() => onToggle(kind.id)}
              className={cn(
                'group flex flex-col gap-2 rounded-xl border-[1.5px] bg-card p-4 text-left shadow-card transition-[border-color,box-shadow] duration-150 outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
                kind.real ? 'hover:border-foreground/30' : 'cursor-not-allowed bg-card/40 shadow-none',
                on && 'border-foreground',
              )}
            >
              <span className="flex items-center gap-2.5">
                <span className={cn('flex size-8 items-center justify-center rounded-lg bg-muted', !kind.real && 'opacity-60')}>
                  <Icon className="size-4" strokeWidth={1.75} />
                </span>
                <span className={cn('flex-1 font-medium', !kind.real && 'text-muted-foreground')}>{kind.label}</span>
                {kind.real ? (
                  <span className={cn('flex size-[18px] items-center justify-center rounded-[5px] border-[1.5px] border-border transition-colors', on && 'border-foreground bg-foreground text-background')}>
                    {on && <Check className="size-3" strokeWidth={3} />}
                  </span>
                ) : (
                  <Badge variant="outline" className="font-normal text-muted-foreground">
                    規劃中
                  </Badge>
                )}
              </span>
              <span className="text-xs leading-relaxed text-muted-foreground">{kind.why}</span>
              <span className="text-[11.5px] text-muted-foreground/80">{kind.sources.join('、')}</span>
              {notFound.has(kind.id) && <span className="text-[11.5px] text-brand">這台電腦上沒有找到，所以先不選。之後找到了，首頁會問你要不要保存。</span>}
            </button>
          )
        })}
      </div>
      {selected.size === 0 && <p className="text-[13px] text-destructive">至少選一種。</p>}
    </>
  )
}
