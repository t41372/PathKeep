/**
 * The small "≈ …" beside the hours of Agent work time and of the time you
 * were there (`agent-history.md` §1.12): what those hours come to, one
 * reading at a time. It only converts the number beside it; the hours stay.
 * Pressing it switches both tiles to the other reading, which is saved, so
 * the two stay comparable and the choice holds next time.
 */
import { ArrowLeftRight } from 'lucide-react'
import type { ReactNode } from 'react'
import type { EquivalentReading } from '@/lib/backend-client/agent-insights'
import { useI18n } from '@/lib/i18n'
import { useInsightsFormat } from '@/features/insights/zone'
import { equivalents } from './words'

const small =
  'text-[12.5px] leading-none font-normal tracking-normal whitespace-nowrap text-muted-foreground'

export function Equivalent({
  ms,
  reading,
  onSwitch,
}: {
  ms: number
  reading: EquivalentReading
  onSwitch: (next: EquivalentReading) => void
}) {
  const { t } = useI18n()
  const f = useInsightsFormat()
  const found = equivalents(t, f, ms, reading)
  if (!found) return null
  const shown = t('agentInsights.equivalent.shown', { reading: found.shown })
  if (!found.other)
    return (
      <span className={small} title={found.detail}>
        {shown}
      </span>
    )
  return (
    <button
      type="button"
      title={found.detail}
      aria-label={t('agentInsights.equivalent.press', {
        reading: found.shown,
        other: found.other,
      })}
      onClick={() => onSwitch(reading === 'nonstop' ? 'workdays' : 'nonstop')}
      className={`${small} group -mx-1.5 inline-flex items-center gap-1 rounded-md px-1.5 py-1 transition-colors outline-none hover:bg-muted hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/40`}
    >
      {/* The key restarts the short rise when the reading changes. */}
      <span key={reading} className="animate-rise">
        {shown}
      </span>
      <ArrowLeftRight
        aria-hidden
        className="size-3 opacity-50 transition-opacity group-hover:opacity-100"
      />
    </button>
  )
}

/** An hours figure with its "≈" on the same line when it fits, else under it. */
export function HoursWithEquivalent({
  children,
  equivalent,
}: {
  children: ReactNode
  equivalent: ReactNode
}) {
  return (
    <span className="flex flex-wrap items-baseline gap-x-2.5 gap-y-0.5">
      <span className="whitespace-nowrap">{children}</span>
      {equivalent}
    </span>
  )
}
