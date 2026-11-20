/**
 * How Agent Insights put numbers into words: hours and minutes, money,
 * shares, token units, the range and bucket a chart covers, and the "≈"
 * beside an hours figure (`agent-history.md` §1.3, §1.12).
 *
 * Responsibilities: choosing the words and the rounding. A value nothing
 * recorded is the caller's to say ("No record", "—"); these never turn it
 * into 0. Not responsible for: the copy (`agentInsights` messages).
 */
import { hoursText } from '@/features/agent-chats/chat-words'
import { tokenTotal } from '@/features/agent-reader/reader-words'
import type {
  BucketUnit,
  EquivalentReading,
  InsightsRange,
  TokenKinds,
} from '@/lib/backend-client/agent-insights'
import type {
  Formatters,
  MessageKey,
  ResolvedLanguage,
  Translator,
} from '@/lib/i18n'
import { dateKeyIn } from '@/lib/time-zone'

export { tokenTotal }

/** The ranges in the toggle, in the order §1.2 lists them. */
export const RANGES: readonly InsightsRange[] = [
  'today',
  '7d',
  '30d',
  '90d',
  '1y',
  'all',
]

/** Under an hour in minutes, then hours: "45 min", "3.5 h", "120 h". */
export function hours(t: Translator, f: Formatters, ms: number) {
  return hoursText(t, f, ms)
}

/** US dollars; small amounts keep the cents' fractions that make them up. */
export function usd(value: number, locale: string) {
  return new Intl.NumberFormat(locale, {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: value > 0 && value < 1 ? 4 : 2,
  }).format(value)
}

/** A share of a whole, "<1%" for a sliver that is not nothing. */
export function share(f: Formatters, part: number, whole: number) {
  if (whole <= 0) return null
  const ratio = part / whole
  return ratio > 0 && ratio < 0.01 ? '<1%' : f.percent(ratio)
}

/** One decimal under 10, whole numbers from 10 (§1.12). */
const rounded = (value: number) =>
  value < 10 ? Math.round(value * 10) / 10 : Math.round(value)

export function amount(f: Formatters, value: number) {
  return f.number(rounded(value))
}

export interface TokenUnit {
  divisor: number
  /** The words after the subtitle, or `null` when the numbers are plain. */
  key: MessageKey | null
}

/**
 * The unit a token chart's axis counts in, from its largest bar: Chinese
 * readers count in 萬 and 億, English in thousands, millions, billions.
 */
export function tokenUnit(max: number, language: ResolvedLanguage): TokenUnit {
  const base = 'agentInsightsCards.tokens.units.'
  if (language === 'en') {
    if (max >= 1e9) return { divisor: 1e9, key: `${base}billion` as MessageKey }
    if (max >= 1e6) return { divisor: 1e6, key: `${base}million` as MessageKey }
    if (max >= 1e3)
      return { divisor: 1e3, key: `${base}thousand` as MessageKey }
    return { divisor: 1, key: null }
  }
  if (max >= 1e8)
    return { divisor: 1e8, key: `${base}hundredMillion` as MessageKey }
  if (max >= 1e4)
    return { divisor: 1e4, key: `${base}tenThousand` as MessageKey }
  return { divisor: 1, key: null }
}

/** The bar length each range has (§1.3), for the subtitle before data arrives. */
export const RANGE_UNIT: Record<InsightsRange, BucketUnit> = {
  today: 'hour',
  '7d': 'day',
  '30d': 'day',
  '90d': 'day',
  '1y': 'week',
  all: 'month',
}

export const windowKey = (range: InsightsRange) =>
  `agentInsights.window.${range}` as MessageKey
export const spanKey = (range: InsightsRange) =>
  `agentInsights.span.${range}` as MessageKey
export const rangeKey = (range: InsightsRange) =>
  `agentInsights.range.${range}` as MessageKey
export const perKey = (unit: BucketUnit) =>
  `agentInsightsCards.per.${unit}` as MessageKey

