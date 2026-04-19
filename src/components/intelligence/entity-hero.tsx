/**
 * Shared hero chrome for route-first intelligence entities.
 *
 * Why this file exists:
 * - Day, domain, query-family, refind, session, and trail routes all need the
 *   same back/title/subtitle/action structure.
 * - Reusing one hero component keeps entity promotion focused on navigation and
 *   read-model truth instead of repeated route-local chrome.
 */

import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'

export function InsightEntityHero({
  actions,
  backHref,
  backLabel,
  eyebrow,
  subtitle,
  title,
}: {
  actions?: ReactNode
  backHref: string
  backLabel: string
  eyebrow: string
  subtitle: string
  title: ReactNode
}) {
  return (
    <div className="day-insights__hero">
      <div className="day-insights__hero-copy">
        <Link className="btn-secondary" to={backHref}>
          ← {backLabel}
        </Link>
        <span className="mono-kicker">{eyebrow}</span>
        <h1 className="day-insights__title">{title}</h1>
        <p className="day-insights__subtitle">{subtitle}</p>
      </div>
      {actions ? <div className="day-insights__actions">{actions}</div> : null}
    </div>
  )
}
