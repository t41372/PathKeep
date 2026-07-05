/**
 * Day-insights strip rendered below each Browse day separator.
 *
 * Mirrors the design tool's `DayInsightsStrip` (pk-contactsheet.jsx:345):
 * three columns — top domains with relative-frequency bars, activity
 * tallies (pages / typed / links / searches), and a 24-hour sparkline
 * with session + distinct-domain counts.
 *
 * ## Responsibilities
 * - Render the three-column strip from a `DayInsights` aggregate.
 * - Stay non-sticky — the strip scrolls with the day's content; only
 *   the day header sticks.
 *
 * ## Not responsible for
 * - Aggregating the visits. That lives in
 *   `paper-day-insights-helpers.ts` so it can be unit-tested without a
 *   DOM.
 * - Localization of the visit-history detail panel (separate copy bag).
 *
 * ## Design history
 * A search-queries + most-revisited hero row was attempted briefly in
 * 2026-05-24 to surface "memory-jog" signals more prominently; the user
 * rejected the visual outcome as worse than the original three-column
 * strip and asked to roll back. The aggregator still extracts
 * `topSearchQueries` / `topUrls` so a future, more carefully art-directed
 * surface can pick them up without re-deriving — they just don't render
 * in the primary panel any more. The `Most revisited` block in the
 * folded More-details disclosure still surfaces `topUrls`, since that
 * was the pre-rollback baseline behaviour.
 *
 * ## Performance notes
 * - The component is a leaf node so React can skip subtree reconciliation
 *   when the parent re-renders without changing `insights`. Callers
 *   wrap the aggregator in `useMemo` keyed on the `PaperDay` reference.
 */

import { cn } from '@/lib/cn'
import { Sparkline } from '@/components/charts'
import { formatDuration, type DayInsights } from './paper-day-insights-helpers'

export interface PaperDayInsightsCopy {
  /** Eyebrow for the top-domains column. */
  topDomainsTitle: string
  /** Eyebrow for the activity tallies column. */
  activityTitle: string
  /** Eyebrow for the hourly sparkline column. */
  hourlyTitle: string
  /** Tally labels — emitted alongside the numeric value. */
  pagesLabel: string
  typedLabel: string
  linksLabel: string
  searchesLabel: string
  /** Template for the session-count caption, e.g. "{count} sessions". */
  sessionsTemplate: string
  /** Template for the distinct-domains caption, e.g. "{count} domains". */
  domainsTemplate: string
  /** Disclosure label for the expandable extra-detail section. */
  moreDetailsLabel: string
  /** Label preceding the day's first visit time. */
  firstVisitLabel: string
  /** Label preceding the day's last visit time. */
  lastVisitLabel: string
  /** Label preceding the peak hour cell (formatted like "3 PM"). */
  peakHourLabel: string
  /** Label preceding the longest-session duration cell. */
  longestSessionLabel: string
  /** Eyebrow for the most-revisited URLs list. */
  topUrlsTitle: string
  /** Plural-aware "{count} visits" template for top-URL rows. */
  visitsCountTemplate: string
}

export interface PaperDayInsightsProps {
  insights: DayInsights
  copy: PaperDayInsightsCopy
  /** Optional language tag for compact-number formatting. */
  language?: string
  /** True (default) to render time stamps in 12h AM/PM. */
  hour12?: boolean
  className?: string
  testId?: string
}

