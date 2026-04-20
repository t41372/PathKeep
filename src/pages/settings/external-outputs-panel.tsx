/**
 * Manual review/export surface for Core Intelligence external outputs.
 *
 * Why this file exists:
 * - `embed cards`, `widget snapshot`, and `public snapshot` now have a real
 *   front-end consumer surface in Settings instead of living only as backend
 *   payload-provider commands.
 * - Keeping this panel out of `settings/index.tsx` preserves the route's role
 *   as a control tower without turning it into another unreadable mega-file.
 *
 * Main declarations:
 * - `SettingsExternalOutputsPanel`
 *
 * Source-of-truth notes:
 * - Keep the manual-only boundary aligned with `docs/features/core-intelligence-ultimate-design.md`.
 * - Keep scope honesty and time-range behavior aligned with
 *   `docs/design/screens-and-nav.md`.
 */

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { IntelligenceMetricGrid } from '../../components/intelligence/metric-grid'
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import {
  WorkbenchCodePreview,
  WorkbenchReviewSection,
  WorkbenchTargetLinksRow,
  type WorkbenchCopyFeedback,
  type WorkbenchTargetLink,
} from '../../components/intelligence/workbench'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  getIntelligenceEmbedCards,
  getIntelligencePublicSnapshot,
  getIntelligenceWidgetSnapshot,
  useAsyncData,
  useTimeRange,
  type DateRange,
  type IntelligenceEmbedCardPayload,
  type IntelligencePublicSnapshot,
  type IntelligenceWidgetSnapshot,
} from '../../lib/core-intelligence'
import { formatDateTime } from '../../lib/format'
import { createNamespaceTranslator } from '../../lib/i18n/catalog'
import { useI18n } from '../../lib/i18n/hooks'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import {
  dayInsightsHref,
  domainInsightsHref,
  insightEntityReferenceLabel,
  insightEntityReferenceHref,
} from '../../lib/intelligence'
import { SettingsExternalOutputLocalHostPanel } from './external-output-local-host-panel'

type OutputTab = 'embed' | 'widget' | 'public'
type Translate = (key: string, vars?: Record<string, string | number>) => string

interface SettingsExternalOutputsPanelProps {
  initialized: boolean
  unlocked: boolean
}

interface ExternalOutputsPayload {
  embedCards: IntelligenceEmbedCardPayload[]
  widgetSnapshot: IntelligenceWidgetSnapshot
  publicSnapshot: IntelligencePublicSnapshot
}

function prettyJson(value: unknown) {
  return JSON.stringify(value, null, 2)
}

/**
 * Renders the Settings review/export surface for manual Core Intelligence
 * output consumption.
 */
