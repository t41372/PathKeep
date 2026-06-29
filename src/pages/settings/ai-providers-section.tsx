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

import { useEffect, useRef, useState } from 'react'
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
import { backend } from '../../lib/backend-client'
import {
  isModelDownloadInFlight,
  markModelDownloadSettled,
  markModelDownloadStarted,
  subscribeToModelDownloadProgress,
} from '../../lib/ipc/model-download'
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
  StaticEmbeddingStatus,
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
 * Provider ids that are built-in and must never be user-deletable or require an API key.
 * The static-in-app embedding tier is always in this list; external providers are never here.
 */
const STATIC_EMBEDDING_PROVIDER_IDS = ['static-in-app']

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

  // Bug 1: a Build-index CTA is offered in the index-health box only when a
  // from-scratch build would actually do something — i.e. an embedding provider
  // is configured AND the index is empty (no index yet) or stale. We deliberately
  // do NOT show it for any other state (ready/rebuilding/queued/paused/failed/
  // blocked/disabled): never nag when there is nothing to build (optional-AI), and
  // never duplicate the recovery path the warning box already drives. "Configured"
  // means a selected embedding provider that still exists in the draft.
  const embeddingProviderConfigured = currentSettings.embeddingProviders.some(
    (provider) => provider.id === currentSettings.embeddingProviderId,
  )
  const indexBuildActionable =
    embeddingProviderConfigured &&
    !!aiStatus &&
    (aiStatus.state === 'stale' ||
      // The empty state is whatever `aiStatusMeta`'s default branch maps — anything
      // that is not one of the known non-empty/non-stale lifecycle states.
      ![
        'ready',
        'rebuilding',
        'queued',
        'paused',
        'failed',
        'degraded',
        'blocked',
        'disabled',
      ].includes(aiStatus.state))

  // Show the Reset control only when the build is stuck in a terminal state that won't resolve on
  // its own. `failed` and `degraded` are the two states where a clear-and-rebuild is actionable.
  // `blocked` means a precondition is unsatisfied (e.g. archive not initialized) — clearing the
  // job won't help there, so we skip it. We always show it even without an embedding provider
  // configured, since the reset may be needed to clear a job from a now-removed provider.
  const indexResetActionable =
    !!aiStatus && (aiStatus.state === 'failed' || aiStatus.state === 'degraded')

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

        {/*
          Static embedding tier download panel. Shown when the backend reports
          a `staticEmbedding` status — i.e. the static-in-app provider is registered.
          The panel only shows the download affordance when the model is not yet
          present; once downloaded it shows a quiet "Ready" confirmation. It is
          consent-gated (the user must click "Download model") and never auto-downloads.
        */}
        {aiStatus?.staticEmbedding ? (
          <StaticEmbeddingPanel
            disabled={editorsDisabled}
            staticEmbedding={aiStatus.staticEmbedding}
            onSelect={(providerId) =>
              flashOnSave(onSelectProvider('embedding', providerId))
            }
            t={t}
          />
        ) : null}

        <AiProviderEditorList
          addLabel={t('settings.aiAddEmbeddingProvider')}
          apiKeys={aiApiKeys}
          builtInProviderIds={STATIC_EMBEDDING_PROVIDER_IDS}
          builtInBadgeLabel={t('settings.aiStaticModelBuiltInBadge')}
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
              // `aiStatusMeta` builds the description in the intelligence namespace,
              // but for any state that carries a stable warning CODE it falls back to
              // the backend's raw ENGLISH `warning` sentence (e.g. the empty / not-built
              // state), which would leak English into the zh UI. A coded warning already
              // renders, localized, in the dedicated "Current index warning" box below —
              // so DON'T repeat it in the callout body (that double-printed the same
              // sentence). For a coded state the title + tone convey it and the box has
              // the detail; only the code-free states (ready/disabled) show the
              // already-localized `indexMeta.description`.
              body={aiStatus.warningCode ? undefined : indexMeta.description}
            />
            {/*
              Build-index CTA right where the user just configured AI (Bug 1). The
              actual build flow also lives in the collapsed GPU section, but a user
              who just added an embedding provider never finds it there — so offer
              it here whenever a from-scratch build is actionable. This is an
              explicit, expensive action (a full backfill), so it stays a button,
              NOT auto-save. It is hidden entirely when no embedding provider is
              configured (optional-AI: never nag), so it only appears once building
              the index would actually do something.
            */}
            {indexBuildActionable ? (
              <IndexBuildButton
                idleLabel={t('settings.aiIndexBuildCta')}
                buildingLabel={t('settings.aiIndexBuildingCta')}
                doneLabel={t('settings.aiIndexBuildQueued')}
                errorLabel={t('settings.aiIndexBuildError')}
              />
            ) : null}
            {/*
              Reset control for stuck/failed/degraded build states (M-8 honest recovery). Only shown
              when the build is in a terminal non-recoverable state on its own so the user has a
              clear "clear and rebuild" path without hunting through the GPU section. Not shown for
              states that will resolve on their own (queued, running, paused) or states that have no
              build to clear (disabled, empty, stale → use Build index CTA instead).
            */}
            {indexResetActionable ? (
              <IndexResetButton
                idleLabel={t('settings.aiResetIndexBuildAction')}
                confirmPrompt={t('settings.aiResetIndexBuildConfirmPrompt')}
                confirmYesLabel={t('settings.aiResetIndexBuildConfirmYes')}
                confirmNoLabel={t('settings.aiResetIndexBuildConfirmNo')}
                resettingLabel={t('settings.aiResetIndexBuildResetting')}
                doneLabel={t('settings.aiResetIndexBuildQueued')}
                errorLabel={t('settings.aiResetIndexBuildError')}
              />
            ) : null}
            <div className="settings-field-grid">
              <div className="config-row">
                <span className="config-label">
                  {t('settings.aiIndexedRows')}
                </span>
                <span className="config-value mono">
                  {aiStatus.indexedItems.toLocaleString(language)}
                </span>
              </div>
              {/*
                Show the REAL vector count whenever the backend reports it (additive field). This
                is the only truthful measure of whether smart search will actually work: metadata
                rows count SQL insertions; `semanticVectorCount` counts the HNSW vectors actually
                written to the sidecar. When they diverge (degraded state), the user sees that 0
                vectors were written even though N pages appear "indexed" in metadata.
              */}
              {aiStatus.semanticVectorCount != null ? (
                <div
                  className="config-row"
                  data-testid="ai-semantic-vector-count-row"
                >
                  <span className="config-label">
                    {t('settings.aiSemanticVectors')}
                  </span>
                  <span className="config-value mono">
                    {aiStatus.semanticVectorCount.toLocaleString(language)}
                  </span>
                </div>
              ) : null}
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
      </PaperCardBody>
    </PaperCard>
  )
}

