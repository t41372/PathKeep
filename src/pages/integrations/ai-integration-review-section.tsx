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
  const { currentSettings } = state

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
        <span className="panel-badge">{t('settings.aiDeferredBadge')}</span>
      </div>
      <div className="panel-body settings-remote-grid">
        <StatusCallout
          tone="info"
          title={t('settings.aiIntegrationDeferredTitle')}
          body={t('settings.aiIntegrationDeferredBody')}
        />

        <div className="settings-result-list">
          <div className="result-row" aria-disabled="true">
            <div className="result-row__header">
              <strong>{t('settings.aiMcpCommand')}</strong>
              <span className="mono-support">
                {t('settings.aiDeferredBadge')}
              </span>
            </div>
            <p>{t('settings.aiIntegrationDeferredMcpBody')}</p>
          </div>
          <div className="result-row" aria-disabled="true">
            <div className="result-row__header">
              <strong>{t('settings.aiGeneratedFiles')}</strong>
              <span className="mono-support">
                {t('settings.aiDeferredBadge')}
              </span>
            </div>
            <p>{t('settings.aiIntegrationDeferredFilesBody')}</p>
          </div>
        </div>
      </div>
    </div>
  )
}
