/**
 * Shared refind workbench surfaces used by overview, day-insights, and the dedicated refind route.
 *
 * Why this file exists:
 * - M10 consolidates refind title/description/action/factor chrome so multiple
 *   consumers stop re-implementing the same entity shell.
 */

import { useId, useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import {
  InsightEntityActions,
  type InsightEntityActionLink,
} from '../entity-actions'

export interface RefindWorkbenchFactor {
  label: string
  valueLabel: ReactNode
  emphasis?: number | null
}

export function RefindFactorList({
  factors,
}: {
  factors: RefindWorkbenchFactor[]
}) {
  const maxEmphasis = Math.max(
    ...factors.map((factor) => Math.max(factor.emphasis ?? 0, 0)),
    1,
  )

  return (
    <div className="refind-card__factors">
      {factors.map((factor) => {
        const barWidth =
          factor.emphasis && factor.emphasis > 0
            ? `${Math.round((factor.emphasis / maxEmphasis) * 80)}px`
            : null

        return (
          <div key={factor.label} className="refind-card__factor">
            <span className="refind-card__factor-label">{factor.label}</span>
            {barWidth ? (
              <span
                className="refind-card__factor-bar"
                style={{ width: barWidth }}
              />
            ) : null}
            <span className="refind-card__factor-value">
              {factor.valueLabel}
            </span>
          </div>
        )
      })}
    </div>
  )
}

export function RefindSummaryCard({
  actionItems,
  className = 'refind-card',
  description,
  explainability,
  expandLabel,
  factorRows,
  scoreLabel,
  title,
  titleHref,
}: {
  actionItems: InsightEntityActionLink[]
  className?: string
  description: string
  explainability?: ReactNode
  expandLabel?: string
  factorRows?: RefindWorkbenchFactor[]
  scoreLabel?: string
  title: string
  titleHref: string
}) {
  const factorPanelId = useId()
  const [showFactors, setShowFactors] = useState(false)
  const hasFactors = Boolean(factorRows?.length)

  return (
    <div className={className}>
      <div className="refind-card__header">
        <span className="refind-card__icon">📄</span>
        <Link className="refind-card__title" to={titleHref}>
          {title}
        </Link>
      </div>
      <p className="refind-card__description">{description}</p>
      {hasFactors && expandLabel && scoreLabel ? (
        <button
          aria-controls={factorPanelId}
          aria-expanded={showFactors}
          className="refind-card__expand-toggle"
          type="button"
          onClick={() => setShowFactors((value) => !value)}
        >
          <span>{showFactors ? '▾' : '▸'}</span>
          <span>{expandLabel}</span>
          <span style={{ marginLeft: 'auto', fontFamily: 'var(--font-code)' }}>
            {scoreLabel}
          </span>
        </button>
      ) : null}
      <InsightEntityActions className="intelligence-actions" items={actionItems} />
      {hasFactors && showFactors ? (
        <div id={factorPanelId}>
          <RefindFactorList factors={factorRows ?? []} />
        </div>
      ) : null}
      {explainability}
    </div>
  )
}