// ─── IndexBuildButton ────────────────────────────────────────────────────────

type BuildState = 'idle' | 'building' | 'done' | 'error'

/**
 * Explicit "Build index" action for the index-health box (Bug 1).
 *
 * `backend.buildAiIndex` only ENQUEUES a background full backfill (the real work
 * runs on the worker), so this fires the from-scratch full build — the same
 * request shape the GPU section uses for a full re-embed (`fullRebuild: true`,
 * `clearOnly: false`, `scope: 'full'`) — then settles to a "queued" confirmation
 * rather than claiming the (minutes-long) index is already built. The button is
 * disabled while the enqueue is in flight so a double-click can't double-enqueue,
 * and a failed enqueue shows an honest, retry-able error instead of a dead button.
 */
function IndexBuildButton({
  idleLabel,
  buildingLabel,
  doneLabel,
  errorLabel,
}: {
  idleLabel: string
  buildingLabel: string
  doneLabel: string
  errorLabel: string
}) {
  const [state, setState] = useState<BuildState>('idle')

  const onBuild = () => {
    setState('building')
    backend
      .buildAiIndex({ fullRebuild: true, clearOnly: false, scope: 'full' })
      .then(() => setState('done'))
      .catch(() => setState('error'))
  }

  const building = state === 'building'
  const label = building ? buildingLabel : idleLabel

  return (
    <div className="flex flex-col items-start gap-1.5">
      <button
        type="button"
        className="btn-secondary self-start"
        disabled={building}
        onClick={onBuild}
        data-testid="ai-index-build"
      >
        {label}
      </button>
      {state === 'done' ? (
        <p
          aria-live="polite"
          className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]"
          data-testid="ai-index-build-queued"
        >
          {doneLabel}
        </p>
      ) : null}
      {state === 'error' ? (
        <p
          aria-live="polite"
          className="text-ink-faint m-0 font-sans text-[12px] leading-[1.5] italic"
          data-testid="ai-index-build-error"
        >
          {errorLabel}
        </p>
      ) : null}
    </div>
  )
}

