/**
 * Navigation Tracer panel — shows "How did you get here?" for a selected visit.
 *
 * Why this file exists:
 * - Part of Core Intelligence P1-3b Navigation Path Tracer.
 * - Renders the from_visit chain as a visual path, showing how the user arrived at a page.
 * - On-demand: fetches the navigation path when a user expands a history record.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md` §3.3
 */

import { useState } from 'react'
import * as api from '../../../lib/core-intelligence/api'
import type { NavigationPathStep } from '../../../lib/core-intelligence/types'
import { describeError } from '../../../lib/errors'
import { sanitizeExplorerDisplayText } from '../helpers'
import type { Translator } from '../types'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface NavigationTracerProps {
  visitId: number
  intelligenceT: Translator
  onSelectVisitUrl?: (url: string) => void
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function NavigationTracer({
  visitId,
  intelligenceT,
  onSelectVisitUrl,
}: NavigationTracerProps) {
  const [steps, setSteps] = useState<NavigationPathStep[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [expanded, setExpanded] = useState(false)

  const handleToggle = async () => {
    const willExpand = !expanded
    setExpanded(willExpand)
    if (willExpand && !steps) {
      setLoading(true)
      setError(null)
      try {
        const result = await api.getNavigationPath(visitId)
        setSteps(result.steps)
      } catch (err) {
        setError(describeError(err, 'get_navigation_path'))
      } finally {
        setLoading(false)
      }
    }
  }

  return (
    <div className="navigation-tracer">
      <button
        className="navigation-tracer__toggle"
        type="button"
        aria-expanded={expanded}
        onClick={() => void handleToggle()}
      >
        <span className="navigation-tracer__toggle-icon" aria-hidden="true">
          {expanded ? '▾' : '▸'}
        </span>
        <span className="navigation-tracer__toggle-label">
          {intelligenceT('tracerTitle')}
        </span>
      </button>

      {expanded && (
        <div className="navigation-tracer__body">
          {loading ? (
            <div
              className="intelligence-skeleton intelligence-skeleton--card"
              style={{ height: 80 }}
            />
          ) : error ? (
            <p className="intelligence-empty__text">{error}</p>
          ) : steps && steps.length > 0 ? (
            <div className="navigation-tracer__path">
              {steps.map((step, i) => {
                const isLast = i === steps.length - 1
                return (
                  <div
                    key={step.visitId}
                    className={`navigation-tracer__step${isLast ? ' navigation-tracer__step--current' : ''}`}
                  >
                    <span className="navigation-tracer__depth">
                      {'  '.repeat(step.depth)}
                      {step.depth > 0 ? '→ ' : ''}
                    </span>
                    <span
                      className="navigation-tracer__step-content"
                      role="button"
                      tabIndex={0}
                      onClick={() => onSelectVisitUrl?.(step.url)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault()
                          onSelectVisitUrl?.(step.url)
                        }
                      }}
                    >
                      {sanitizeExplorerDisplayText(step.title || step.url)}
                    </span>
                    {isLast && (
                      <span className="navigation-tracer__here-badge">
                        ← {intelligenceT('tracerHere')}
                      </span>
                    )}
                  </div>
                )
              })}
            </div>
          ) : (
            <p className="intelligence-empty__text">
              {intelligenceT('tracerEmpty')}
            </p>
          )}
        </div>
      )}
    </div>
  )
}
