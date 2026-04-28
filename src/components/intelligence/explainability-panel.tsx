/**
 * Explainability Panel — reusable component for displaying how intelligence decisions were made.
 *
 * Why this file exists:
 * - Part of Core Intelligence P1-10a Explainability Panel.
 * - Shows trigger rules, score factor breakdown, and linked visit IDs.
 * - Used by Refind Pages, Sessions, Trails, and other intelligence features.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md` §4.A
 */

import { useState } from 'react'
import * as api from '../../lib/core-intelligence/api'
import type { Explanation } from '../../lib/core-intelligence/types'

import './explainability-panel.css'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface ExplainabilityPanelProps {
  entityType: string
  entityId: string
  /** Pre-loaded explanation. If provided, skips the IPC fetch. */
  explanation?: Explanation | null
  /** i18n translator for the intelligence namespace */
  t: (key: string, vars?: Record<string, string | number>) => string
}

function localizeExplainabilityFactorLabel(
  label: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
) {
  const keyMap: Record<string, string> = {
    cross_day_count: 'explainFactorCrossDayCount',
    trail_count: 'explainFactorTrailCount',
    search_arrival_count: 'explainFactorSearchArrivalCount',
    typed_revisit_count: 'explainFactorTypedRevisitCount',
    visit_count: 'explainFactorVisitCount',
    search_count: 'explainFactorSearchCount',
    unique_domain_count: 'explainFactorUniqueDomainCount',
    navigation_chain_depth: 'explainFactorNavigationChainDepth',
    duration_minutes: 'explainFactorDurationMinutes',
    reformulation_count: 'explainFactorReformulationCount',
    max_depth: 'explainFactorMaxDepth',
    landing_detected: 'explainFactorLandingDetected',
    member_count: 'explainFactorMemberCount',
    distinct_query_count: 'explainFactorDistinctQueryCount',
    occurrence_count: 'explainFactorOccurrenceCount',
    distinct_days: 'explainFactorDistinctDays',
    domain_count: 'explainFactorDomainCount',
    mean_interval_days: 'explainFactorMeanIntervalDays',
    page_count: 'explainFactorPageCount',
    coefficient_of_variation: 'explainFactorCoefficientOfVariation',
    interrupted: 'explainFactorInterrupted',
    step_count: 'explainFactorStepCount',
    alternation_count: 'explainFactorAlternationCount',
  }

  return keyMap[label] ? t(keyMap[label]) : label.replaceAll('_', ' ')
}

function localizeHabitType(
  habitType: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
) {
  const keyMap: Record<string, string> = {
    daily_habit: 'habitType_daily_habit',
    weekly_habit: 'habitType_weekly_habit',
    periodic_reference: 'habitType_periodic_reference',
  }

  return keyMap[habitType]
    ? t(keyMap[habitType])
    : habitType.replaceAll('_', ' ')
}