// ─── IndexResetButton ─────────────────────────────────────────────────────────

type ResetState = 'idle' | 'confirming' | 'resetting' | 'done' | 'error'

/**
 * "Clear stuck build & rebuild" recovery action for the index-health box.
 *
 * Calls `reset_ai_index_build` which clears the terminal failed/degraded job and re-enqueues
 * a clean incremental build. A two-step confirm is required before the write fires because
 * clearing discards partially-written index data. The button is only rendered when the index
 * state is `failed` or `degraded` (the `indexResetActionable` gate in the section), so it
 * is never a nag — it only surfaces when there is actually something stuck to clear.
 */
function IndexResetButton({
  idleLabel,
  confirmPrompt,
  confirmYesLabel,
  confirmNoLabel,
  resettingLabel,
  doneLabel,
  errorLabel,
}: {
  idleLabel: string
  confirmPrompt: string
  confirmYesLabel: string
  confirmNoLabel: string
  resettingLabel: string
  doneLabel: string
  errorLabel: string
}) {
  const [state, setState] = useState<ResetState>('idle')

  const onConfirm = () => {
    setState('resetting')
    backend
      .resetAiIndexBuild()
      .then(() => setState('done'))
      .catch(() => setState('error'))
  }

  if (state === 'idle') {
    return (
      <div className="flex flex-col items-start gap-1.5">
        <button
          type="button"
          className="btn-secondary self-start"
          data-testid="ai-index-reset"
          onClick={() => setState('confirming')}
        >
          {idleLabel}
        </button>
      </div>
    )
  }

  if (state === 'confirming') {
    return (
      <div
        className="flex flex-col items-start gap-2"
        data-testid="ai-index-reset-confirm"
      >
        <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
          {confirmPrompt}
        </p>
        <div className="flex gap-2">
          <button
            type="button"
            className="btn-secondary self-start"
            data-testid="ai-index-reset-confirm-yes"
            onClick={onConfirm}
          >
            {confirmYesLabel}
          </button>
          <button
            type="button"
            className="btn-ghost self-start"
            data-testid="ai-index-reset-confirm-no"
            onClick={() => setState('idle')}
          >
            {confirmNoLabel}
          </button>
        </div>
      </div>
    )
  }

  if (state === 'resetting') {
    return (
      <div className="flex flex-col items-start gap-1.5">
        <button
          type="button"
          className="btn-secondary self-start"
          data-testid="ai-index-reset"
          disabled
        >
          {resettingLabel}
        </button>
      </div>
    )
  }

  if (state === 'done') {
    return (
      <p
        aria-live="polite"
        className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]"
        data-testid="ai-index-reset-queued"
      >
        {doneLabel}
      </p>
    )
  }

  // error state
  return (
    <div className="flex flex-col items-start gap-1.5">
      <button
        type="button"
        className="btn-secondary self-start"
        data-testid="ai-index-reset"
        onClick={() => setState('confirming')}
      >
        {idleLabel}
      </button>
      <p
        aria-live="polite"
        className="text-ink-faint m-0 font-sans text-[12px] leading-[1.5] italic"
        data-testid="ai-index-reset-error"
      >
        {errorLabel}
      </p>
    </div>
  )
}

