/**
 * @file external-outputs-widget-tab.tsx
 * @description Renders the widget snapshot review tab inside Settings external outputs.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Render the widget snapshot digest and highlight preview.
 * - Keep trusted-only warnings and JSON export affordances tied to the widget payload.
 *
 * ## Not responsible for
 * - Fetching payloads or managing the active tab state
 * - Rendering embed/public snapshot previews
 *
 * ## Dependencies
 * - Depends on intelligence metric/review primitives, the shared digest helper, and target-link renderer.
 *
 * ## Performance notes
 * - Pure render component; digest rows are derived from one shared helper to avoid duplicate work across tabs.
 */

import { IntelligenceMetricGrid } from '../../components/intelligence/metric-grid'
import {
  ReviewCodePreview,
  ReviewSection,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import type { IntelligenceWidgetSnapshot } from '../../lib/core-intelligence'
import { formatDateTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'
import {
  buildDigestMetricItems,
  localizeOutputCardBody,
  localizeOutputCardEyebrow,
  localizeOutputCardTitle,
  type Translate,
} from './external-outputs-shared'
import { OutputTargetLinks } from './external-outputs-target-links'

interface ExternalOutputsWidgetTabProps {
  activeProfileId: string | null
  copyFeedback: ReviewCopyFeedback | null
  copyLabel: string
  commonT: Translate
  json: string
  language: ResolvedLanguage
  onCopy: (key: string, payload: string) => void | Promise<void>
  snapshot: IntelligenceWidgetSnapshot
  t: Translate
  trustedCards: boolean
  intelligenceT: Translate
}

/**
 * Keeps the widget snapshot review path isolated so settings shell changes do not disturb payload-specific UI details.
 */
export function ExternalOutputsWidgetTab({
  activeProfileId,
  copyFeedback,
  copyLabel,
  commonT,
  json,
  language,
  onCopy,
  snapshot,
  t,
  trustedCards,
  intelligenceT,
}: ExternalOutputsWidgetTabProps) {
  return (
    <>
      <ReviewSection
        headerMeta={
          <span className="mono">
            {formatDateTime(snapshot.generatedAt, language) ??
              snapshot.generatedAt}
          </span>
        }
        title={t('externalOutputsWidgetPreviewTitle')}
      >
        <p className="dashboard-next-action">
          {t('externalOutputsWindowLabel', {
            start: snapshot.dateRange.start,
            end: snapshot.dateRange.end,
          })}
        </p>

        {trustedCards ? (
          <StatusCallout
            tone="warning"
            title={t('externalOutputsWidgetTrustedTitle')}
            body={t('externalOutputsWidgetTrustedBody')}
          />
        ) : null}

        <IntelligenceMetricGrid
          className="digest-cards settings-output-digest-grid"
          items={buildDigestMetricItems(
            snapshot.digestSummary,
            language,
            intelligenceT,
          )}
        />

        <div className="settings-output-card-grid">
          {snapshot.highlights.map((card) => (
            <article key={card.cardId} className="settings-output-card">
              <div className="settings-output-card__header">
                <div>
                  {card.eyebrow ? (
                    <p className="mono-kicker">
                      {localizeOutputCardEyebrow(card.eyebrow, t)}
                    </p>
                  ) : null}
                  <h3>{localizeOutputCardTitle(card.title, t)}</h3>
                </div>
                {card.internalOnly ? (
                  <span className="panel-badge">
                    {t('externalOutputsTrustedOnlyBadge')}
                  </span>
                ) : null}
              </div>
              <p>{localizeOutputCardBody(card.body, t)}</p>
              <OutputTargetLinks
                activeProfileId={activeProfileId}
                card={card}
                dateRange={snapshot.dateRange}
                t={t}
              />
            </article>
          ))}
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
        copyKey="widget"
        copyLabel={copyLabel}
        code={json}
        defaultOpen={false}
        errorMessage={t('externalOutputsCopyFailed')}
        onCopy={onCopy}
        successMessage={commonT('copiedNotice')}
        title={t('externalOutputsJsonTitle')}
      />
    </>
  )
}
