/** Small display helpers shared by the Insights cards and drill-ins. */
import type { Formatters, Translator } from '@/lib/i18n'

/** A full-width row link inside a card list, with hover feedback. */
export const rowLink =
  'flex min-w-0 items-center gap-2.5 rounded-md px-2 py-1.5 text-[13px] transition-colors hover:bg-muted hover:no-underline'

/** Active time as minutes under 90 minutes, otherwise hours. */
export function formatDuration(ms: number, t: Translator, format: Formatters) {
  const minutes = ms / 60_000
  if (minutes < 90)
    return t('insights.kpi.minutes', {
      value: format.number(Math.round(minutes)),
    })
  return t('insights.kpi.hours', {
    value: format.number(Math.round(minutes / 60)),
  })
}

/** Share of a bar, for the soft fill behind ranked rows. */
export function shareBar(value: number, max: number) {
  return { width: `${Math.max(2, (value / Math.max(1, max)) * 100)}%` }
}
