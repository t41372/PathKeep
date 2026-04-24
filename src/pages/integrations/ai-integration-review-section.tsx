/**
 * @file ai-integration-review-section.tsx
 * @description Renders MCP and skill artifact review under the Integrations route.
 * @module pages/integrations
 *
 * ## Responsibilities
 * - Own the generated MCP command, capability notes, scope boundaries, audit trace, and generated-file review UI.
 * - Keep external integration artifacts out of AI provider preferences.
 * - Preserve preview-only honesty: users review and copy artifacts manually.
 *
 * ## Not responsible for
 * - Editing AI provider configuration or API keys.
 * - Installing third-party tools or executing generated commands.
 * - Running derived-data rebuilds or queue retries.
 *
 * ## Dependencies
 * - Consumes the AI route state from `use-settings-ai-state` so preview generation stays single-sourced.
 * - Uses shared review primitives for generated files and code previews.
 *
 * ## Performance notes
 * - Preview payloads are loaded by the route state hook; this component only renders bounded review panels.
 */

import { GeneratedArtifactViewer, ReviewSection } from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import type { AiProvidersSectionState } from '../settings/ai-providers-section'

/**
 * Props for the Integrations-owned AI artifact review surface.
 */
export interface AiIntegrationReviewSectionProps {
  state: AiProvidersSectionState
}

/**
 * Renders the optional AI/MCP generated artifact review surface.
 */
export function AiIntegrationReviewSection({
  state,
}: AiIntegrationReviewSectionProps) {
  const { t } = useI18n()
  const {
    copyFeedback,
    currentSettings,
    integrationError,
    integrationPreview,
    onCopyIntegrationValue,
    onOpenPath,
  } = state

  if (!currentSettings) {
    return null
  }

  return (
    <div className="panel panel--optional" id="integrations-ai-artifacts">
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon="smart_toy" filled />
          <span>{t('settings.aiIntegrationArtifactsTitle')}</span>
        </span>
        <span className="panel-badge">{t('settings.externalReviewBadge')}</span>
      </div>
      <div className="panel-body settings-remote-grid">
        <StatusCallout
          tone="info"
          title={t('settings.aiIntegrationArtifactsSummaryTitle')}
          body={t('settings.aiIntegrationArtifactsSummaryBody')}
        />

        <div className="settings-result-list">
          {integrationError ? (
            <StatusCallout
              tone="warning"
              title={t('settings.aiIntegrationUnavailable')}
              body={integrationError}
            />
          ) : integrationPreview ? (
            <>
              <StatusCallout
                tone={
                  integrationPreview.warnings.length > 0 ? 'warning' : 'info'
                }
                title={t('settings.aiIntegrationReview')}
                body={integrationPreview.consentSummary}
              />
              <ReviewSection title={t('settings.aiMcpCommand')}>
                <div className="code-panel">
                  <pre>{integrationPreview.mcpCommand}</pre>
                </div>
              </ReviewSection>
              <ReviewSection title={t('settings.aiCapabilityNotes')}>
                {integrationPreview.capabilityNotes.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </ReviewSection>
              <ReviewSection title={t('settings.aiScopeBoundary')}>
                {integrationPreview.scopeBoundary.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </ReviewSection>
              <ReviewSection title={t('settings.aiAuditTrace')}>
                {integrationPreview.auditTrace.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </ReviewSection>
              <ReviewSection title={t('settings.aiGeneratedFiles')}>
                {integrationPreview.generatedFiles.length > 0 ? (
                  <GeneratedArtifactViewer
                    copyFeedback={copyFeedback}
                    copyLabel={t('common.copyAction')}
                    copyPathLabel={t('common.copyAction')}
                    errorMessage={t('audit.copyFailed')}
                    files={integrationPreview.generatedFiles}
                    onCopy={(key, value) => {
                      void onCopyIntegrationValue(key, value)
                    }}
                    onOpenPath={onOpenPath}
                    openPathLabel={t('common.openPath')}
                    successMessage={t('common.copiedNotice')}
                  />
                ) : null}
              </ReviewSection>
              <ReviewSection title={t('settings.aiManualSteps')}>
                {integrationPreview.manualSteps.map((step) => (
                  <p key={step}>{step}</p>
                ))}
                {integrationPreview.warnings.map((warning) => (
                  <p key={warning}>{warning}</p>
                ))}
              </ReviewSection>
            </>
          ) : (
            <StatusCallout
              tone="info"
              title={t('settings.aiIntegrationLoadingTitle')}
              body={t('settings.aiIntegrationLoadingBody')}
            />
          )}
        </div>
      </div>
    </div>
  )
}
