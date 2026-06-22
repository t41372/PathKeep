/**
 * @file ai-providers-section.tsx
 * @description Renders the optional AI provider management surface from route-owned Settings state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 AI master toggle、provider editor lists、index health 與 consent 披露。
 * - 把 provider draft、API key、save/reset 行為交回 route-owned handlers。
 * - 保持 optional intelligence honesty：AI 預設關閉、需顯式 consent，editor 只在開啟後可編輯。
 *
 * ## 不負責
 * - 不保存 AI config 或 API key。
 * - 不管理 semantic index queue。
 * - 不決定 MCP / skill / embedding backend contract（generated artifacts 在 Integrations 頁）。
 *
 * ## 依賴關係
 * - 依賴 `AiProviderEditorList` 呈現 per-purpose provider editor。
 * - 依賴 route hook 提供 current draft、index meta 與 mutation handlers。
 *
 * ## 性能備注
 * - provider editor 只編輯已載入的 draft；index health 來自既有 snapshot，不在 section 內額外 fan-out。
 */

import { Link } from 'react-router-dom'
import type { ReviewCopyFeedback } from '../../components/review'
import { AiProviderEditorList } from '../../components/ai-provider-editor'
import { AiSearchTuningSection } from './ai-search-tuning-section'
import type { SearchTuningKnob } from './search-tuning-helpers'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
import { ToggleRow } from '../../components/ui'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type { aiStatusMeta } from '../../lib/intelligence-ai-presentation'
import type {
  AiIndexStatus,
  AiIntegrationPreview,
  AiProviderConfig,
  AiProviderConnectionTestReport,
  AiRequestFormat,
  AiSettings,
} from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

