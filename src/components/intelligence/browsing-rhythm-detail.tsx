import './browsing-rhythm-detail.css'

import { useMemo } from 'react'
import type {
  ActivityMix,
  DayInsightsHourlyBucket,
} from '../../lib/core-intelligence'
import { localeTag, type ResolvedLanguage } from '../../lib/i18n'

type Translate = (key: string, vars?: Record<string, string | number>) => string

export function RhythmHourStrip({
  date,
  hourly,
  t,
}: {
  date: string
  hourly: DayInsightsHourlyBucket[]
  t: Translate
}) {
  const normalized = useMemo(() => {
    const byHour = new Map(
      hourly.map((bucket) => [bucket.hour, bucket.visitCount]),
    )
    return Array.from({ length: 24 }, (_, hour) => ({
      hour,
      visitCount: byHour.get(hour) ?? 0,
    }))
  }, [hourly])
  const maxHourlyCount = Math.max(
    ...normalized.map((bucket) => bucket.visitCount),
    1,
  )

  return (
    <div
      className="rhythm-distribution"
      data-testid="rhythm-hour-strip"
      role="img"
      aria-label={t('rhythmHourStripLabel', { date })}
    >
      <div className="rhythm-distribution__grid">
        {normalized.map((bucket) => (
          <span
            key={bucket.hour}
            className="rhythm-distribution__cell"
            data-level={heatLevel(bucket.visitCount, maxHourlyCount)}
            title={t('rhythmHourTooltip', {
              hour: formatHourRange(bucket.hour),
              count: bucket.visitCount,
            })}
          />
        ))}
      </div>
      <div className="rhythm-distribution__labels" aria-hidden="true">
        {[0, 6, 12, 18, 23].map((hour) => (
          <span key={hour}>{hour}</span>
        ))}
      </div>
    </div>
  )
}

export function RhythmActivityProportionBar({
  categories,
  categoryLabel,
  language,
  t,
}: {
  categories: ActivityMix['categories']
  categoryLabel: (domainCategory: string) => string
  language: ResolvedLanguage
  t: Translate
}) {
  const numberFormatter = useMemo(
    () => new Intl.NumberFormat(localeTag(language)),
    // Stryker disable next-line ArrayDeclaration: supported en/zh locales render the integer visit counts in this component identically.
    [language],
  )
  const sortedCategories = useMemo(
    () => [...categories].sort((left, right) => right.share - left.share),
    [categories],
  )

  return (
    <div className="rhythm-proportion" data-testid="rhythm-activity-proportion">
      <div className="rhythm-proportion__bar" aria-hidden="true">
        {sortedCategories.map((category) => (
          <span
            key={category.domainCategory}
            className="rhythm-proportion__segment"
            data-category={category.domainCategory}
            style={{ width: `${category.share * 100}%` }}
          />
        ))}
      </div>
      <div className="rhythm-proportion__legend">
        {sortedCategories.map((category) => {
          const sharePercent = Math.round(category.share * 100)

          return (
            <div
              key={category.domainCategory}
              className="rhythm-proportion__legend-row"
            >
              <span className="rhythm-proportion__legend-main">
                <span
                  className="rhythm-proportion__legend-swatch rhythm-proportion__segment"
                  data-category={category.domainCategory}
                />
                <span className="rhythm-proportion__legend-label">
                  {categoryLabel(category.domainCategory)}
                </span>
              </span>
              <span className="rhythm-proportion__legend-share">
                {sharePercent}%
              </span>
              <span className="rhythm-proportion__legend-count">
                {numberFormatter.format(category.visitCount)} {t('visits')}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function heatLevel(count: number, maxCount: number) {
  // Stryker disable next-line ConditionalExpression,EqualityOperator: public callers clamp maxCount to at least 1 before this helper runs.
  if (count <= 0 || maxCount <= 0) {
    return 0
  }

  const ratio = count / maxCount
  if (ratio >= 0.75) return 4
  if (ratio >= 0.5) return 3
  if (ratio >= 0.25) return 2
  return 1
}

function formatHourRange(hour: number) {
  const start = String(hour).padStart(2, '0')
  const end = String((hour + 1) % 24).padStart(2, '0')
  return `${start}:00-${end}:00`
}