// ─── StaticEmbeddingPanel ─────────────────────────────────────────────────────

type StaticDownloadDisplayState =
  | 'not-downloaded'
  | 'downloading'
  | 'ready'
  | 'failed'

/**
 * Download consent + status + selection panel for the built-in static embedding model.
 *
 * The static tier runs fully on-device and needs no external server or API key. The panel:
 *   - shows whether the static tier is the ACTIVE vector model, and offers a one-click "Use this
 *     model" when it is not, so a user stuck on a broken external provider can switch here without
 *     hunting for the radio (the select is always offered, even if the provider row is missing —
 *     never leave the unstuck path dead);
 *   - when the weights are absent, shows a consent-gated "Download model" button that calls
 *     `download_static_embedding_model`, then subscribes to `pathkeep://model-download-progress`
 *     for live per-file progress and a Cancel control (`cancel_ai_embedding_model_download`).
 *
 * Honesty + robustness:
 *   - `ready` is derived from the prop (`modelDownloaded`), the single source of truth.
 *   - A process-global in-flight latch (`lib/ipc/model-download`) survives a remount or a snapshot
 *     poll that momentarily drops `staticEmbedding`, so the Download button cannot silently
 *     re-trigger an in-flight download.
 *   - The backend stream is per-file with unknown sizes, so we show the current file + spinner,
 *     never a fabricated percentage.
 *
 * State machine:
 *   - `not-downloaded` → idle, shows download button
 *   - `downloading`    → download in flight, shows spinner + current file + cancel
 *   - `ready`          → model present (from prop), shows confirmation
 *   - `failed`         → download errored, shows error + retry
 */
