/**
 * @file ai-providers-section.tsx
 * @description Renders the optional AI provider management surface from route-owned Settings state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 AI master toggle、provider editor lists、index health 與 integration review artifacts。
 * - 把 provider draft、API key、save/reset、copy/open-path 行為交回 route-owned handlers。
 * - 保持 optional intelligence honesty，不把 external integration preview 誤包裝成自動執行。
 *
 * ## 不負責
 * - 不保存 AI config 或 API key。
 * - 不管理 semantic index queue。
 * - 不決定 MCP / skill / embedding backend contract。
 *
 * ## 依賴關係
 * - 依賴 `AiProviderEditorList` 與 review components 呈現 provider 與 integration review surface。
 * - 依賴 route hook 提供 current draft、integration preview 與 mutation handlers。
 *
 * ## 性能備注
 * - provider editor 只編輯已載入的 draft；generated files 與 integration preview 來自既有 backend preview，不在 section 內額外 fan-out。
 */

import type { ComponentProps } from 'react'
import {
  GeneratedArtifactViewer,
  ReviewSection,
  type ReviewCopyFeedback,
} from '../../components/review'
import { AiProviderEditorList } from '../../components/ai-provider-editor'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type { aiStatusMeta } from '../../lib/intelligence'
import type {
  AiIndexStatus,
  AiIntegrationPreview,
  AiProviderConfig,
  AiSettings,
} from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

type AiProviderTranslations = ComponentProps<
  typeof AiProviderEditorList
>['translations']

/**
 * Defines the route-owned AI settings state consumed by this section.
 */
export interface AiProvidersSectionState {
  aiApiKeys: Record<string, string>
  aiStatus: AiIndexStatus | null
  configDirty: boolean
  copyFeedback: ReviewCopyFeedback | null
  currentSettings: AiSettings | null
  indexMeta: ReturnType<typeof aiStatusMeta> | null
  integrationError: string | null
  integrationPreview: AiIntegrationPreview | null
  noProviders: boolean
  persistedProviderIds: Set<string>
  providerTranslations: AiProviderTranslations
  saving: boolean
  onAddProvider: (purpose: 'llm' | 'embedding') => void
  onApiKeyChange: (providerId: string, value: string) => void
  onClearAiApiKey: (providerId: string) => Promise<void>
  onCopyIntegrationValue: (key: string, value: string) => Promise<void>
  onOpenPath: (path: string) => void
  onRemoveProvider: (purpose: 'llm' | 'embedding', providerId: string) => void
  onResetAiConfig: () => void
  onSaveAiApiKey: (providerId: string) => Promise<void>
  onSaveAiConfig: () => Promise<void>
  onSelectProvider: (purpose: 'llm' | 'embedding', providerId: string) => void
  onToggleAi: () => void
  onUpdateProvider: (
    purpose: 'llm' | 'embedding',
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) => void
}

/**
 * Groups the stable section anchor descriptor with the AI settings view-model.
 */
export interface AiProvidersSectionProps {
  navItem: SettingsSectionNavItem
  state: AiProvidersSectionState
}

/**
 * Renders the optional AI provider management surface from route-owned state.
 *
 * The section exits early when there is no current AI draft, so it never
 * invents placeholder providers outside the real Settings route owner.
 */
