/**
 * @file path-flows-section.tsx
 * @description Renders the secondary-grid path-flow card that summarizes repeated browsing sequences.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Load the bounded path-flow overview payload for the current scope and selected step count.
 * - Preserve the existing low-signal hiding rules and per-flow explainability affordance.
 * - Keep path-flow row rendering local so route composition does not absorb sequence-specific UI.
 *
 * ## Non-Responsibilities
 * - Does not own shared route composition or secondary-grid ordering.
 * - Does not define focus routing beyond consuming the provided href builder.
 * - Does not promote path flows into a shared helper without an explicit owner decision.
 *
 * ## Dependencies
 * - `lib/core-intelligence` for typed path-flow data and async loading.
 * - `lib/core-intelligence/api` for deterministic path-flow reads.
 * - `ExplainabilityPanel`, `section-meta`, `section-body`, and local heuristics/shared helpers.
 *
 * ## Performance Notes
 * - Fetches the same capped path-flow payload as the original section and reuses cache peeks.
 * - Filters only the already-bounded result list, so it does not scale with raw archive size.
 */

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../../components/intelligence/explainability-panel'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type PathFlow,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'
import { isMeaningfulPathFlow } from './heuristics'

/**
 * Surfaces repeated multi-step browsing sequences while keeping auth noise and
 * same-site loops out of the secondary grid. The card owns its own step-count
 * state so the route shell only needs to supply scope and navigation grammar.
 *
 * @param dateRange The current intelligence time window.
 * @param focusedDomainHref Builds the existing shared domain deep-link with focus context.
 * @param profileId Optional profile scope; `null` means archive-wide.
 * @param scopeLabel Human-readable scope string for shared evidence metadata.
 * @param t Route-local translator used by the unchanged card copy.
 * @returns The path-flow section, its empty/loading state, or `null` when the ready payload is low-signal.
 */
export function PathFlowsSection({
  dateRange,
  focusedDomainHref,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  focusedDomainHref: (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) => string
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const [stepCount, setStepCount] = useState<number>(3)
  const { data, loading } = useAsyncData(
    () => api.getPathFlows(dateRange, profileId, stepCount, 15),
    [dateRange, profileId, stepCount],
    {
      getCached: () => api.peekPathFlows(dateRange, profileId, stepCount, 15),
    },
  )
  const flows = (data?.data ?? []).filter(isMeaningfulPathFlow)

  if (!loading && flows.length === 0 && data?.meta.state === 'ready') {
    return null
  }

  return (
    <section className="intelligence-section path-flows-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('pathFlowsTitle')}</h2>
        <select
          className="top-sites-controls__sort"
          value={stepCount}
          onChange={(event) => setStepCount(Number(event.target.value))}
          aria-label={t('pathFlowsStepLabel')}
        >
          <option value={2}>{t('pathFlowsStep2')}</option>
          <option value={3}>{t('pathFlowsStep3')}</option>
        </select>
      </div>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : flows.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('pathFlowsEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <ul className="path-flows">
            {flows.map((flow) => (
              <PathFlowRow
                key={flow.flowId}
                focusedDomainHref={focusedDomainHref}
                flow={flow}
                profileId={profileId}
                t={t}
              />
            ))}
          </ul>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function PathFlowRow({
  focusedDomainHref,
  flow,
  profileId,
  t,
}: {
  focusedDomainHref: (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) => string
  flow: PathFlow
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${flow.stepCount}::${flow.flowPattern}`
    : null

  return (
    <li className="path-flow-row">
      <div className="path-flow-row__chips">
        {flow.steps.map((step) => (
          <span
            key={`${flow.flowId}:${step.index}`}
            className="path-flow-row__group"
          >
            {step.registrableDomain ? (
              <Link
                className="path-flow-row__chip intelligence-link"
                to={focusedDomainHref(step.registrableDomain, {
                  focusType: 'path-flow',
                  focusId: flow.flowId,
                })}
              >
                {step.label}
              </Link>
            ) : (
              <span className="path-flow-row__chip">{step.label}</span>
            )}
            {step.index < flow.steps.length - 1 ? (
              <span className="path-flow-row__arrow" aria-hidden="true">
                →
              </span>
            ) : null}
          </span>
        ))}
      </div>
      <span className="path-flow-row__count">
        {t('pathFlowsOccurrences', { count: flow.occurrenceCount })}
      </span>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="path_flow"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}
