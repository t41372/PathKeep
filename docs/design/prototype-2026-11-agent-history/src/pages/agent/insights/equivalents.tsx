/**
 * "≈ …" beside an hours figure: what those hours come to, one reading at a
 * time. The hours stay as they are; the small text beside them is a button
 * that switches between days without a break and 8-hour working time. Both
 * cards that carry it (「Agent 工作時間」, 「你在場的時間」) follow one choice,
 * so the two stay comparable, and the choice is remembered.
 *
 * It is a way to feel the size of the number, so it only appears once the
 * number is big enough to say something: from 8 hours (one working day). Under
 * 24 hours there is only the working-time reading and nothing to switch.
 */
import { ArrowLeftRight } from 'lucide-react'
import { useSyncExternalStore } from 'react'

const WORK_DAY_HOURS = 8
const WORK_DAYS_PER_YEAR = 260 // five days a week
const WORK_DAYS_PER_MONTH = WORK_DAYS_PER_YEAR / 12

type Reading = 'nonstop' | 'fullTime'
const KEY = 'pk-proto-hours-reading'
let reading: Reading = 'nonstop'
try {
  if (localStorage.getItem(KEY) === 'fullTime') reading = 'fullTime'
} catch {
  // No storage (private window): the choice lasts for this visit.
}
const listeners = new Set<() => void>()
function setReading(next: Reading) {
  reading = next
  try {
    localStorage.setItem(KEY, next)
  } catch {
    // Same as above.
  }
  listeners.forEach((listener) => listener())
}
function useReading() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => reading,
  )
}

function amount(value: number) {
  const rounded = value < 10 ? Math.round(value * 10) / 10 : Math.round(value)
  return rounded.toLocaleString('zh-TW')
}

export function equivalents(ms: number) {
  const hours = ms / 3_600_000
  if (hours < WORK_DAY_HOURS) return null
  const days = hours / 24
  const workDays = hours / WORK_DAY_HOURS
  const workMonths = workDays / WORK_DAYS_PER_MONTH
  const workYears = workDays / WORK_DAYS_PER_YEAR
  const nonstop = hours < 24 ? null : days < 1000 ? `${amount(days)} 天` : `${amount(days / 365)} 年`
  const fullTime = workDays < 22 ? `${amount(workDays)} 個工作天` : workMonths < 12 ? `${amount(workMonths)} 個月` : `${amount(workYears)} 年`
  const detail = [
    nonstop && `把這些小時連在一起，是 ${amount(days)} 天不眠不休。`,
    `按 8 小時工作制（每天 8 小時、每週 5 天）算，是 ${amount(workDays)} 個工作天` +
      (workDays >= 22 ? `，約 ${amount(workMonths)} 個月` : '') +
      (workMonths >= 12 ? `，${amount(workYears)} 年` : '') +
      '。',
  ]
    .filter(Boolean)
    .join('')
  return { nonstop, fullTime, detail }
}

const small = 'text-[12.5px] leading-none font-normal tracking-normal whitespace-nowrap text-muted-foreground'

/** Sits on the same line as the hours, in small type. */
export function Equivalent({ ms }: { ms: number }) {
  const chosen = useReading()
  const e = equivalents(ms)
  if (!e) return null
  if (!e.nonstop) {
    return (
      <span className={small} title={e.detail}>
        ≈ 8 小時工作制 {e.fullTime}
      </span>
    )
  }
  const shown = chosen === 'nonstop' ? `不眠不休 ${e.nonstop}` : `8 小時工作制 ${e.fullTime}`
  return (
    <button
      type="button"
      title={e.detail}
      aria-label={`相當於${shown}。按一下換成${chosen === 'nonstop' ? ' 8 小時工作制' : '不眠不休的天數'}`}
      onClick={() => setReading(chosen === 'nonstop' ? 'fullTime' : 'nonstop')}
      className={`${small} group -mx-1.5 inline-flex items-center gap-1 rounded-md px-1.5 py-1 transition-colors outline-none hover:bg-muted hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/40`}
    >
      {/* The key restarts the short rise when the reading changes. */}
      <span key={chosen} className="animate-rise">
        ≈ {shown}
      </span>
      <ArrowLeftRight className="size-3 opacity-50 transition-opacity group-hover:opacity-100" aria-hidden />
    </button>
  )
}

/** An hours figure with its "≈ …" beside it, for a Stat's value. */
export function HoursWithEquivalent({ ms, children }: { ms: number; children: React.ReactNode }) {
  return (
    <span className="flex flex-wrap items-baseline gap-x-2.5 gap-y-0.5">
      {children}
      <Equivalent ms={ms} />
    </span>
  )
}
