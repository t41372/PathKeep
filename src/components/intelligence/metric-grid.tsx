import type { ReactNode } from 'react'
import type { KpiMetric } from '../../lib/core-intelligence'

type Translate = (key: string, vars?: Record<string, string | number>) => string

export interface IntelligenceMetricItem {
  icon?: ReactNode
  label: string
  value: ReactNode
  trend?: KpiMetric | null
}

function TrendBadge({ metric, t }: { metric: KpiMetric; t: Translate }) {
  if (metric.changePercent == null) return null
  const arrow =
    metric.trend === 'up' ? '↑' : metric.trend === 'down' ? '↓' : '='
  const sign = metric.changePercent > 0 ? '+' : ''
  return (
    <span
      className={`trend-badge trend-badge--${metric.trend}`}
      aria-label={t('trendLabel', {
        direction: metric.trend,
        percent: Math.abs(metric.changePercent),
      })}
    >
      {sign}
      {Math.round(metric.changePercent)}% {arrow}
    </span>
  )
}

export function IntelligenceMetricGrid({
  className = 'digest-cards',
  items,
  t,
}: {
  className?: string
  items: IntelligenceMetricItem[]
  t?: Translate
}) {
  return (
    <div className={className}>
      {items.map((item) => (
        <div key={item.label} className="digest-card">
          {item.icon ? (
            <span className="digest-card__icon">{item.icon}</span>
          ) : null}
          <span className="digest-card__value">{item.value}</span>
          <span className="digest-card__label">{item.label}</span>
          {item.trend && t ? <TrendBadge metric={item.trend} t={t} /> : null}
        </div>
      ))}
    </div>
  )
}