export function PaperDayInsights({
  insights,
  copy,
  language = 'en',
  hour12 = true,
  className,
  testId,
}: PaperDayInsightsProps) {
  if (insights.totalPages === 0) {
    // Days with zero visits surface their empty state via the parent
    // contact sheet's "Nothing here yet" copy; the strip stays hidden
    // so we don't render an honest-looking row of zeros.
    return null
  }
  return (
    <div
      data-testid={testId ?? 'paper-day-insights'}
      className={cn(
        'grid grid-cols-1 gap-4 px-1 pb-4 pt-3 md:grid-cols-3',
        className,
      )}
    >
      <DayInsightsColumn eyebrow={copy.topDomainsTitle}>
        {insights.topDomains.length === 0 ? (
          <span className="font-mono text-[10.5px] text-ink-faint">—</span>
        ) : (
          <ul className="m-0 flex flex-col gap-[3px] p-0">
            {insights.topDomains.map((row) => (
              <li
                key={row.domain}
                data-testid={`paper-day-insights-domain-${row.domain}`}
                className="flex items-center gap-[6px]"
              >
                <span
                  className="font-mono text-[10px] text-ink-muted truncate"
                  style={{ width: 88 }}
                  title={row.domain}
                >
                  {row.domain.replace(/^www\./, '')}
                </span>
                <span
                  className="relative flex-1 h-1 rounded-[1px] bg-page overflow-hidden"
                  aria-hidden="true"
                >
                  <span
                    className="absolute inset-y-0 left-0 bg-accent opacity-55"
                    style={{
                      width: `${(row.visits / Math.max(1, insights.topDomains[0].visits)) * 100}%`,
                    }}
                  />
                </span>
                <span className="w-5 text-right font-mono text-[9.5px] text-ink-faint tabular-nums">
                  {row.visits.toLocaleString(language)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </DayInsightsColumn>

      <DayInsightsColumn eyebrow={copy.activityTitle}>
        <ul className="m-0 flex flex-col gap-[2px] p-0">
          {(
            [
              { label: copy.pagesLabel, value: insights.totalPages },
              { label: copy.typedLabel, value: insights.typedCount },
              { label: copy.linksLabel, value: insights.linkCount },
              { label: copy.searchesLabel, value: insights.searchCount },
            ] as const
          ).map((row) => (
            <li
              key={row.label}
              className="flex justify-between text-[11.5px] text-ink-secondary"
            >
              <span>{row.label}</span>
              <span className="font-mono text-[10.5px] tabular-nums">
                {row.value.toLocaleString(language)}
              </span>
            </li>
          ))}
        </ul>
      </DayInsightsColumn>

      <DayInsightsColumn eyebrow={copy.hourlyTitle}>
        <HourlySparkline insights={insights} />
        <div className="mt-1 flex justify-between text-[11.5px] text-ink-secondary">
          <span>
            {copy.sessionsTemplate.replace(
              '{count}',
              insights.sessionCount.toLocaleString(language),
            )}
          </span>
          <span className="font-mono text-[10.5px] tabular-nums">
            {copy.domainsTemplate.replace(
              '{count}',
              insights.distinctDomains.toLocaleString(language),
            )}
          </span>
        </div>
      </DayInsightsColumn>

      <DayInsightsMoreDetails
        insights={insights}
        copy={copy}
        language={language}
        hour12={hour12}
      />
    </div>
  )
}

function DayInsightsMoreDetails({
  insights,
  copy,
  language,
  hour12,
}: {
  insights: DayInsights
  copy: PaperDayInsightsCopy
  language: string
  hour12: boolean
}) {
  // Why <details>: zero-JS disclosure that respects user agent styling
  // (keyboard, screen reader, search-in-page). The Browse list is
  // scroll-heavy; we don't want a third-party disclosure dependency or
  // a controlled state holder when the platform primitive already does
  // the job and persists open state per page-life.
  const hasExtras =
    insights.firstVisitMs !== null ||
    insights.peakHour !== null ||
    insights.longestSessionMs > 0 ||
    insights.topUrls.length > 0
  if (!hasExtras) return null
  return (
    <details className="col-span-full mt-1 border-t border-border-light pt-2 font-sans text-[11.5px] text-ink-secondary">
      <summary
        data-testid="paper-day-insights-more-summary"
        className="cursor-pointer select-none list-none font-mono text-[10px] uppercase tracking-[0.08em] text-ink-faint hover:text-ink-muted"
      >
        {copy.moreDetailsLabel}
      </summary>
      <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2">
        <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-3 gap-y-[3px] p-0 font-mono text-[10.5px]">
          {insights.firstVisitMs !== null ? (
            <>
              <dt className="text-ink-faint">{copy.firstVisitLabel}</dt>
              <dd className="m-0 text-ink-secondary tabular-nums">
                {formatTime(insights.firstVisitMs, language, hour12)}
              </dd>
            </>
          ) : null}
          {insights.lastVisitMs !== null ? (
            <>
              <dt className="text-ink-faint">{copy.lastVisitLabel}</dt>
              <dd className="m-0 text-ink-secondary tabular-nums">
                {formatTime(insights.lastVisitMs, language, hour12)}
              </dd>
            </>
          ) : null}
          {insights.peakHour !== null ? (
            <>
              <dt className="text-ink-faint">{copy.peakHourLabel}</dt>
              <dd className="m-0 text-ink-secondary tabular-nums">
                {formatHourOfDay(insights.peakHour, language, hour12)}
              </dd>
            </>
          ) : null}
          {insights.longestSessionMs > 0 ? (
            <>
              <dt className="text-ink-faint">{copy.longestSessionLabel}</dt>
              <dd className="m-0 text-ink-secondary tabular-nums">
                {formatDuration(insights.longestSessionMs, language)}
              </dd>
            </>
          ) : null}
        </dl>
        {insights.topUrls.length > 0 ? (
          <div>
            <div className="mb-[6px] font-mono text-[9px] uppercase tracking-[0.08em] text-ink-faint">
              {copy.topUrlsTitle}
            </div>
            <ul className="m-0 flex flex-col gap-[3px] p-0">
              {insights.topUrls.map((row) => (
                <li
                  key={row.url}
                  className="grid grid-cols-[1fr_auto] items-baseline gap-2 text-[11.5px]"
                >
                  {/* Render the URL (sans protocol) rather than the page
                      title so this list never collides with the same
                      day's contact-sheet titles when a test or screen
                      reader walks the DOM. Tooltip carries the title for
                      the few characters where it adds context. */}
                  <span
                    className="truncate font-mono text-[10.5px] text-ink-secondary"
                    title={row.title ?? row.url}
                  >
                    {compactUrl(row.url)}
                  </span>
                  <span className="shrink-0 font-mono text-[9.5px] text-ink-faint tabular-nums">
                    {copy.visitsCountTemplate.replace(
                      '{count}',
                      row.visits.toLocaleString(language),
                    )}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>
    </details>
  )
}

function compactUrl(url: string): string {
  // Drop the http(s):// scheme so the row is dominated by host + path.
  // Anything past the first 60 chars gets ellipsised — most "I keep
  // coming back here" URLs fit comfortably and the longer tail (cache-
  // busting query params, session ids) just creates visual noise.
  const trimmed = url.replace(/^https?:\/\//i, '')
  return trimmed.length > 60 ? `${trimmed.slice(0, 57)}…` : trimmed
}

function formatTime(ms: number, language: string, hour12: boolean): string {
  try {
    return new Date(ms).toLocaleTimeString(language, {
      hour: hour12 ? 'numeric' : '2-digit',
      minute: '2-digit',
      hour12,
    })
  } catch {
    return '--:--'
  }
}

function formatHourOfDay(
  hour: number,
  language: string,
  hour12: boolean,
): string {
  try {
    const today = new Date()
    today.setHours(hour, 0, 0, 0)
    return today.toLocaleTimeString(language, {
      hour: hour12 ? 'numeric' : '2-digit',
      hour12,
    })
  } catch {
    return String(hour)
  }
}

function DayInsightsColumn({
  eyebrow,
  children,
}: {
  eyebrow: string
  children: React.ReactNode
}) {
  return (
    <div>
      <div className="mb-[6px] font-mono text-[9px] uppercase tracking-[0.08em] text-ink-faint">
        {eyebrow}
      </div>
      {children}
    </div>
  )
}

/** Hours a gridline is drawn under; 23 gets a tick label but no gridline (matches the original hand-rolled layout). */
const HOURLY_GRIDLINE_HOURS = [0, 6, 12, 18]
const HOURLY_TICK_HOURS = [0, 6, 12, 18, 23]

/**
 * 24-hour sparkline built on the shared `Sparkline` primitive. Renders a
 * filled area under the line, highlights buckets with > 0 visits as accent
 * dot markers, four gridlines at the 0/6/12/18 hour marks, and labels the
 * 0/6/12/18/23 hour ticks along the bottom. Skips dots that would crowd a
 * tiny chart — only non-empty hours light up.
 */
function HourlySparkline({ insights }: { insights: DayInsights }) {
  const { hourBuckets } = insights
  const markers = hourBuckets
    .map((count, hour) => ({ count, hour }))
    .filter(({ count }) => count > 0)
    .map(({ hour }) => ({ index: hour }))
  return (
    <Sparkline
      values={hourBuckets}
      ariaLabel="24-hour activity"
      width={220}
      height={36}
      paddingX={10}
      gridlines={HOURLY_GRIDLINE_HOURS.map((hour) => ({ index: hour }))}
      markers={markers}
      ticks={HOURLY_TICK_HOURS.map((hour) => ({
        index: hour,
        label: String(hour),
      }))}
    />
  )
}
