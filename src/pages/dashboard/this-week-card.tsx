/**
 * Dashboard "This week" summary card.
 *
 * Honest LLM fallback: if no provider is configured, render the deterministic
 * page counts directly without forging an editorial summary.
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { useI18n } from '@/lib/i18n'

export interface DashboardThisWeekProps {
  totalPages: number
  totalUrls: number
  recentRunsCount: number
}

export function DashboardThisWeek({
  totalPages,
  totalUrls,
  recentRunsCount,
}: DashboardThisWeekProps) {
  const { t, language } = useI18n()
  const weekNumber = isoWeek(new Date())
  const fmt = (value: number): string =>
    new Intl.NumberFormat(language === 'en' ? 'en-US' : language).format(value)

  return (
    <PaperCard testId="dashboard-this-week">
      <PaperCardHeader
        title={t('dashboard.thisWeekTitle')}
        right={
          <PaperCardBadge>
            {t('dashboard.weekBadge', { week: weekNumber })}
          </PaperCardBadge>
        }
      />
      <PaperCardBody className="px-[18px] py-[14px]">
        <p className="m-0 mb-2 font-serif text-[14px] leading-[1.55] text-ink-secondary">
          {t('dashboard.thisWeekFallbackHeadline')}
        </p>
        <p className="m-0 font-serif text-[13px] leading-[1.55] text-ink-faint italic">
          {t('dashboard.thisWeekFallbackHint')}
        </p>

        <div className="border-border-light mt-[14px] flex gap-0 border-t border-dashed pt-3">
          {[
            { val: fmt(totalPages), label: t('dashboard.weekStatPages') },
            { val: fmt(totalUrls), label: t('dashboard.weekStatUrls') },
            {
              val: String(recentRunsCount),
              label: t('dashboard.weekStatRuns'),
            },
          ].map((stat) => (
            <div key={stat.label} className="flex-1">
              <div className="font-serif text-[20px] font-normal tracking-[-0.01em] text-ink">
                {stat.val}
              </div>
              <div className="mt-0.5 font-mono text-[8.5px] uppercase tracking-[0.06em] text-ink-faint">
                {stat.label}
              </div>
            </div>
          ))}
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}

function isoWeek(date: Date): number {
  const target = new Date(date.valueOf())
  target.setHours(0, 0, 0, 0)
  target.setDate(target.getDate() + 4 - (target.getDay() || 7))
  const firstThursday = new Date(target.getFullYear(), 0, 4)
  const diff = (target.getTime() - firstThursday.getTime()) / 86400000
  return 1 + Math.round(diff / 7)
}
