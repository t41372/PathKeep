/**
 * @file ai-providers-section.tsx
 * @description Renders the optional AI provider management surface from route-owned Settings state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 AI master toggle、provider editor lists、index health 與 consent 披露。
 * - 全部 auto-save：toggle / selection / tuning / GPU / add / remove 立即存，provider 欄位在 blur 時 commit；成功後閃 "Saved" chip。
 * - 把 API key save/clear、test connection 這類顯式動作交回 route-owned handlers。
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
import { AiGpuSection } from './ai-gpu-section'
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
import { localizeAiIndexWarning } from '../../lib/ai/note-codes'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import { SettingsSavedChip } from './settings-saved-feedback'
import { useSavedFeedback } from './use-saved-feedback'
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
  copyFeedback: ReviewCopyFeedback | null
  currentSettings: AiSettings | null
  indexMeta: ReturnType<typeof aiStatusMeta> | null
  integrationError: string | null
  integrationPreview: AiIntegrationPreview | null
  noProviders: boolean
  providerProbes: Record<string, AiProviderConnectionTestReport>
  providerTranslations: AiProviderTranslations
  saving: boolean
  testingProviderId: string | null
  // Structural controls auto-save and resolve to `true` only when a write landed,
  // so the section can flash the quiet "Saved" chip on success and stay silent on
  // a no-op (settings unchanged) or a failure.
  onAddProvider: (
    purpose: 'llm' | 'embedding',
    format: AiRequestFormat,
  ) => Promise<boolean>
  onApiKeyChange: (providerId: string, value: string) => void
  onClearAiApiKey: (providerId: string) => Promise<void>
  // Commit in-progress provider field edits on blur (auto-save). No-ops to false
  // when the editing buffer already matches saved config.
  onCommitProviders: () => Promise<boolean>
  onCopyIntegrationValue: (key: string, value: string) => Promise<void>
  onOpenPath: (path: string) => void
  onProviderProbe: (
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) => Promise<void>
  onRemoveProvider: (
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) => Promise<boolean>
  onResetSearchTuning: () => Promise<boolean>
  onSaveAiApiKey: (providerId: string) => Promise<void>
  onSearchTuningChange: (
    knob: SearchTuningKnob,
    value: number,
  ) => Promise<boolean>
  onSelectProvider: (
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) => Promise<boolean>
  onToggleAi: () => Promise<boolean>
  onToggleAssistant: () => Promise<boolean>
  onToggleGpu: () => Promise<boolean>
  onToggleMcp: () => Promise<boolean>
  onToggleSkill: () => Promise<boolean>
  onToggleSemanticIndex: () => Promise<boolean>
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
  const { visible: savedVisible, flash } = useSavedFeedback()
  const {
    aiApiKeys,
    aiStatus,
    currentSettings,
    indexMeta,
    noProviders,
    providerProbes,
    providerTranslations,
    saving,
    testingProviderId,
    onAddProvider,
    onApiKeyChange,
    onClearAiApiKey,
    onCommitProviders,
    onProviderProbe,
    onRemoveProvider,
    onResetSearchTuning,
    onSaveAiApiKey,
    onSearchTuningChange,
    onSelectProvider,
    onToggleAi,
    onToggleAssistant,
    onToggleGpu,
    onToggleMcp,
    onToggleSkill,
    onToggleSemanticIndex,
    onUpdateProvider,
  } = state

  // Flash the quiet "Saved" chip only when an auto-save actually persisted. Every
  // structural control returns true on a real write and false on a no-op/failure.
  // `persistAi` re-throws when the underlying save fails (the shell already set the
  // error banner), so swallow the rejection here: the chip correctly stays hidden
  // and we avoid an unhandled-rejection on every failing toggle.
  const flashOnSave = (saved: Promise<boolean>) => {
    void saved
      .then((didSave) => {
        if (didSave) {
          flash()
        }
      })
      .catch(() => {})
  }

  if (!currentSettings) {
    return null
  }

  // Master AI consent is OFF by default. While off the provider editors stay
  // visible but inert, and saving also freezes them so a write in flight can
  // never be raced. This is the only place AI work becomes possible.
  const aiOn = currentSettings.enabled
  const editorsDisabled = saving || !aiOn

  // Probe a provider whenever no save and no other probe is in flight. Because a
  // provider is auto-persisted the moment it's added, there is no "save the
  // provider first" gate anymore — every provider on screen is already saved
  // config the backend can probe by id. This deliberately does NOT require the AI
  // master toggle to be on (you test an endpoint BEFORE opting into AI).
  const onProbeDisabled = () => saving || testingProviderId !== null
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

  // Save key is enabled as soon as a key is typed for a (now always-persisted)
  // provider; Clear key is enabled whenever a save isn't already in flight.
  const onSaveKeyDisabled = (providerId: string) =>
    saving || !aiApiKeys[providerId]?.trim()
  const onClearKeyDisabled = () => saving

  return (
    <PaperCard testId={navItem.id}>
      <PaperCardHeader
        title={navItem.label}
        right={
          <div className="flex items-center gap-2">
            <SettingsSavedChip visible={savedVisible} />
            <PaperCardBadge>{t('settings.optional')}</PaperCardBadge>
          </div>
        }
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

        <div className="toggleList">
          <ToggleRow
            checked={aiOn}
            describedById="ai-consent-disclosure"
            disabled={saving}
            label={t('settings.aiMasterToggle')}
            onChange={() => flashOnSave(onToggleAi())}
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
            onChange={() => flashOnSave(onToggleAssistant())}
          />
          <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
            {t('settings.aiAssistantToggleHelp')}
          </p>
          <ToggleRow
            checked={currentSettings.semanticIndexEnabled}
            disabled={editorsDisabled}
            label={t('settings.aiSemanticToggle')}
            onChange={() => flashOnSave(onToggleSemanticIndex())}
          />
          <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
            {t('settings.aiSemanticToggleHelp')}
          </p>
          {/*
            Outward data-surface consent (W-AI-9 Sub-block B). This is the ONLY
            switch that exposes the archive to tools outside PathKeep, so the
            disclosure is calmer and more explicit than the in-app sub-toggles:
            it names exactly what enabling does (a localhost-only server that
            lets external AI tools you connect run the same bounded, read-only
            search the in-app agent uses, with every query audited), states that
            nothing is exposed until you turn it on, and points to Integrations
            for the connect command. Hard-default-OFF, gated behind AI like the
            others, mutates only the draft, and never auto-starts the worker.
          */}
          <ToggleRow
            checked={currentSettings.mcpEnabled}
            describedById="ai-mcp-disclosure"
            disabled={editorsDisabled}
            label={t('settings.aiMcpToggle')}
            onChange={() => flashOnSave(onToggleMcp())}
          />
          <div
            className="text-ink-muted m-0 flex flex-col items-start gap-1.5 font-sans text-[12px] leading-[1.5]"
            data-testid="ai-mcp-disclosure"
            id="ai-mcp-disclosure"
          >
            <p className="m-0">{t('settings.aiMcpToggleHelp')}</p>
            <p className="m-0">{t('settings.aiMcpToggleAudit')}</p>
            {/*
              The audit sentence above promises every external query is logged,
              so give the user a way to actually read it: mcp_query runs surface
              on the Audit Ledger (route /audit), filterable by run type. Mirror
              the connect link's btn-tiny pattern so the trust signal is
              actionable, not just a claim.
            */}
            <Link className="btn-tiny" to="/audit">
              {t('settings.aiMcpToggleAuditLink')}
            </Link>
            <p className="m-0">{t('settings.aiMcpToggleConnect')}</p>
            <Link className="btn-tiny" to="/integrations">
              {t('settings.aiMcpToggleConnectLink')}
            </Link>
          </div>
          {/*
            Skill / usage-guide consent (W-AI-9 Sub-block C). This does NOT expose
            any new data: it serves a built-in, read-only guide that teaches a
            connected external agent HOW to query effectively (granularity,
            search-mode selection, citing evidence). The disclosure is honest
            about the dependency — the guide is only reachable when the MCP
            server above is also on — so the user is never misled into thinking
            this alone opens anything. Hard-default-OFF, gated behind AI like the
            others, mutates only the draft, never cascades from the master.
          */}
          <ToggleRow
            checked={currentSettings.skillEnabled}
            describedById="ai-skill-disclosure"
            disabled={editorsDisabled}
            label={t('settings.aiSkillToggle')}
            onChange={() => flashOnSave(onToggleSkill())}
          />
          <div
            className="text-ink-muted m-0 flex flex-col items-start gap-1.5 font-sans text-[12px] leading-[1.5]"
            data-testid="ai-skill-disclosure"
            id="ai-skill-disclosure"
          >
            <p className="m-0">{t('settings.aiSkillToggleHelp')}</p>
            <p className="m-0">{t('settings.aiSkillToggleDependency')}</p>
          </div>
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
          knobs auto-save and flash the Saved chip on each landed write.
        */}
        <AiSearchTuningSection
          settings={currentSettings}
          disabled={editorsDisabled}
          onChange={(knob, value) =>
            flashOnSave(onSearchTuningChange(knob, value))
          }
          onReset={() => flashOnSave(onResetSearchTuning())}
        />

        {/*
          GPU heavy-tier + re-embed (W-AI-9 Sub-block D). Same collapsed-disclosure
          treatment + `editorsDisabled` gate as search tuning: visible-but-inert
          while AI is off so the user can see what GPU acceleration unlocks before
          opting in. The toggle auto-saves (flashing the Saved chip); the section
          is honest when this build cannot run Metal.
        */}
        <AiGpuSection
          settings={currentSettings}
          disabled={editorsDisabled}
          onToggleGpu={() => flashOnSave(onToggleGpu())}
        />

        <AiProviderEditorList
          addLabel={t('settings.aiAddLlmProvider')}
          apiKeys={aiApiKeys}
          disabled={editorsDisabled}
          formatLabel={probeLatencyLabel}
          presetLabel={t('settings.aiAddProviderPresetLabel')}
          presetLabels={presetLabels}
          onAdd={(format) => flashOnSave(onAddProvider('llm', format))}
          onApiKeyChange={onApiKeyChange}
          onClearKey={(providerId) => {
            void onClearAiApiKey(providerId)
          }}
          onClearKeyDisabled={onClearKeyDisabled}
          onCommit={() => flashOnSave(onCommitProviders())}
          onProbe={(providerId) => {
            void onProviderProbe('llm', providerId)
          }}
          onProbeDisabled={onProbeDisabled}
          onRemove={(providerId) =>
            flashOnSave(onRemoveProvider('llm', providerId))
          }
          onSaveKey={(providerId) => {
            void onSaveAiApiKey(providerId)
          }}
          onSaveKeyDisabled={onSaveKeyDisabled}
          onSelect={(providerId) =>
            flashOnSave(onSelectProvider('llm', providerId))
          }
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
          onAdd={(format) => flashOnSave(onAddProvider('embedding', format))}
          onApiKeyChange={onApiKeyChange}
          onClearKey={(providerId) => {
            void onClearAiApiKey(providerId)
          }}
          onClearKeyDisabled={onClearKeyDisabled}
          onCommit={() => flashOnSave(onCommitProviders())}
          onProbe={(providerId) => {
            void onProviderProbe('embedding', providerId)
          }}
          onProbeDisabled={onProbeDisabled}
          onRemove={(providerId) =>
            flashOnSave(onRemoveProvider('embedding', providerId))
          }
          onSaveKey={(providerId) => {
            void onSaveAiApiKey(providerId)
          }}
          onSaveKeyDisabled={onSaveKeyDisabled}
          onSelect={(providerId) =>
            flashOnSave(onSelectProvider('embedding', providerId))
          }
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
            {aiStatus.warning || aiStatus.warningCode ? (
              <div className="result-row">
                <div className="result-row__header">
                  <strong>{t('settings.aiIndexWarning')}</strong>
                </div>
                <p>
                  {/* Resolve the stable warning CODE (review-fix M-7) to localized copy for ALL
                      variants — never an English-sentence match. Fall back to the legacy English
                      `warning` only when an older payload carried no code (additive contract). */}
                  {aiStatus.warningCode
                    ? localizeAiIndexWarning(
                        aiStatus.warningCode,
                        (key, vars) => t(`settings.${key}`, vars),
                      )
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
