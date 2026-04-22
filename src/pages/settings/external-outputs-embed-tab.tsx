/**
 * @file external-outputs-embed-tab.tsx
 * @description Renders the embed-card review tab inside Settings external outputs.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Render the manual preview for embed-card payloads.
 * - Keep JSON copy/export affordances paired with the visual preview.
 *
 * ## Not responsible for
 * - Fetching payloads or managing the active tab state
 * - Rendering widget/public snapshot previews
 *
 * ## Dependencies
 * - Depends on review primitives plus the shared output target-link renderer.
 *
 * ## Performance notes
 * - Pure render component; avoid extra state so tab switching stays cheap even when several cards are present.
 */

import {
  ReviewCodePreview,
  ReviewSection,
  type ReviewCopyFeedback,
} from '../../components/review'
import type {
  DateRange,
  IntelligenceEmbedCardPayload,
} from '../../lib/core-intelligence'
import { OutputTargetLinks } from './external-outputs-target-links'
import type { Translate } from './external-outputs-shared'

interface ExternalOutputsEmbedTabProps {
  activeProfileId: string | null
  cards: IntelligenceEmbedCardPayload[]
  copyFeedback: ReviewCopyFeedback | null
  copyLabel: string
  commonT: Translate
  dateRange: DateRange
  json: string
  onCopy: (key: string, payload: string) => void | Promise<void>
  t: Translate
}

/**
 * Keeps the embed preview isolated from the panel shell so card presentation can evolve without re-growing the route file.
 */
export function ExternalOutputsEmbedTab({
  activeProfileId,
  cards,
  copyFeedback,
  copyLabel,
  commonT,
  dateRange,
  json,
  onCopy,
  t,
}: ExternalOutputsEmbedTabProps) {
  return (
    <>
      <ReviewSection title={t('externalOutputsEmbedPreviewTitle')}>
        {cards.length > 0 ? (
          <div className="settings-output-card-grid">
            {cards.map((card) => (
              <article key={card.cardId} className="settings-output-card">
                <div className="settings-output-card__header">
                  <div>
                    {card.eyebrow ? (
                      <p className="mono-kicker">{card.eyebrow}</p>
                    ) : null}
                    <h3>{card.title}</h3>
                  </div>
                  {card.internalOnly ? (
                    <span className="panel-badge">
                      {t('externalOutputsTrustedOnlyBadge')}
                    </span>
                  ) : null}
                </div>
                <p>{card.body}</p>
                {card.metricLabel && card.metricValue ? (
                  <div className="config-row">
                    <span className="config-label mono">
                      {card.metricLabel}
                    </span>
                    <span className="config-value mono">
                      {card.metricValue}
                    </span>
                  </div>
                ) : null}
                <OutputTargetLinks
                  activeProfileId={activeProfileId}
                  card={card}
                  dateRange={dateRange}
                  t={t}
                />
              </article>
            ))}
          </div>
        ) : (
          <p>{t('externalOutputsEmbedEmpty')}</p>
        )}
      </ReviewSection>

      <ReviewCodePreview
        copyFeedback={copyFeedback}
        copyKey="embed"
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
