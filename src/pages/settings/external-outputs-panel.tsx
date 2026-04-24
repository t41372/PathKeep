/**
 * Manual review/export surface for Core Intelligence external outputs.
 *
 * Why this file exists:
 * - `embed cards`, `widget snapshot`, and `public snapshot` now have a real
 *   front-end consumer surface in Integrations instead of living only as backend
 *   payload-provider commands.
 * - Keeping this panel out of `settings/index.tsx` preserves Settings as
 *   preferences while Integrations owns payload/artifact review.
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
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import {
  copyReviewValue,
  type ReviewCopyFeedback,
} from '../../components/review'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  getIntelligenceEmbedCards,
  getIntelligencePublicSnapshot,
  getIntelligenceWidgetSnapshot,
  useAsyncData,
  useTimeRange,
} from '../../lib/core-intelligence'
import { useI18n } from '../../lib/i18n/hooks'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { SettingsExternalOutputLocalHostPanel } from './external-output-local-host-panel'
import { ExternalOutputsEmbedTab } from './external-outputs-embed-tab'
import { ExternalOutputsPublicTab } from './external-outputs-public-tab'
import {
  type ExternalOutputsPayload,
  type OutputTab,
  prettyJson,
} from './external-outputs-shared'
import { ExternalOutputsWidgetTab } from './external-outputs-widget-tab'

interface SettingsExternalOutputsPanelProps {
  initialized: boolean
  unlocked: boolean
}

/**
 * Renders the Integrations review/export surface for manual Core Intelligence
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
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
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
    await copyReviewValue(payload, {
      key,
      onFeedback: setCopyFeedback,
    })
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
              {commonT('refreshAction')}
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
                    {commonT('refreshAction')}
                  </button>
                }
              />
            ) : (
              <div className="settings-result-list">
                {activeTab === 'embed' ? (
                  <ExternalOutputsEmbedTab
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
                  <ExternalOutputsWidgetTab
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
                  <ExternalOutputsPublicTab
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