type AiProviderTranslations = {
  providerName: string
  providerId: string
  requestFormat: string
  baseUrl: string
  baseUrlPlaceholder: string
  defaultModel: string
  modelCatalog: string
  modelCatalogHint: string
  enabled: string
  temperature: string
  maxTokens: string
  dimensions: string
  notes: string
  apiKey: string
  apiKeyPlaceholder: string
  keySaved: string
  keyNotSaved: string
  saveKey: string
  clearKey: string
  remove: string
  testConnection: string
  testingConnection: string
  probeReachable: string
  probeUnreachable: string
  requestFormatLabels: Record<AiRequestFormat, string>
}

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
  providerProbes: Record<string, AiProviderConnectionTestReport>
  providerTranslations: AiProviderTranslations
  saving: boolean
  testingProviderId: string | null
  onAddProvider: (purpose: 'llm' | 'embedding', format: AiRequestFormat) => void
  onApiKeyChange: (providerId: string, value: string) => void
  onClearAiApiKey: (providerId: string) => Promise<void>
  onCopyIntegrationValue: (key: string, value: string) => Promise<void>
  onOpenPath: (path: string) => void
  onProviderProbe: (
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) => Promise<void>
  onRemoveProvider: (purpose: 'llm' | 'embedding', providerId: string) => void
  onResetAiConfig: () => void
  onResetSearchTuning: () => void
  onSaveAiApiKey: (providerId: string) => Promise<void>
  onSaveAiConfig: () => Promise<void>
  onSearchTuningChange: (knob: SearchTuningKnob, value: number) => void
  onSelectProvider: (purpose: 'llm' | 'embedding', providerId: string) => void
  onToggleAi: () => void
  onToggleAssistant: () => void
  onToggleSemanticIndex: () => void
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
 * invents placeholder providers outside the real Settings route owner. The
 * master toggle reflects the persisted (default-OFF) `config.ai.enabled`; while
 * it is off the provider editors render disabled-but-visible so the user can
 * see exactly what configuring AI would unlock before opting in.
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
    currentSettings,
    indexMeta,
    noProviders,
    persistedProviderIds,
    providerProbes,
    providerTranslations,
    saving,
    testingProviderId,
    onAddProvider,
    onApiKeyChange,
    onClearAiApiKey,
    onProviderProbe,
    onRemoveProvider,
    onResetAiConfig,
    onResetSearchTuning,
    onSaveAiApiKey,
    onSaveAiConfig,
    onSearchTuningChange,
    onSelectProvider,
    onToggleAi,
    onToggleAssistant,
    onToggleSemanticIndex,
    onUpdateProvider,
  } = state

  if (!currentSettings) {
    return null
  }

  // Master AI consent is OFF by default. While off the provider editors stay
  // visible but inert, and saving also freezes them so a write in flight can
  // never be raced. This is the only place AI work becomes possible.
  const aiOn = currentSettings.enabled
  const editorsDisabled = saving || !aiOn

  // Probe a provider only once it is persisted (the backend reads saved config
  // by id, not the in-flight draft) and AI is on, and never while another probe
  // or save is in flight.
  const onProbeDisabled = (providerId: string) =>
    editorsDisabled ||
    testingProviderId !== null ||
    !persistedProviderIds.has(providerId)
  const probeLatencyLabel = (latency: number, model: string) =>
    t('settings.aiProbeLatency', {
      latency: latency.toLocaleString(language),
      model,
    })
  const presetLabels: Record<AiRequestFormat, string> = {
    'lm-studio': t('settings.aiPresetLmStudio'),
    ollama: t('settings.aiPresetOllama'),
    openai: t('settings.aiPresetOpenai'),
    anthropic: t('settings.aiPresetAnthropic'),
    google: t('settings.aiPresetGoogle'),
  }

  const onSaveKeyDisabled = (providerId: string) =>
    saving ||
    !persistedProviderIds.has(providerId) ||
    !aiApiKeys[providerId]?.trim()
  const onClearKeyDisabled = (providerId: string) =>
    saving || !persistedProviderIds.has(providerId)

  return (
    <PaperCard testId={navItem.id}>
      <PaperCardHeader
        title={navItem.label}
        right={<PaperCardBadge>{t('settings.optional')}</PaperCardBadge>}
      />
      <PaperCardBody className="flex flex-col gap-4">
        <p className="text-ink-muted m-0 font-serif text-[13.5px] leading-[1.55] italic">
          {t('settings.aiProviderBody')}
        </p>

        {/*
          AISETUP-5 consent disclosure — intentionally always visible, even
          while AI is off, so the user reads exactly what enabling AI means
          (where their history text / queries / chat goes, that PathKeep ships
          no provider, what stays local) BEFORE they flip the switch.
        */}
        <div data-testid="ai-consent-disclosure" id="ai-consent-disclosure">
          <StatusCallout
            tone="info"
            title={t('settings.aiConsentDisclosureTitle')}
            body={t('settings.aiConsentDisclosureBody')}
          />
          <ul className="text-ink-muted mt-2 flex list-none flex-col gap-1.5 p-0 font-sans text-[12px] leading-[1.5]">
            <li>{t('settings.aiConsentDisclosureNoProvider')}</li>
            <li>{t('settings.aiConsentDisclosureEgress')}</li>
            {/*
              W-AI-8 WU-3 — code-mode disclosure (not a per-run gate): the assistant's tool harness
              is default-enabled and can WRITE + RUN small sandboxed (read-only, no egress, bounded)
              programs over the history to search/combine results, always showing the exact code +
              queries it ran. Stated here so the always-visible consent surface is complete; there is
              no "agent wants to run code" affordance because the sandbox makes a per-run gate noise.
            */}
            <li>{t('settings.aiConsentDisclosureCodeMode')}</li>
            <li>{t('settings.aiConsentDisclosureLocal')}</li>
          </ul>
        </div>

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
            <div className="flex flex-wrap items-center gap-2">
              <button
                className="btn-primary"
                type="button"
                disabled={saving || !configDirty}
                onClick={() => {
                  void onSaveAiConfig()
                }}
                data-testid="ai-save-config"
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
                data-testid="ai-reset-config"
              >
                {t('settings.aiResetDraft')}
              </button>
            </div>
          }
        />

        <div className="toggleList">
          <ToggleRow
            checked={aiOn}
            describedById="ai-consent-disclosure"
            disabled={saving}
            label={t('settings.aiMasterToggle')}
            onChange={onToggleAi}
          />
          {/*
            Granular consent (B1): the assistant and semantic-search capabilities
            each have their own opt-in below the master switch. They stay visible
            but inert while the master is off (same pattern as the disabled
            provider editors) so the user can see what AI unlocks before opting
            in, and they never cascade from the master — turning AI on does not
            silently start a chat plane or a 14.4M-row embedding backfill.
          */}
          <ToggleRow
            checked={currentSettings.assistantEnabled}
            disabled={editorsDisabled}
            label={t('settings.aiAssistantToggle')}
            onChange={onToggleAssistant}
          />
          <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
            {t('settings.aiAssistantToggleHelp')}
          </p>
          <ToggleRow
            checked={currentSettings.semanticIndexEnabled}
            disabled={editorsDisabled}
            label={t('settings.aiSemanticToggle')}
            onChange={onToggleSemanticIndex}
          />
          <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
            {t('settings.aiSemanticToggleHelp')}
          </p>
          {!aiOn ? (
            <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5] italic">
              {t('settings.aiSubToggleDisabledHint')}
            </p>
          ) : null}
        </div>

        {/*
          Power-user search tuning (W-AI-9 / W-AI-6). Tucked behind a collapsed
          disclosure so it never clutters the normal AI config, and gated by the
          same `editorsDisabled` as the provider editors — visible-but-inert while
          AI is off so the user can see what tuning unlocks before opting in. The
          knobs mutate the draft only and persist through the shared Save above.
        */}
        <AiSearchTuningSection
          settings={currentSettings}
          disabled={editorsDisabled}
          onChange={onSearchTuningChange}
          onReset={onResetSearchTuning}
        />

        <AiProviderEditorList
          addLabel={t('settings.aiAddLlmProvider')}
          apiKeys={aiApiKeys}
          disabled={editorsDisabled}
          formatLabel={probeLatencyLabel}
          presetLabel={t('settings.aiAddProviderPresetLabel')}
          presetLabels={presetLabels}
          onAdd={(format) => onAddProvider('llm', format)}
          onApiKeyChange={onApiKeyChange}
          onClearKey={(providerId) => {
            void onClearAiApiKey(providerId)
          }}
          onClearKeyDisabled={onClearKeyDisabled}
          onProbe={(providerId) => {
            void onProviderProbe('llm', providerId)
          }}
          onProbeDisabled={onProbeDisabled}
          onRemove={(providerId) => onRemoveProvider('llm', providerId)}
          onSaveKey={(providerId) => {
            void onSaveAiApiKey(providerId)
          }}
          onSaveKeyDisabled={onSaveKeyDisabled}
          onSelect={(providerId) => onSelectProvider('llm', providerId)}
          onUpdate={(providerId, patch) =>
            onUpdateProvider('llm', providerId, patch)
          }
          providerProbes={providerProbes}
          providers={currentSettings.llmProviders}
          purpose="llm"
          selectedProviderId={currentSettings.llmProviderId ?? null}
          testingProviderId={testingProviderId}
          title={t('settings.aiLlmProviders')}
          translations={providerTranslations}
        />

        <AiProviderEditorList
          addLabel={t('settings.aiAddEmbeddingProvider')}
          apiKeys={aiApiKeys}
          disabled={editorsDisabled}
          formatLabel={probeLatencyLabel}
          presetLabel={t('settings.aiAddProviderPresetLabel')}
          presetLabels={presetLabels}
          onAdd={(format) => onAddProvider('embedding', format)}
          onApiKeyChange={onApiKeyChange}
          onClearKey={(providerId) => {
            void onClearAiApiKey(providerId)
          }}
          onClearKeyDisabled={onClearKeyDisabled}
          onProbe={(providerId) => {
            void onProviderProbe('embedding', providerId)
          }}
          onProbeDisabled={onProbeDisabled}
          onRemove={(providerId) => onRemoveProvider('embedding', providerId)}
          onSaveKey={(providerId) => {
            void onSaveAiApiKey(providerId)
          }}
          onSaveKeyDisabled={onSaveKeyDisabled}
          onSelect={(providerId) => onSelectProvider('embedding', providerId)}
          onUpdate={(providerId, patch) =>
            onUpdateProvider('embedding', providerId, patch)
          }
          providerProbes={providerProbes}
          providers={currentSettings.embeddingProviders}
          purpose="embedding"
          selectedProviderId={currentSettings.embeddingProviderId ?? null}
          testingProviderId={testingProviderId}
          title={t('settings.aiEmbeddingProviders')}
          translations={providerTranslations}
        />

        <div className="config-row">
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

        {indexMeta && aiStatus ? (
          <>
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
            <div className="settings-field-grid">
              <div className="config-row">
                <span className="config-label">
                  {t('settings.aiIndexedRows')}
                </span>
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
                <p>
                  {aiStatus.warning ===
                  'Select an embedding provider in Settings before enabling semantic retrieval.'
                    ? t('settings.aiIndexWarningEmbeddingMissing')
                    : aiStatus.warning}
                </p>
              </div>
            ) : null}
          </>
        ) : null}

        <StatusCallout
          tone="info"
          title={t('settings.aiArtifactsMovedTitle')}
          body={t('settings.aiArtifactsMovedBody')}
          actions={
            <Link className="btn-secondary" to="/integrations">
              {t('navigation.integrationsLabel')}
            </Link>
          }
        />
      </PaperCardBody>
    </PaperCard>
  )
}
