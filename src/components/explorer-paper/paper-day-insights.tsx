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
 * ## Performance notes
 * - The component is a leaf node so React can skip subtree reconciliation
 *   when the parent re-renders without changing `insights`. Callers
 *   wrap the aggregator in `useMemo` keyed on the `PaperDay` reference.
 */

import { cn } from '@/lib/cn'
import type { DayInsights } from './paper-day-insights-helpers'

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
}

export interface PaperDayInsightsProps {
  insights: DayInsights
  copy: PaperDayInsightsCopy
  /** Optional language tag for compact-number formatting. */
  language?: string
  className?: string
  testId?: string
}

export function PaperDayInsights({
  insights,
  copy,
  language = 'en',
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
                      width: `${(row.visits / Math.max(1, insights.topDomains[0]?.visits ?? 1)) * 100}%`,
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
    </div>
  )
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

/**
 * Inline-SVG 24-hour sparkline. Renders a filled polygon under the line,
 * highlights buckets with > 0 visits as accent dots, and labels the 0 / 6 /
 * 12 / 18 / 23 hour ticks along the bottom. Skips bucket dots that would
 * crowd a tiny chart — only non-empty hours light up.
 */
function HourlySparkline({ insights }: { insights: DayInsights }) {
  const { hourBuckets, hourPeak } = insights
  const W = 220
  const H = 36
  const pL = 10
  const pR = 10
  const pY = 4
  const iW = W - pL - pR
  const points: string[] = []
  for (let hour = 0; hour < 24; hour += 1) {
    const x = pL + (hour / 23) * iW
    const y = pY + (1 - hourBuckets[hour] / hourPeak) * (H - pY * 2)
    points.push(`${x.toFixed(1)},${y.toFixed(1)}`)
  }
  const fill = [`${pL},${H}`, ...points, `${pL + iW},${H}`].join(' ')
  return (
    <svg
      role="img"
      aria-label="24-hour activity"
      width="100%"
      viewBox={`0 0 ${W} ${H + 12}`}
      style={{ display: 'block', overflow: 'visible' }}
    >
      {[0, 6, 12, 18].map((hour) => (
        <line
          key={hour}
          x1={pL + (hour / 23) * iW}
          y1={0}
          x2={pL + (hour / 23) * iW}
          y2={H}
          stroke="var(--border-light)"
          strokeWidth="0.5"
        />
      ))}
      <polygon points={fill} fill="var(--accent)" opacity="0.1" />
      <polyline
        points={points.join(' ')}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {hourBuckets.map((count, hour) =>
        count > 0 ? (
          <circle
            key={hour}
            cx={pL + (hour / 23) * iW}
            cy={pY + (1 - count / hourPeak) * (H - pY * 2)}
            r="2"
            fill="var(--accent)"
          />
        ) : null,
      )}
      {[0, 6, 12, 18, 23].map((hour) => (
        <text
          key={hour}
          x={pL + (hour / 23) * iW}
          y={H + 10}
          textAnchor="middle"
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: 7.5,
            fill: 'var(--ink-faint)',
          }}
        >
          {hour}
        </text>
      ))}
    </svg>
  )
}
