/**
 * Dashboard "On this day, a year ago" hero card.
 *
 * Reads OnThisDayEntry records from the core-intelligence API (loaded by the
 * parent route). Each entry is a same-day rollup from a previous year — top
 * domains, total visits, optional LLM summary. Clicking jumps into Explorer
 * pinned to that date.
 */

import { useMemo } from 'react'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { useI18n } from '@/lib/i18n'
import type { OnThisDayEntry } from '@/lib/core-intelligence/types'

export interface DashboardOnThisDayProps {
  entries: OnThisDayEntry[]
  loading: boolean
  error: string | null
  onJumpToDate: (date: string) => void
  onOpenEntry: (entry: OnThisDayEntry) => void
}

export function DashboardOnThisDay({
  entries,
  loading,
  error,
  onJumpToDate,
  onOpenEntry,
}: DashboardOnThisDayProps) {
  const { t, language } = useI18n()
  const targetDate = useMemo(() => {
    const oneYearAgo = new Date()
    oneYearAgo.setFullYear(oneYearAgo.getFullYear() - 1)
    return oneYearAgo
  }, [])

  const targetIso = targetDate.toISOString().slice(0, 10)
  const targetLabel = targetDate.toLocaleDateString(
    language === 'en' ? 'en-US' : language,
    {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    },
  )

  const visible = entries.slice(0, 4)
  const hasSummary = visible.some((entry) => entry.summary)

  return (
    <PaperCard accent testId="dashboard-on-this-day">
      <PaperCardHeader
        title={t('dashboard.onThisDayTitle')}
        right={
          <button
            type="button"
            onClick={() => onJumpToDate(targetIso)}
            className="border-border-default text-ink-muted hover:border-accent hover:text-accent-text hover:bg-accent-soft rounded-paper inline-flex items-center border px-[10px] py-[2px] font-mono text-[10px] tracking-[0.04em] transition-colors"
          >
            {targetLabel}
            <span aria-hidden className="ml-1.5">
              →
            </span>
          </button>
        }
      />
      <PaperCardBody className="px-[18px] py-[14px]">
        {loading ? (
          <div className="pk-skeleton h-[160px] w-full" />
        ) : error ? (
          <div className="font-serif text-[13px] italic text-ink-faint">
            {t('dashboard.onThisDayError')}
          </div>
        ) : visible.length === 0 ? (
          <div className="font-serif text-[13px] italic text-ink-faint">
            {t('dashboard.onThisDayEmpty')}
          </div>
        ) : (
          <ul className="m-0 flex list-none flex-col gap-2 p-0">
            {visible.map((entry) => (
              <li key={`${entry.year}-${entry.date}`}>
                <button
                  type="button"
                  onClick={() => onOpenEntry(entry)}
                  className="grid w-full grid-cols-[64px_1fr] items-start gap-3 py-2 text-left transition-colors hover:bg-hover"
                >
                  <span className="grid h-full place-items-center font-serif text-[18px] font-medium text-accent-text">
                    {entry.year}
                  </span>
                  <span className="flex min-w-0 flex-col">
                    <span className="font-serif text-[13px] leading-snug text-ink">
                      {entry.summary?.trim() ||
                        t('dashboard.onThisDayCountFallback', {
                          count: entry.totalVisits,
                          deep: entry.deepDiveSessions,
                        })}
                    </span>
                    <span className="mt-0.5 truncate font-mono text-[10px] text-ink-faint">
                      {entry.topDomains.slice(0, 4).join(' · ') || entry.date}
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
        {!hasSummary ? (
          <p className="mt-2 font-mono text-[9px] text-ink-ghost">
            ◌ {t('dashboard.onThisDayFallbackNote')}
          </p>
        ) : null}
      </PaperCardBody>
    </PaperCard>
  )
}
