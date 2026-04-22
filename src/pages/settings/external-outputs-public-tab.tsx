/**
 * @file external-outputs-public-tab.tsx
 * @description Renders the redacted public snapshot review tab inside Settings external outputs.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Render the public snapshot preview with its redaction callout.
 * - Keep domain/search/discovery drilldown links and JSON export affordances tied to the public payload.
 *
 * ## Not responsible for
 * - Fetching payloads or managing the active tab state
 * - Rendering embed/widget snapshot previews
 *
 * ## Dependencies
 * - Depends on intelligence metric/review primitives plus shared href helpers for day/domain drilldowns.
 *
 * ## Performance notes
 * - Pure render component; public snapshot lists are bounded by backend payload caps and should stay render-only.
 */

import { Link } from 'react-router-dom'
import { IntelligenceMetricGrid } from '../../components/intelligence/metric-grid'
import {
  ReviewCodePreview,
  ReviewSection,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import type { IntelligencePublicSnapshot } from '../../lib/core-intelligence'
import {
  dayInsightsHref,
  domainInsightsHref,
} from '../../lib/core-intelligence/routes'
import { formatDateTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'
import {
  buildDigestMetricItems,
  type Translate,
} from './external-outputs-shared'

interface ExternalOutputsPublicTabProps {
  activeProfileId: string | null
  copyFeedback: ReviewCopyFeedback | null
  copyLabel: string
  commonT: Translate
  json: string
  language: ResolvedLanguage
  onCopy: (key: string, payload: string) => void | Promise<void>
  snapshot: IntelligencePublicSnapshot
  t: Translate
  intelligenceT: Translate
}

/**
 * Keeps the redacted public snapshot preview isolated so public-boundary copy and links can evolve independently.
 */
export function ExternalOutputsPublicTab({
  activeProfileId,
  copyFeedback,
  copyLabel,
  commonT,
  json,
  language,
  onCopy,
  snapshot,
  t,
  intelligenceT,
}: ExternalOutputsPublicTabProps) {
  return (
    <>
      <ReviewSection
        headerMeta={
          <span className="mono">
            {formatDateTime(snapshot.generatedAt, language) ??
              snapshot.generatedAt}
          </span>
        }
        title={t('externalOutputsPublicPreviewTitle')}
      >
        <StatusCallout
          tone="info"
          title={t('externalOutputsPublicRedactedTitle')}
          body={t('externalOutputsPublicRedactedBody')}
        />

        <p className="dashboard-next-action">
          {t('externalOutputsWindowLabel', {
            start: snapshot.dateRange.start,
            end: snapshot.dateRange.end,
          })}
        </p>

        <IntelligenceMetricGrid
          className="digest-cards settings-output-digest-grid"
          items={buildDigestMetricItems(
            snapshot.digestSummary,
            language,
            intelligenceT,
          )}
        />

        <div className="settings-field-grid">
          <div className="result-row result-row--active">
            <div className="result-row__header">
              <strong>{t('externalOutputsTopDomains')}</strong>
            </div>
            <div className="settings-output-chip-list">
              {snapshot.topDomains.map((domain) => (
                <Link
                  key={domain}
                  className="chip-button"
                  to={domainInsightsHref({
                    domain,
                    dateRange: snapshot.dateRange,
                    preset: 'custom',
                    profileId: activeProfileId,
                  })}
                >
                  {domain}
                </Link>
              ))}
            </div>
          </div>

          <div className="result-row result-row--active">
            <div className="result-row__header">
              <strong>{t('externalOutputsSearchEngines')}</strong>
            </div>
            {snapshot.searchEngines.length > 0 ? (
              snapshot.searchEngines.map((engine) => (
                <div key={engine.searchEngine} className="config-row">
                  <span className="config-label">
                    {engine.displayName ?? engine.searchEngine}
                  </span>
                  <span className="config-value mono">
                    {engine.searchCount.toLocaleString(language)}
                  </span>
                </div>
              ))
            ) : (
              <p>{t('externalOutputsNoSearchEngines')}</p>
            )}
          </div>

          <div className="result-row result-row--active">
            <div className="result-row__header">
              <strong>{t('externalOutputsDiscoveryTrend')}</strong>
            </div>
            {snapshot.discoveryTrend.points.length > 0 ? (
              snapshot.discoveryTrend.points.map((point) => (
                <div key={point.dateKey} className="config-row">
                  <Link
                    className="config-label mono intelligence-link"
                    to={dayInsightsHref(point.dateKey, activeProfileId)}
                  >
                    {point.dateKey}
                  </Link>
                  <span className="config-value mono">
                    {point.discoveryRate.toFixed(2)}
                  </span>
                </div>
              ))
            ) : (
              <p>{t('externalOutputsNoDiscoveryTrend')}</p>
            )}
          </div>
        </div>

        {snapshot.notes.length > 0 ? (
          <div className="inline-note-list">
            {snapshot.notes.map((note) => (
              <p key={note}>{note}</p>
            ))}
          </div>
        ) : null}
      </ReviewSection>

      <ReviewCodePreview
        copyFeedback={copyFeedback}
        copyKey="public"
        copyLabel={copyLabel}
        code={json}
        errorMessage={t('externalOutputsCopyFailed')}
        onCopy={onCopy}
        successMessage={commonT('copiedNotice')}
        title={t('externalOutputsJsonTitle')}
      />
    </>
  )
}