function StaticEmbeddingPanel({
  disabled,
  staticEmbedding,
  onSelect,
  t,
}: {
  disabled: boolean
  staticEmbedding: StaticEmbeddingStatus
  onSelect: (providerId: string) => void
  t: (key: string, vars?: Record<string, string | number>) => string
}) {
  // Local download tracking. Initialized from the global latch so a remount mid-download shows
  // "downloading" (and a disabled action) immediately, before the next progress event arrives.
  const [localState, setLocalState] = useState<
    'not-downloaded' | 'downloading' | 'failed'
  >(isModelDownloadInFlight() ? 'downloading' : 'not-downloaded')
  const [currentFile, setCurrentFile] = useState<string | null>(null)
  // Set when the user cancels: the backend aborts the download with a terminal `error`, which we
  // then treat as a clean stop (return to idle) rather than a scary failure.
  const cancelledRef = useRef(false)

  // Subscribe to live per-file progress while the model is not yet present. The event stream is the
  // durable "downloading" signal: a remount mid-download recovers as soon as the next event lands,
  // and the terminal done/error clears the global in-flight latch.
  useEffect(() => {
    if (staticEmbedding.modelDownloaded) return
    let active = true
    let unsub = () => {}
    void subscribeToModelDownloadProgress((event) => {
      switch (event.kind) {
        case 'fileStarted':
          markModelDownloadStarted()
          setLocalState('downloading')
          setCurrentFile(event.file)
          break
        case 'fileFinished':
          setLocalState('downloading')
          setCurrentFile(null)
          break
        case 'done':
          // Keep the spinner until the next snapshot flips `modelDownloaded`, so we never flash
          // "not downloaded" between the done event and the poll that confirms the weights.
          markModelDownloadSettled()
          setCurrentFile(null)
          break
        case 'error':
          markModelDownloadSettled()
          setCurrentFile(null)
          setLocalState(cancelledRef.current ? 'not-downloaded' : 'failed')
          cancelledRef.current = false
          break
      }
    }).then((fn) => {
      if (active) unsub = fn
      else fn()
    })
    return () => {
      active = false
      unsub()
    }
  }, [staticEmbedding.modelDownloaded])

  // The prop is the source of truth for "ready"; otherwise the local download state drives display.
  const displayState: StaticDownloadDisplayState =
    staticEmbedding.modelDownloaded ? 'ready' : localState

  const onDownload = () => {
    cancelledRef.current = false
    markModelDownloadStarted()
    setLocalState('downloading')
    setCurrentFile(null)
    // The command returns once the background thread is spawned; the actual outcome arrives on the
    // progress channel. A rejection here means the command itself could not start.
    backend.downloadStaticEmbeddingModel().catch(() => {
      markModelDownloadSettled()
      setLocalState('failed')
    })
  }

  const onCancelDownload = () => {
    cancelledRef.current = true
    markModelDownloadSettled()
    setLocalState('not-downloaded')
    setCurrentFile(null)
    void backend.cancelStaticEmbeddingModelDownload().catch(() => {})
  }

  return (
    <div
      className="surfaceInset flex flex-col gap-2 p-3"
      data-testid="ai-static-embedding-panel"
    >
      <div className="flex items-start gap-2">
        <strong className="font-sans text-[13px]">
          {t('settings.aiStaticModelTitle')}
        </strong>
        <span
          aria-live="polite"
          className="text-ink-muted font-sans text-[11px]"
          data-testid="ai-static-model-status"
        >
          {displayState === 'ready'
            ? t('settings.aiStaticModelReady')
            : displayState === 'downloading'
              ? t('settings.aiStaticModelDownloading')
              : displayState === 'failed'
                ? t('settings.aiStaticModelDownloadFailed')
                : t('settings.aiStaticModelNotDownloaded')}
        </span>
      </div>
      <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
        {t('settings.aiStaticModelDescription')}
      </p>

      <div className="flex flex-wrap items-center gap-2">
        {staticEmbedding.selected ? (
          <span
            className="providerBuiltInBadge"
            data-testid="ai-static-model-active"
          >
            {t('settings.aiStaticModelActive')}
          </span>
        ) : (
          <>
            <span className="text-ink-muted font-sans text-[12px]">
              {t('settings.aiStaticModelRecommendedNotSelected')}
            </span>
            <button
              type="button"
              className="btn-secondary"
              disabled={disabled}
              data-testid="ai-static-model-select"
              onClick={() => onSelect(staticEmbedding.providerId)}
            >
              {t('settings.aiStaticModelSelectAction')}
            </button>
          </>
        )}
      </div>

      {displayState === 'downloading' ? (
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2">
            <span
              className="inlineSpinner"
              aria-hidden="true"
              data-testid="ai-static-model-downloading"
            >
              <span className="inlineSpinner__dot" />
              <span className="inlineSpinner__dot" />
              <span className="inlineSpinner__dot" />
            </span>
            <button
              type="button"
              className="btn-ghost"
              data-testid="ai-static-model-cancel"
              onClick={onCancelDownload}
            >
              {t('settings.aiStaticModelCancelDownload')}
            </button>
          </div>
          {currentFile ? (
            <p
              className="text-ink-faint m-0 font-sans text-[11px] leading-[1.4]"
              data-testid="ai-static-model-current-file"
            >
              {t('settings.aiStaticModelDownloadingFile', {
                file: currentFile,
              })}
            </p>
          ) : null}
        </div>
      ) : displayState === 'not-downloaded' || displayState === 'failed' ? (
        <button
          type="button"
          className="btn-secondary self-start"
          disabled={disabled}
          data-testid="ai-static-model-download"
          onClick={onDownload}
        >
          {t('settings.aiStaticModelDownloadAction')}
        </button>
      ) : null}
    </div>
  )
}