/** A bar's label on the axis: its hour, day, week's Monday or month. */
export function bucketTick(
  f: Formatters,
  unit: BucketUnit,
  start: number,
  zone: string,
) {
  if (unit === 'hour') return f.time(start)
  const day = dateKeyIn(start, zone)
  return unit === 'month'
    ? f.day(day, { month: 'short', year: 'numeric' })
    : f.day(day, { month: 'short', day: 'numeric' })
}

/** A bar's name in its tooltip: the hour span, the day, the week or the month. */
export function bucketTitle(
  t: Translator,
  f: Formatters,
  unit: BucketUnit,
  start: number,
  end: number,
  zone: string,
) {
  if (unit === 'hour') return f.timeRange(start, end)
  const day = dateKeyIn(start, zone)
  if (unit === 'month') return f.day(day, { month: 'long', year: 'numeric' })
  if (unit === 'week')
    return t('agentInsightsCards.work.week', {
      day: f.day(day, { month: 'long', day: 'numeric' }),
    })
  return f.day(day, { weekday: 'short', month: 'long', day: 'numeric' })
}

/** The sum of the four kinds, cache reads left out unless asked for. */
export function tokensOf(tokens: TokenKinds, withCacheRead: boolean) {
  return withCacheRead
    ? tokenTotal(tokens)
    : tokens.input + tokens.cacheWrite + tokens.output
}

const WORK_DAY_HOURS = 8
const WORK_DAYS_PER_YEAR = 260
const WORK_DAYS_PER_MONTH = WORK_DAYS_PER_YEAR / 12

export interface Equivalents {
  /** The reading shown now, in words. */
  shown: string
  /** The other reading's name, or `null` when there is nothing to switch to. */
  other: string | null
  /** Every reading that applies, for hovering. */
  detail: string
}

/**
 * What an hours figure comes to (§1.12): days without a break, or
 * 8-hour working days, months and years. Nothing under 8 hours; under 24
 * only the working-day reading, which cannot switch.
 */
export function equivalents(
  t: Translator,
  f: Formatters,
  ms: number,
  reading: EquivalentReading,
): Equivalents | null {
  const total = ms / 3_600_000
  if (total < WORK_DAY_HOURS) return null
  const days = total / 24
  const workDays = total / WORK_DAY_HOURS
  const months = workDays / WORK_DAYS_PER_MONTH
  const years = workDays / WORK_DAYS_PER_YEAR
  const say = (key: string, value: number) =>
    t(`agentInsights.equivalent.${key}` as MessageKey, {
      count: rounded(value),
      n: amount(f, value),
    })
  const work =
    workDays < 22
      ? say('workDays', workDays)
      : months < 12
        ? say('workMonths', months)
        : say('workYears', years)
  const workDetail =
    workDays < 22
      ? t('agentInsights.equivalent.detailDays', {
          workDays: amount(f, workDays),
        })
      : months < 12
        ? t('agentInsights.equivalent.detailMonths', {
            workDays: amount(f, workDays),
            months: amount(f, months),
          })
        : t('agentInsights.equivalent.detailYears', {
            workDays: amount(f, workDays),
            months: amount(f, months),
            years: amount(f, years),
          })
  if (total < 24) return { shown: work, other: null, detail: workDetail }
  const nonstop =
    days < 1000 ? say('nonstopDays', days) : say('nonstopYears', days / 365)
  const detail = `${t('agentInsights.equivalent.detailNonstop', { days: amount(f, days) })} ${workDetail}`
  return reading === 'nonstop'
    ? { shown: nonstop, other: t('agentInsights.equivalent.asWork'), detail }
    : { shown: work, other: t('agentInsights.equivalent.asNonstop'), detail }
}

/** A project's name; the loose folders have one of their own. */
export function projectName(
  t: Translator,
  project: { name?: string; displayName?: string; kind: string },
) {
  const name = project.name ?? project.displayName ?? ''
  return (
    name || (project.kind === 'scratch' ? t('agentInsights.filter.loose') : '')
  )
}