function localizeTriggerRule(
  rule: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
) {
  const refindMatch = /^Refind score >= ([\d.]+)$/.exec(rule)
  if (refindMatch) {
    return t('explainRuleRefindScore', {
      score: refindMatch[1],
    })
  }

  if (
    rule ===
    'Deep dive session matched the navigation-depth, domain-count, and visit-count thresholds.'
  ) {
    return t('explainRuleSessionDeepDive')
  }

  if (
    rule ===
    'Visits were grouped into one session because adjacent gaps stayed within 30 minutes.'
  ) {
    return t('explainRuleSessionGap')
  }

  const searchTrailMatch =
    /^Search trail anchored by '(.+)' and extended through navigation ancestry within the session window\.$/.exec(
      rule,
    )
  if (searchTrailMatch) {
    return t('explainRuleSearchTrail', {
      query: searchTrailMatch[1],
    })
  }

  const queryFamilyMatch =
    /^Queries were merged into one family because their Jaccard or containment similarity matched '(.+)'\.$/.exec(
      rule,
    )
  if (queryFamilyMatch) {
    return t('explainRuleQueryFamily', {
      query: queryFamilyMatch[1],
    })
  }

  if (
    rule ===
    'This investigation reopened because the same anchor reappeared across distinct days or repeated deterministic evidence.'
  ) {
    return t('explainRuleReopenedInvestigation')
  }

  // Stryker disable Regex: the leading anchor is equivalent here because the captured habit name intentionally accepts custom text before " cadence".
  const interruptedHabitMatch =
    /^(.+) cadence was detected and later crossed its interruption threshold\.$/.exec(
      rule,
    )
  // Stryker restore Regex
  if (interruptedHabitMatch) {
    return t('explainRuleHabitPatternInterrupted', {
      habit: localizeHabitType(interruptedHabitMatch[1], t),
    })
  }

  // Stryker disable Regex: the leading anchor is equivalent here because the captured habit name intentionally accepts custom text before " cadence".
  const habitMatch =
    /^(.+) cadence was detected from repeated cross-day visits\.$/.exec(rule)
  // Stryker restore Regex
  if (habitMatch) {
    return t('explainRuleHabitPattern', {
      habit: localizeHabitType(habitMatch[1], t),
    })
  }

  if (
    rule === 'This flow pattern recurs across session-local domain n-grams.'
  ) {
    return t('explainRulePathFlow')
  }

  if (
    rule ===
    'This compare set alternated between multiple comparable pages within one search trail.'
  ) {
    return t('explainRuleCompareSet')
  }

  return rule
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ExplainabilityPanel({
  entityType,
  entityId,
  explanation: preloaded,
  t,
}: ExplainabilityPanelProps) {
  const [expanded, setExpanded] = useState(false)
  const [data, setData] = useState<Explanation | null>(preloaded ?? null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleToggle = async () => {
    const willExpand = !expanded
    setExpanded(willExpand)
    if (willExpand && !data) {
      setLoading(true)
      setError(null)
      try {
        const result = await api.explainEntity(entityType, entityId)
        setData(result)
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err)
        setError(msg)
      } finally {
        setLoading(false)
      }
    }
  }

  return (
    <div className="explainability-panel">
      <button
        className="explainability-panel__toggle"
        type="button"
        aria-expanded={expanded}
        onClick={() => void handleToggle()}
      >
        <span className="explainability-panel__toggle-icon" aria-hidden="true">
          {expanded ? '▾' : '▸'}
        </span>
        <span className="explainability-panel__toggle-label">
          {t('explainTitle')}
        </span>
      </button>

      {expanded && (
        <div className="explainability-panel__body">
          {loading ? (
            <div
              className="intelligence-skeleton intelligence-skeleton--card"
              style={{ height: 100 }}
            />
          ) : error ? (
            <p className="explainability-panel__error">{error}</p>
          ) : data ? (
            <>
              {/* Trigger rule */}
              <div className="explainability-panel__rule">
                <span className="explainability-panel__rule-label">
                  {t('explainRule')}
                </span>
                <span className="explainability-panel__rule-value">
                  {localizeTriggerRule(data.triggerRule, t)}
                </span>
              </div>

              {/* Factor breakdown */}
              {data.factors.length > 0 && (
                <div className="explainability-panel__factors">
                  <span className="explainability-panel__factors-label">
                    {t('explainFactors')}
                  </span>
                  <table className="explainability-panel__factors-table">
                    <thead>
                      <tr>
                        <th>{t('explainFactorName')}</th>
                        <th>{t('explainFactorRaw')}</th>
                        <th>{t('explainFactorWeight')}</th>
                        <th>{t('explainFactorContribution')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.factors.map((factor, i) => (
                        <tr key={i}>
                          <td className="explainability-panel__factor-label">
                            {localizeExplainabilityFactorLabel(factor.label, t)}
                          </td>
                          <td className="explainability-panel__factor-value">
                            {factor.rawValue}
                          </td>
                          <td className="explainability-panel__factor-value">
                            ×{factor.weight}
                          </td>
                          <td className="explainability-panel__factor-value">
                            <FactorBar
                              value={factor.contribution}
                              maxValue={Math.max(
                                ...data.factors.map((f) => f.contribution),
                              )}
                            />
                            {factor.contribution.toFixed(1)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              {/* Participating visit IDs */}
              {data.participatingVisitIds.length > 0 && (
                <div className="explainability-panel__visits">
                  <span className="explainability-panel__visits-label">
                    {t('explainVisits', {
                      count: data.participatingVisitIds.length,
                    })}
                  </span>
                  <div className="explainability-panel__visit-ids">
                    {data.participatingVisitIds.slice(0, 20).map((id) => (
                      <span key={id} className="explainability-panel__visit-id">
                        #{id}
                      </span>
                    ))}
                    {data.participatingVisitIds.length > 20 && (
                      <span className="explainability-panel__visit-id">
                        +{data.participatingVisitIds.length - 20}
                      </span>
                    )}
                  </div>
                </div>
              )}
            </>
          ) : (
            <p className="explainability-panel__error">
              {t('explainUnavailable')}
            </p>
          )}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Factor bar mini-visualization
// ---------------------------------------------------------------------------

function FactorBar({ value, maxValue }: { value: number; maxValue: number }) {
  const pct = maxValue > 0 ? Math.round((value / maxValue) * 100) : 0
  return (
    <span
      className="explainability-panel__factor-bar"
      style={{ width: `${pct}%` }}
      aria-hidden
    />
  )
}
