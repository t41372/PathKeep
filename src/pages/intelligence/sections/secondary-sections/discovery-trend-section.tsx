/**
 * @file discovery-trend-section.tsx
 * @description `/intelligence` secondary grid里的 Discovery Trend 周趋势卡片。
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - 读取 weekly discovery trend 并渲染最近几周的发现率走势。
 * - 复用现有 week-label heuristic，保证显示文案与其他 section 保持一致。
 * - 在没有趋势点时退化为隐藏或空状态，而不是输出原始统计噪音。
 *
 * ## Non-Responsibilities
 * - 不负责 route scope、query grammar 或 day-level rhythm 交互。
 * - 不负责缓存策略定义；这里只消费 overview API 的 cached read contract。
 * - 不负责新增 discovery 指标或解释逻辑。
 *
 * ## Dependencies
 * - `lib/core-intelligence/api` 提供 weekly discovery trend 读取。
 * - `./heuristics` 提供 ISO week -> localized label 的既有解释规则。
 * - `section-body` / `section-meta` 保持 shared Intelligence card chrome 一致。
 *
 * ## Performance Notes
 * - 只展示最近 6 个趋势点，避免次级卡片在大范围时间窗里无限增长。
 * - 所有运算都基于已经聚合好的 weekly points，不在前端重算原始访问数据。
 */

import { useMemo } from 'react'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import { useAsyncData, type DateRange } from '../../../../lib/core-intelligence'
import type { DiscoveryTrendPoint } from '../../../../lib/core-intelligence/types-analysis'
import { buildSparklinePath } from './discovery-trend-helpers'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'
import { humanizeDiscoveryWeekLabel } from './heuristics'

type DiscoveryTrendSectionProps = {
  dateRange: DateRange
  profileId: string | null
  scopeLabel: string
  t: T
}

/**
 * Summarizes how much of the user's browsing scope was spent discovering new
 * domains, without forcing the route shell to know weekly chart details.
 *
 * @param dateRange The active Intelligence date range used for discovery-trend reads.
 * @param profileId Optional profile scope used by the deterministic overview cache.
 * @param scopeLabel Human-readable scope text rendered with freshness metadata.
 * @param t Route-local translator for the existing discovery trend copy.
 * @returns The discovery-trend section, a loading/empty state, or `null` once the ready payload has no trend points worth showing.
 *
 * Edge cases:
 * - Hides the entire card when the ready payload is empty so low-signal trends do not crowd the secondary grid.
 * - Keeps the existing tooltip/title string exactly intact for parity with the current UI copy contract.
 */
export function DiscoveryTrendSection({
  dateRange,
  profileId,
  scopeLabel,
  t,
}: DiscoveryTrendSectionProps) {
  const { data, loading } = useAsyncData(
    () => api.getDiscoveryTrend(dateRange, profileId, 'week'),
    [dateRange, profileId],
    {
      getCached: () => api.peekDiscoveryTrend(dateRange, profileId, 'week'),
    },
  )
  const trend = data?.data ?? null
  if (
    !loading &&
    (!trend || trend.points.length === 0) &&
    data?.meta.state === 'ready'
  ) {
    return null
  }
  const visiblePoints = trend ? [...trend.points].slice(-6).reverse() : []
  const chronologicalPoints = trend ? [...trend.points].slice(-6) : []

  return (
    <section className="intelligence-section discovery-trend-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('discoveryTrendTitle')}
        </h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      <p className="intelligence-section__help">{t('discoveryTrendHelp')}</p>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !trend || trend.points.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('discoveryTrendEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="discovery-trend">
          {chronologicalPoints.length >= 2 ? (
            <DiscoverySparkline points={chronologicalPoints} t={t} />
          ) : null}
          {visiblePoints.map((point) => {
            const ratePercent = Math.round(point.discoveryRate * 100)

            return (
              <div
                key={point.dateKey}
                className="discovery-trend__row"
                title={`${humanizeDiscoveryWeekLabel(point.dateKey, t)}: ${ratePercent}% · ${point.newDomainCount} ${t('discoveryTrendNewDomains')} · ${point.totalVisits} ${t('visits')}`}
              >
                <div className="discovery-trend__row-header">
                  <span className="discovery-trend__date-label">
                    {humanizeDiscoveryWeekLabel(point.dateKey, t)}
                  </span>
                  <span className="discovery-trend__rate">
                    {t('discoveryTrendRatePercent', {
                      count: ratePercent,
                    })}
                  </span>
                </div>
                <span className="discovery-trend__bar">
                  <span
                    className="discovery-trend__bar-fill"
                    style={{ width: `${Math.max(ratePercent, 2)}%` }}
                  />
                </span>
                <div className="discovery-trend__stats">
                  <span className="discovery-trend__stat">
                    {t('discoveryTrendDomainsLabel')}: {point.newDomainCount}
                  </span>
                  <span className="discovery-trend__stat">
                    {t('discoveryTrendVisitsLabel', {
                      count: point.totalVisits,
                    })}
                  </span>
                </div>
              </div>
            )
          })}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function DiscoverySparkline({
  points,
  t,
}: {
  points: DiscoveryTrendPoint[]
  t: T
}) {
  const viewWidth = 200
  const viewHeight = 48
  const pad = 4

  const polylinePoints = useMemo(
    () => buildSparklinePath(points, viewWidth, viewHeight, pad),
    [points],
  )

  const meanRate = useMemo(() => {
    const sum = points.reduce((acc, p) => acc + p.discoveryRate, 0)
    return sum / points.length
  }, [points])

  const maxRate = useMemo(
    () => Math.max(...points.map((p) => p.discoveryRate), 0.01),
    [points],
  )

  const meanY =
    pad + (viewHeight - pad * 2) - (meanRate / maxRate) * (viewHeight - pad * 2)

  const areaPath = useMemo(() => {
    /* v8 ignore next -- DiscoverySparkline only mounts when chronologicalPoints.length >= 2; this mirrors buildSparklinePath's guard for direct/defensive use. */
    if (points.length < 2) return ''
    const innerWidth = viewWidth - pad * 2
    const innerHeight = viewHeight - pad * 2
    const coords = points.map((point, index) => {
      const x = pad + (index / (points.length - 1)) * innerWidth
      const y =
        pad + innerHeight - (point.discoveryRate / maxRate) * innerHeight
      return `${x.toFixed(1)},${y.toFixed(1)}`
    })
    const bottomRight = `${(pad + innerWidth).toFixed(1)},${(pad + innerHeight).toFixed(1)}`
    const bottomLeft = `${pad.toFixed(1)},${(pad + innerHeight).toFixed(1)}`
    return `M${coords[0]} L${coords.join(' L')} L${bottomRight} L${bottomLeft} Z`
  }, [points, maxRate])

  return (
    <svg
      className="discovery-trend__sparkline"
      viewBox={`0 0 ${viewWidth} ${viewHeight}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={t('discoveryTrendSparklineLabel')}
      data-testid="discovery-sparkline"
    >
      <title>
        {t('discoveryTrendMeanLabel', {
          percent: Math.round(meanRate * 100),
        })}
      </title>
      <path d={areaPath} fill="var(--accent)" opacity="0.08" />
      <polyline
        points={polylinePoints}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1.5"
        vectorEffect="non-scaling-stroke"
      />
      <line
        x1={pad}
        y1={meanY}
        x2={viewWidth - pad}
        y2={meanY}
        stroke="var(--accent)"
        strokeWidth="0.75"
        strokeDasharray="4 3"
        opacity="0.5"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  )
}