export function AiProvidersSection({
  navItem,
  state,
}: AiProvidersSectionProps) {
  const { language, t } = useI18n()
  const {
    aiApiKeys,
    aiStatus,
    configDirty,
    copyFeedback,
    currentSettings,
    indexMeta,
    integrationError,
    integrationPreview,
    noProviders,
    persistedProviderIds,
    providerTranslations,
    saving,
    onAddProvider,
    onApiKeyChange,
    onClearAiApiKey,
    onCopyIntegrationValue,
    onOpenPath,
    onRemoveProvider,
    onResetAiConfig,
    onSaveAiApiKey,
    onSaveAiConfig,
    onSelectProvider,
    onToggleAi,
    onUpdateProvider,
  } = state

  if (!currentSettings || !indexMeta || !aiStatus) {
    return null
  }

  return (
    <div className="panel panel--optional" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
        <span className="panel-badge">{t('settings.optional')}</span>
      </div>
      <div className="panel-body">
        <p className="dashboard-next-action">{t('settings.aiProviderBody')}</p>
        {noProviders ? (
          <StatusCallout
            tone="info"
            title={t('settings.aiGettingStartedTitle')}
            body={t('settings.aiGettingStartedBody')}
          />
        ) : null}
        <StatusCallout
          tone={configDirty ? 'warning' : 'info'}
          title={
            configDirty
              ? t('settings.aiUnsavedChanges')
              : t('settings.aiDraftSaved')
          }
          body={t('settings.aiDraftBoundaryBody')}
          actions={
            <div className="settings-action-row">
              <button
                className="btn-primary"
                type="button"
                disabled={saving || !configDirty}
                onClick={() => {
                  void onSaveAiConfig()
                }}
              >
                {saving
                  ? t('settings.aiSavingConfig')
                  : t('settings.aiSaveConfig')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={saving || !configDirty}
                onClick={onResetAiConfig}
              >
                {t('settings.aiResetDraft')}
              </button>
            </div>
          }
        />

        <label className="checkbox-row">
          <input
            aria-label={t('settings.aiMasterToggle')}
            checked={currentSettings.enabled}
            type="checkbox"
            disabled={saving}
            onChange={() => {
              onToggleAi()
            }}
          />
          <span>{t('settings.aiMasterToggle')}</span>
        </label>

        <AiProviderEditorList
          addLabel={t('settings.aiAddLlmProvider')}
          apiKeys={aiApiKeys}
          disabled={saving}
          onAdd={() => onAddProvider('llm')}
          onApiKeyChange={onApiKeyChange}
          onClearKey={(providerId) => {
            void onClearAiApiKey(providerId)
          }}
          onClearKeyDisabled={(providerId) =>
            saving || !persistedProviderIds.has(providerId)
          }
          onRemove={(providerId) => onRemoveProvider('llm', providerId)}
          onSaveKey={(providerId) => {
            void onSaveAiApiKey(providerId)
          }}
          onSaveKeyDisabled={(providerId) =>
            saving ||
            !persistedProviderIds.has(providerId) ||
            !aiApiKeys[providerId]?.trim()
          }
          onSelect={(providerId) => onSelectProvider('llm', providerId)}
          onUpdate={(providerId, patch) =>
            onUpdateProvider('llm', providerId, patch)
          }
          providers={currentSettings.llmProviders}
          purpose="llm"
          selectedProviderId={currentSettings.llmProviderId ?? null}
          title={t('settings.aiLlmProviders')}
          translations={providerTranslations}
        />

        <AiProviderEditorList
          addLabel={t('settings.aiAddEmbeddingProvider')}
          apiKeys={aiApiKeys}
          disabled={saving}
          onAdd={() => onAddProvider('embedding')}
          onApiKeyChange={onApiKeyChange}
          onClearKey={(providerId) => {
            void onClearAiApiKey(providerId)
          }}
          onClearKeyDisabled={(providerId) =>
            saving || !persistedProviderIds.has(providerId)
          }
          onRemove={(providerId) => onRemoveProvider('embedding', providerId)}
          onSaveKey={(providerId) => {
            void onSaveAiApiKey(providerId)
          }}
          onSaveKeyDisabled={(providerId) =>
            saving ||
            !persistedProviderIds.has(providerId) ||
            !aiApiKeys[providerId]?.trim()
          }
          onSelect={(providerId) => onSelectProvider('embedding', providerId)}
          onUpdate={(providerId, patch) =>
            onUpdateProvider('embedding', providerId, patch)
          }
          providers={currentSettings.embeddingProviders}
          purpose="embedding"
          selectedProviderId={currentSettings.embeddingProviderId ?? null}
          title={t('settings.aiEmbeddingProviders')}
          translations={providerTranslations}
        />

        <div className="config-row" style={{ marginTop: 'var(--space-4)' }}>
          <span className="config-label">
            {t('settings.aiActiveLlmProvider')}
          </span>
          <span className="config-value mono">
            {currentSettings.llmProviders.find(
              (provider) => provider.id === currentSettings.llmProviderId,
            )?.name ?? t('settings.aiNoneSelected')}
          </span>
        </div>
        <div className="config-row">
          <span className="config-label">
            {t('settings.aiActiveEmbeddingProvider')}
          </span>
          <span className="config-value mono">
            {currentSettings.embeddingProviders.find(
              (provider) => provider.id === currentSettings.embeddingProviderId,
            )?.name ?? t('settings.aiNoneSelected')}
          </span>
        </div>

        <div className="ai-health-indicator">
          <span className={`ai-health-dot ai-health-dot--${indexMeta.tone}`} />
          <StatusCallout
            tone={
              indexMeta.tone === 'success'
                ? 'success'
                : indexMeta.tone === 'warning'
                  ? 'warning'
                  : indexMeta.tone === 'blocked'
                    ? 'blocked'
                    : 'info'
            }
            title={t('settings.aiIndexHealthTitle', {
              status: indexMeta.label,
            })}
            body={indexMeta.description}
          />
        </div>

        <div className="settings-field-grid">
          <div className="config-row">
            <span className="config-label">{t('settings.aiIndexedRows')}</span>
            <span className="config-value mono">
              {aiStatus.indexedItems.toLocaleString(language)}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.aiSemanticSidecar')}
            </span>
            <span className="config-value mono">
              {formatBytes(aiStatus.semanticSidecarBytes, language)}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.aiSemanticMetadata')}
            </span>
            <span className="config-value mono">
              {formatBytes(aiStatus.semanticMetadataBytes, language)}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.aiEstimatedTokens')}
            </span>
            <span className="config-value mono">
              {aiStatus.estimatedEmbeddingTokens.toLocaleString(language)}
            </span>
          </div>
        </div>

        {aiStatus.warning ? (
          <div className="result-row">
            <div className="result-row__header">
              <strong>{t('settings.aiIndexWarning')}</strong>
            </div>
            <p>{aiStatus.warning}</p>
          </div>
        ) : null}

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
          ) : null}
        </div>
      </div>
    </div>
  )
}
