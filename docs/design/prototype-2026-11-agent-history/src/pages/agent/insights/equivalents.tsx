/**
 * "相當於…": what a number of hours comes to as days without a break and as
 * full-time work, in small type under 「Agent 工作時間」 and 「你在場」.
 *
 * It is a way to feel the size of the number, so it only appears once the
 * number is big enough to say something: from 8 hours (one working day), with
 * the days-without-a-break part from 24 hours. Each part uses the largest unit
 * that keeps the figure readable; the tooltip gives all of them.
 */
const WORK_DAY_HOURS = 8
const WORK_DAYS_PER_YEAR = 260 // five days a week
const WORK_DAYS_PER_MONTH = WORK_DAYS_PER_YEAR / 12

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

export function Equivalents({ ms }: { ms: number }) {
  const e = equivalents(ms)
  if (!e) return null
  return (
    <span title={e.detail}>
      {/* Two unbreakable halves, so a narrow card wraps between them and not inside one. */}
      {e.nonstop && <span className="whitespace-nowrap">相當於不眠不休 {e.nonstop} · </span>}
      <span className="whitespace-nowrap">
        {!e.nonstop && '相當於 '}8 小時工作制 {e.fullTime}
      </span>
    </span>
  )
}