export function SettingsExternalOutputsPanel({
  initialized,
  unlocked,
}: SettingsExternalOutputsPanelProps) {
  const { language, ns } = useI18n()
  const t = ns('settings')
  const commonT = ns('common')
  const intelligenceT = ns('intelligence')
  const navigationT = ns('navigation')
  const { activeProfileId } = useProfileScope()
  const { dateRange, preset, setCustomRange, setPreset } = useTimeRange('month')
  const [activeTab, setActiveTab] = useState<OutputTab>('embed')
  const [copyFeedback, setCopyFeedback] = useState<WorkbenchCopyFeedback | null>(
    null,
  )
  const ready = initialized && unlocked
  const profileScopeLabel = activeProfileId
    ? profileIdLabel(activeProfileId)
    : null
  const outputs = useAsyncData<ExternalOutputsPayload | null>(async () => {
    if (!ready) {
      return null
    }

    const [embedCards, widgetSnapshot, publicSnapshot] = await Promise.all([
      getIntelligenceEmbedCards(dateRange, activeProfileId, 6),
      getIntelligenceWidgetSnapshot(dateRange, activeProfileId, 4),
      getIntelligencePublicSnapshot(dateRange, activeProfileId),
    ])

    return {
      embedCards,
      widgetSnapshot,
      publicSnapshot,
    }
  }, [ready, dateRange, activeProfileId])

  const widgetHasTrustedCards =
    outputs.data?.widgetSnapshot.highlights.some((card) => card.internalOnly) ??
    false
  const publicSnapshotJson = outputs.data
    ? prettyJson(outputs.data.publicSnapshot)
    : ''
  const widgetSnapshotJson = outputs.data
    ? prettyJson(outputs.data.widgetSnapshot)
    : ''
  const embedCardsJson = outputs.data ? prettyJson(outputs.data.embedCards) : ''

  async function handleCopyPayload(key: string, payload: string) {
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error('clipboard unavailable')
      }
      await navigator.clipboard.writeText(payload)
      setCopyFeedback({ key, tone: 'success' })
    } catch {
      setCopyFeedback({ key, tone: 'error' })
    }
  }

  const tabs: { key: OutputTab; label: string }[] = [
    { key: 'embed', label: t('externalOutputsTabEmbed') },
    { key: 'widget', label: t('externalOutputsTabWidget') },
    { key: 'public', label: t('externalOutputsTabPublic') },
  ]

  return (
    <div
      className="panel"
      id="settings-external-outputs"
      data-testid="settings-external-outputs"
    >
      <div className="panel-header">
        <span className="panel-title">{t('externalOutputsTitle')}</span>
        <span className="panel-badge">{t('externalOutputsManualBadge')}</span>
      </div>
      <div className="panel-body settings-remote-grid">
        <StatusCallout
          tone="info"
          title={t('externalOutputsSummaryTitle')}
          body={t('externalOutputsSummaryBody')}
          actions={
            <button
              className="btn-secondary"
              type="button"
              disabled={!ready || outputs.loading}
              onClick={() => outputs.refresh()}
            >
              {t('refresh')}
            </button>
          }
        />

        <StatusCallout
          tone="info"
          title={
            activeProfileId
              ? t('externalOutputsScopedTitle')
              : t('externalOutputsArchiveWideTitle')
          }
          body={
            activeProfileId
              ? t('externalOutputsScopedBody', {
                  profile: profileScopeLabel ?? activeProfileId,
                })
              : t('externalOutputsArchiveWideBody')
          }
        />

        {!initialized ? (
          <StatusCallout
            tone="warning"
            title={t('externalOutputsNeedsArchiveTitle')}
            body={t('externalOutputsNeedsArchiveBody')}
            actions={
              <Link className="btn-secondary" to="/onboarding">
                {navigationT('onboardingLabel')}
              </Link>
            }
          />
        ) : null}

        {initialized && !unlocked ? (
          <StatusCallout
            tone="warning"
            title={t('externalOutputsUnlockTitle')}
            body={t('externalOutputsUnlockBody')}
            actions={
              <Link className="btn-secondary" to="/security">
                {navigationT('securityLabel')}
              </Link>
            }
          />
        ) : null}

        {ready ? (
          <>
            <TimeRangeSelector
              key={`${preset}:${dateRange.start}:${dateRange.end}`}
              dateRange={dateRange}
              preset={preset}
              onCustomRange={setCustomRange}
              onPresetChange={setPreset}
              t={intelligenceT}
            />

            <StatusCallout
              tone="info"
              title={t('externalOutputsManualOnlyTitle')}
              body={t('externalOutputsManualOnlyBody')}
            />

            <div
              className="generated-file-tabs settings-output-tabs"
              role="tablist"
            >
              {tabs.map((tab) => (
                <button
                  key={tab.key}
                  className={`chip-button ${
                    activeTab === tab.key ? 'chip-button--active' : ''
                  }`}
                  type="button"
                  role="tab"
                  aria-selected={activeTab === tab.key}
                  data-testid={`settings-external-outputs-tab-${tab.key}`}
                  onClick={() => setActiveTab(tab.key)}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            {outputs.loading ? (
              <LoadingState label={t('externalOutputsLoading')} />
            ) : outputs.error || !outputs.data ? (
              <StatusCallout
                tone="warning"
                title={t('externalOutputsUnavailableTitle')}
                body={outputs.error ?? t('externalOutputsUnavailableBody')}
                actions={
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => outputs.refresh()}
                  >
                    {t('refresh')}
                  </button>
                }
              />
            ) : (
              <div className="settings-result-list">
                {activeTab === 'embed' ? (
                  <EmbedCardsTab
                    activeProfileId={activeProfileId}
                    cards={outputs.data.embedCards}
                    copyFeedback={copyFeedback}
                    copyLabel={commonT('copyAction')}
                    commonT={commonT}
                    dateRange={dateRange}
                    json={embedCardsJson}
                    onCopy={handleCopyPayload}
                    t={t}
                  />
                ) : null}

                {activeTab === 'widget' ? (
                  <WidgetSnapshotTab
                    activeProfileId={activeProfileId}
                    copyFeedback={copyFeedback}
                    copyLabel={commonT('copyAction')}
                    commonT={commonT}
                    json={widgetSnapshotJson}
                    language={language}
                    onCopy={handleCopyPayload}
                    snapshot={outputs.data.widgetSnapshot}
                    t={t}
                    trustedCards={widgetHasTrustedCards}
                    intelligenceT={intelligenceT}
                  />
                ) : null}

                {activeTab === 'public' ? (
                  <PublicSnapshotTab
                    activeProfileId={activeProfileId}
                    copyFeedback={copyFeedback}
                    copyLabel={commonT('copyAction')}
                    commonT={commonT}
                    json={publicSnapshotJson}
                    language={language}
                    onCopy={handleCopyPayload}
                    snapshot={outputs.data.publicSnapshot}
                    t={t}
                    intelligenceT={intelligenceT}
                  />
                ) : null}
              </div>
            )}

            <SettingsExternalOutputLocalHostPanel
              activeProfileId={activeProfileId}
              dateRange={dateRange}
              ready={ready}
            />
          </>
        ) : null}
      </div>
    </div>
  )
}

function EmbedCardsTab({
  activeProfileId,
  cards,
  copyFeedback,
  copyLabel,
  commonT,
  dateRange,
  json,
  onCopy,
  t,
}: {
  activeProfileId: string | null
  cards: IntelligenceEmbedCardPayload[]
  copyFeedback: WorkbenchCopyFeedback | null
  copyLabel: string
  commonT: Translate
  dateRange: DateRange
  json: string
  onCopy: (key: string, payload: string) => void
  t: Translate
}) {
  return (
    <>
      <WorkbenchReviewSection title={t('externalOutputsEmbedPreviewTitle')}>
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
      </WorkbenchReviewSection>

      <WorkbenchCodePreview
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

function WidgetSnapshotTab({
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
}: {
  activeProfileId: string | null
  copyFeedback: WorkbenchCopyFeedback | null
  copyLabel: string
  commonT: Translate
  json: string
  language: ReturnType<typeof useI18n>['language']
  onCopy: (key: string, payload: string) => void
  snapshot: IntelligenceWidgetSnapshot
  t: Translate
  trustedCards: boolean
  intelligenceT: Translate
}) {
  return (
    <>
      <WorkbenchReviewSection
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
          items={[
            {
              label: intelligenceT('digestVisits'),
              value: snapshot.digestSummary.totalVisits.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestSearches'),
              value: snapshot.digestSummary.totalSearches.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestNewSites'),
              value: snapshot.digestSummary.newDomains.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestDeepRead'),
              value: snapshot.digestSummary.deepReadPages.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestRefind'),
              value: snapshot.digestSummary.refindPages.value.toLocaleString(
                language,
              ),
            },
          ]}
        />

        <div className="settings-output-card-grid">
          {snapshot.highlights.map((card) => (
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
      </WorkbenchReviewSection>

      <WorkbenchCodePreview
        copyFeedback={copyFeedback}
        copyKey="widget"
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

function PublicSnapshotTab({
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
}: {
  activeProfileId: string | null
  copyFeedback: WorkbenchCopyFeedback | null
  copyLabel: string
  commonT: Translate
  json: string
  language: ReturnType<typeof useI18n>['language']
  onCopy: (key: string, payload: string) => void
  snapshot: IntelligencePublicSnapshot
  t: Translate
  intelligenceT: Translate
}) {
  return (
    <>
      <WorkbenchReviewSection
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
          items={[
            {
              label: intelligenceT('digestVisits'),
              value: snapshot.digestSummary.totalVisits.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestSearches'),
              value: snapshot.digestSummary.totalSearches.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestNewSites'),
              value: snapshot.digestSummary.newDomains.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestDeepRead'),
              value: snapshot.digestSummary.deepReadPages.value.toLocaleString(
                language,
              ),
            },
            {
              label: intelligenceT('digestRefind'),
              value: snapshot.digestSummary.refindPages.value.toLocaleString(
                language,
              ),
            },
          ]}
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
      </WorkbenchReviewSection>

      <WorkbenchCodePreview
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

function OutputTargetLinks({
  activeProfileId,
  card,
  dateRange,
  t,
}: {
  activeProfileId: string | null
  card: IntelligenceEmbedCardPayload
  dateRange: DateRange
  t: Translate
}) {
  const { language } = useI18n()
  const intelligenceT = createNamespaceTranslator(
    language === 'zh-CN' || language === 'zh-TW' ? language : 'en',
    'intelligence',
  )
  const primaryHref = card.primaryTarget
    ? insightEntityReferenceHref(card.primaryTarget, {
        dateRange,
        preset: 'custom',
        profileId: activeProfileId,
      })
    : null
  const secondaryTargets = card.secondaryTargets ?? []

  if (!primaryHref && secondaryTargets.length === 0 && !card.href) {
    return null
  }

  return (
    <WorkbenchTargetLinksRow
      fallback={card.href ? <span className="mono">{card.href}</span> : null}
      label={t('externalOutputsHref')}
      primaryHref={primaryHref}
      primaryLabel={t('externalOutputsOpenInsights')}
      secondaryLinks={secondaryTargets.map<WorkbenchTargetLink>(
        (target, index) => ({
          href: insightEntityReferenceHref(target, {
            dateRange,
            preset: 'custom',
            profileId: activeProfileId,
          }),
          key: `${card.cardId}:${target.kind}:${index}`,
          label: insightEntityReferenceLabel(target, intelligenceT),
        }),
      )}
    />
  )
}
