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

import { useRef, useState } from 'react'
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
import { Button } from '@/components/ui/button'
import { StatusCallout } from '../../components/primitives/status-callout'
import { ToggleRow } from '../../components/ui'
import { localizeAiIndexWarning } from '../../lib/ai/note-codes'
import { backend } from '../../lib/backend-client'
import {
  isModelDownloadInFlight,
  markModelDownloadSettled,
  markModelDownloadStarted,
  useModelDownloadProgress,
  type ModelDownloadProgress,
} from '../../lib/ipc/model-download'
import { formatBytes } from '../../lib/format'
import { BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID } from '../../lib/types'
import { useI18n, type ResolvedLanguage } from '../../lib/i18n'
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
const STATIC_EMBEDDING_PROVIDER_IDS = [BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID]

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
          Two-tier vector model section.

          BASE TIER: the always-on on-device static model — shown first because it is the
          default starting point and requires no external server or API key. Only rendered
          when the backend reports `staticEmbedding` (i.e. the static-in-app provider is
          registered). The static provider is intentionally NOT rendered in the editable
          external list below so the user never mistakes it for a peer alternative.

          UPGRADE TIER: external embedding providers the user can add, configure, and select
          when they need higher precision. The static provider is filtered out of this list.
        */}
        <div
          className="flex flex-col gap-3"
          data-testid="ai-vector-model-section"
        >
          {/* Section heading with bottom rule */}
          <h4 className="m-0 border-b border-ink-faint pb-1.5 font-sans text-[12px] font-semibold uppercase tracking-[0.08em] text-ink-muted">
            {t('settings.aiVectorModelSectionTitle')}
          </h4>

          {/* BASE TIER ─────────────────────────────────────────────────────── */}
          {/* Label + panel + connector render together: when the static tier can't be resolved
              (transient aiStatus load window, or static_embedding_status -> None) showing a "BASE
              TIER" heading + a connector that references a panel that isn't there is the kind of
              orphaned scaffolding this redesign is removing. */}
          {aiStatus?.staticEmbedding ? (
            <>
              <h5 className="m-0 font-sans text-[11px] font-semibold uppercase tracking-[0.08em] text-ink-muted">
                {t('settings.aiBaseTierLabel')}
              </h5>
              <BaseTierPanel
                disabled={editorsDisabled}
                language={language}
                staticEmbedding={aiStatus.staticEmbedding}
                onSelect={(providerId) =>
                  flashOnSave(onSelectProvider('embedding', providerId))
                }
                t={t}
              />
              {/* Tier connector */}
              <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5] italic">
                {t('settings.aiTierConnectorText')}
              </p>
            </>
          ) : null}

          {/* UPGRADE TIER ───────────────────────────────────────────────────── */}
          <h5 className="m-0 font-sans text-[11px] font-semibold uppercase tracking-[0.08em] text-ink-muted">
            {t('settings.aiUpgradeTierLabel')}
          </h5>
          <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
            {t('settings.aiUpgradeTierBody')}
          </p>

          <AiProviderEditorList
            addLabel={t('settings.aiAddExternalEmbeddingProvider')}
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
            providers={currentSettings.embeddingProviders.filter(
              (p) => !STATIC_EMBEDDING_PROVIDER_IDS.includes(p.id),
            )}
            purpose="embedding"
            selectedProviderId={currentSettings.embeddingProviderId ?? null}
            testingProviderId={testingProviderId}
            title=""
            translations={providerTranslations}
          />
        </div>

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
      <Button
        type="button"
        variant="outline"
        className="self-start"
        disabled={building}
        onClick={onBuild}
        data-testid="ai-index-build"
      >
        {label}
      </Button>
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
        <Button
          type="button"
          variant="outline"
          className="self-start"
          data-testid="ai-index-reset"
          onClick={() => setState('confirming')}
        >
          {idleLabel}
        </Button>
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
          <Button
            type="button"
            variant="outline"
            className="self-start"
            data-testid="ai-index-reset-confirm-yes"
            onClick={onConfirm}
          >
            {confirmYesLabel}
          </Button>
          <Button
            type="button"
            variant="ghost"
            className="self-start"
            data-testid="ai-index-reset-confirm-no"
            onClick={() => setState('idle')}
          >
            {confirmNoLabel}
          </Button>
        </div>
      </div>
    )
  }

  if (state === 'resetting') {
    return (
      <div className="flex flex-col items-start gap-1.5">
        <Button
          type="button"
          variant="outline"
          className="self-start"
          data-testid="ai-index-reset"
          disabled
        >
          {resettingLabel}
        </Button>
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
      <Button
        type="button"
        variant="outline"
        className="self-start"
        data-testid="ai-index-reset"
        onClick={() => setState('confirming')}
      >
        {idleLabel}
      </Button>
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

// ─── BaseTierPanel ────────────────────────────────────────────────────────────

/**
 * Dedicated inset panel for the built-in on-device base-tier embedding model.
 *
 * This is NOT an editable provider card — the static model has no user-configurable fields.
 * Instead it shows:
 *   - A header row with the model name and (when selected) an Active pill.
 *   - A description emphasizing lower precision versus large external models.
 *   - A specs line showing real dimensions (from the backend) and estimated size.
 *   - Honest download state:
 *     - idle     → "Download model" button + size hint.
 *     - downloading → real byte progress bar (determinate or indeterminate shimmer),
 *                  current file basename, restart-if-quit note, Cancel button.
 *     - ready    → "✓ Ready" + "Use built-in model" button when not selected.
 *     - failed   → error message + "Retry download" button.
 *
 * Phase is driven by `useModelDownloadProgress` (event-based bytes + latch) together with
 * small local overrides for the "just clicked Download" gap and "command itself failed" cases.
 */
function BaseTierPanel({
  disabled,
  language,
  staticEmbedding,
  onSelect,
  t,
}: {
  disabled: boolean
  language: ResolvedLanguage
  staticEmbedding: StaticEmbeddingStatus
  onSelect: (providerId: string) => void
  t: (key: string, vars?: Record<string, string | number>) => string
}) {
  // cancelledRef: set to true before the cancel IPC fires so the hook maps the terminal
  // error to 'idle' rather than 'failed'. Must be a plain object (not ReadOnly) so the hook
  // and component share the same writable reference.
  const cancelledRef = useRef<boolean>(false)

  // Bumped on every Download/Retry click so the hook clears any prior terminal state at once —
  // otherwise a retry after a mid-download failure shows a stale "Download failed" until the first
  // progress event arrives (exactly the flaky-network window that caused the original failure).
  const [restartNonce, setRestartNonce] = useState(0)

  // Event-driven phase + byte progress from the shared hook.
  const progress = useModelDownloadProgress(
    staticEmbedding.modelDownloaded,
    cancelledRef,
    restartNonce,
  )

  // Local override for immediate UI transitions (before the first subscription event arrives):
  // - 'downloading': set immediately on Download click (before fileStarted lands).
  // - 'idle':        set immediately on Cancel click (before the terminal error lands).
  // - 'failed':      set when the download *command* itself rejects (before any event fires).
  // null = defer to the hook's phase.
  const [localPhase, setLocalPhase] = useState<
    'idle' | 'downloading' | 'failed' | null
  >(() => (isModelDownloadInFlight() ? 'downloading' : null))

  // Combined phase: 'idle'/'failed' local overrides take priority; hook phase wins when
  // non-idle (i.e. events have arrived); 'downloading' local override covers the brief
  // gap before the first fileStarted event.
  const phase: ModelDownloadProgress['phase'] =
    localPhase === 'idle'
      ? 'idle'
      : localPhase === 'failed'
        ? 'failed'
        : progress.phase !== 'idle'
          ? progress.phase
          : localPhase === 'downloading'
            ? 'downloading'
            : 'idle'

  const onDownload = () => {
    cancelledRef.current = false
    setLocalPhase('downloading')
    // Reset the hook's phase so a retry after a failure never flashes the stale "failed" state.
    setRestartNonce((n) => n + 1)
    markModelDownloadStarted()
    // The command spawns the background thread and returns immediately; actual progress
    // arrives on the `pathkeep://model-download-progress` channel via the hook.
    backend.downloadStaticEmbeddingModel().catch(() => {
      markModelDownloadSettled()
      setLocalPhase('failed')
    })
  }

  const onCancelDownload = () => {
    cancelledRef.current = true
    // Immediately show idle — the terminal error event will confirm it via the hook.
    setLocalPhase('idle')
    markModelDownloadSettled()
    void backend.cancelStaticEmbeddingModelDownload().catch(() => {})
  }

  // Size label: real on-disk bytes when available, else a constant pre-download hint.
  const sizeLabel =
    staticEmbedding.modelSizeBytes && staticEmbedding.modelSizeBytes > 0
      ? formatBytes(staticEmbedding.modelSizeBytes, language)
      : '96 MB'
  const dimensions = staticEmbedding.dimensions ?? 256

  const isDeterminate = progress.totalBytes > 0
  const progressPct = isDeterminate
    ? Math.min(100, (progress.downloadedBytes / progress.totalBytes) * 100)
    : 0

  return (
    <div
      className="surfaceInset flex flex-col gap-2 p-3"
      data-testid="ai-static-embedding-panel"
    >
      {/* Header: title + Active pill */}
      <div className="flex flex-wrap items-center gap-2">
        <strong className="font-sans text-[13px]">
          {t('settings.aiBaseTierPanelTitle')}
        </strong>
        {staticEmbedding.selected ? (
          <span
            className="inline-flex items-center gap-1 rounded-full bg-green-100 px-2 py-0.5 font-sans text-[11px] font-medium text-green-700"
            data-testid="ai-static-model-active"
          >
            <span aria-hidden="true">●</span>
            {t('settings.aiBaseTierActivePill')}
          </span>
        ) : null}
      </div>

      {/* Description — "lower precision" is stated plainly, not softened */}
      <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
        {t('settings.aiBaseTierDescription')}
      </p>

      {/* Specs: real dimensions from the backend + size estimate */}
      <p className="text-ink-muted m-0 font-mono text-[11px] leading-[1.4]">
        {t('settings.aiBaseTierSpecs', { dimensions, size: sizeLabel })}
      </p>

      {/* Download state machine ─────────────────────────────────────────── */}
      {phase === 'idle' ? (
        <div className="flex flex-col gap-1.5">
          <Button
            type="button"
            variant="outline"
            className="self-start"
            disabled={disabled}
            data-testid="ai-static-model-download"
            onClick={onDownload}
          >
            {t('settings.aiBaseTierDownloadButton')}
          </Button>
          <p className="text-ink-muted m-0 font-sans text-[11px] leading-[1.4]">
            {t('settings.aiBaseTierDownloadHint', { size: sizeLabel })}
          </p>
        </div>
      ) : phase === 'downloading' ? (
        <div className="flex flex-col gap-2">
          {/* Real byte progress bar (determinate) or indeterminate shimmer */}
          {isDeterminate ? (
            <>
              <div
                role="progressbar"
                aria-valuenow={Math.min(
                  progress.downloadedBytes,
                  progress.totalBytes,
                )}
                aria-valuemin={0}
                aria-valuemax={progress.totalBytes}
                aria-label={t('settings.aiBaseTierDownloadButton')}
                className="h-[6px] w-full overflow-hidden rounded-full bg-ink-faint"
                data-testid="ai-base-tier-progress-bar"
              >
                <div
                  className="h-full bg-ink-accent transition-[width] motion-reduce:transition-none"
                  style={{ width: `${progressPct}%` }}
                />
              </div>
              <p
                className="text-ink-muted m-0 font-sans text-[11px] leading-[1.4]"
                data-testid="ai-base-tier-progress-label"
              >
                {formatBytes(progress.downloadedBytes, language)} /{' '}
                {formatBytes(progress.totalBytes, language)}
              </p>
            </>
          ) : (
            <div
              role="progressbar"
              aria-busy="true"
              aria-label={t('settings.aiBaseTierDownloadButton')}
              className="h-[6px] w-full overflow-hidden rounded-full bg-ink-faint"
              data-testid="ai-base-tier-progress-bar"
            >
              {/* Indeterminate: the shared canonical sweep (pk-indeterminate-bar) — the same
                  treatment every other progress bar in the app uses, so the model download looks
                  identical here and on the Activity page. Under reduced motion the class falls back
                  to a static partial fill, so a sighted reduced-motion user still sees activity. */}
              <div className="h-full bg-ink-accent pk-indeterminate-bar" />
            </div>
          )}
          {/* Current file (basename only, mono) */}
          {progress.currentFile ? (
            <p
              className="text-ink-faint m-0 font-mono text-[11px] leading-[1.4]"
              data-testid="ai-static-model-current-file"
            >
              {progress.currentFile}
            </p>
          ) : null}
          {/* Restart-if-quit warning */}
          <p
            className="text-tone-warning m-0 font-sans text-[11px] leading-[1.4]"
            data-testid="ai-base-tier-restart-note"
          >
            {'⚠ '}
            {t('settings.aiBaseTierDownloadRestartNote')}
          </p>
          <Button
            type="button"
            variant="ghost"
            className="self-start"
            data-testid="ai-static-model-cancel"
            onClick={onCancelDownload}
          >
            {t('settings.aiBaseTierCancelButton')}
          </Button>
        </div>
      ) : phase === 'ready' ? (
        <div className="flex flex-wrap items-center gap-2">
          <span
            className="text-ink-muted font-sans text-[12px]"
            data-testid="ai-base-tier-ready"
          >
            {t('settings.aiBaseTierReadyText')}
          </span>
          {!staticEmbedding.selected ? (
            <Button
              type="button"
              variant="outline"
              disabled={disabled}
              data-testid="ai-static-model-select"
              onClick={() => onSelect(staticEmbedding.providerId)}
            >
              {t('settings.aiBaseTierUseButton')}
            </Button>
          ) : null}
        </div>
      ) : (
        /* failed */
        <div className="flex flex-col gap-1.5">
          <p
            aria-live="polite"
            className="text-ink-faint m-0 font-sans text-[12px] leading-[1.5]"
            data-testid="ai-base-tier-download-failed"
          >
            {t('settings.aiBaseTierDownloadFailed')}
          </p>
          <Button
            type="button"
            variant="outline"
            className="self-start"
            disabled={disabled}
            data-testid="ai-static-model-download"
            onClick={onDownload}
          >
            {t('settings.aiBaseTierRetryButton')}
          </Button>
        </div>
      )}
    </div>
  )
}
