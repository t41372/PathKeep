/**
 * @file use-settings-ai-state.ts
 * @description Owns the Settings AI provider draft, API key, and integration-preview workflows.
 * @module pages/settings
 *
 * ## 職責
 * - 管理 AI provider draft、API key review、與 integration preview。
 * - 集中 AI config save/reset 與 provider CRUD handlers。
 * - 對 AI section 提供已本地化的 integration preview 和 index-health metadata。
 *
 * ## 不負責
 * - 不渲染 AI section UI。
 * - 不管理 derived runtime queue。
 * - 不決定 MCP / semantic backend contract。
 *
 * ## 依賴關係
 * - 依賴 shell snapshot、Settings helpers、backend client 與 i18n。
 *
 * ## 性能備注
 * - 只在 snapshot AI config 變化時同步 draft，integration preview 也只跟著 config signature 重新取得。
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import {
  copyReviewValue,
  type ReviewCopyFeedback,
} from '../../components/review'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { useI18n } from '../../lib/i18n'
import { aiStatusMeta } from '../../lib/intelligence-ai-presentation'
import type {
  AiIntegrationPreview,
  AiProviderConfig,
  AiProviderConnectionTestReport,
  AiProviderPurpose,
  AiRequestFormat,
  AiSettings,
  AppConfig,
  AppSnapshot,
} from '../../lib/types'
import {
  appendAiProviderDraft,
  cloneAiSettings,
  localizeAiIntegrationPreview,
  makeDefaultAiProviderDraft,
  mergeAiProviderSecretState,
  patchAiProviderDraft,
  removeAiProviderDraft,
  selectAiProviderDraft,
  serializeAiSettings,
} from './helpers'
import {
  type SearchTuningKnob,
  applySearchTuningKnob,
  resetSearchTuningKnobs,
} from './search-tuning-helpers'

interface UseSettingsAiStateArgs {
  enableIntegrationPreview?: boolean
  refreshAppData: () => Promise<void>
  saveConfig: (config: AppConfig) => Promise<AppSnapshot>
  snapshot: AppSnapshot | null
}

/**
 * Keeps Settings AI draft state and integration preview under one focused hook.
 */
export function useSettingsAiState({
  enableIntegrationPreview = true,
  refreshAppData,
  saveConfig,
  snapshot,
}: UseSettingsAiStateArgs) {
  const { t, ns } = useI18n()
  const settingsNs = ns('settings')
  const intelligenceT = ns('intelligence')
  const [saving, setSaving] = useState(false)
  const [aiDraft, setAiDraft] = useState<AiSettings | null>(() =>
    snapshot?.config.ai ? cloneAiSettings(snapshot.config.ai) : null,
  )
  const [aiApiKeys, setAiApiKeys] = useState<Record<string, string>>({})
  // Per-provider connection-probe state. Keyed by provider id so each editor
  // card shows only its own result; `testing` tracks which probe is in flight so
  // a single button can disable + relabel without freezing the others.
  const [aiProviderProbes, setAiProviderProbes] = useState<
    Record<string, AiProviderConnectionTestReport>
  >({})
  const [aiTestingProviderId, setAiTestingProviderId] = useState<string | null>(
    null,
  )
  const [aiIntegrationPreview, setAiIntegrationPreview] =
    useState<AiIntegrationPreview | null>(null)
  const [aiIntegrationError, setAiIntegrationError] = useState<string | null>(
    null,
  )
  const [aiIntegrationCopyFeedback, setAiIntegrationCopyFeedback] =
    useState<ReviewCopyFeedback | null>(null)
  const lastSyncedAiSignatureRef = useRef<string | null>(null)
  const savedAiSettings = snapshot?.config.ai ?? null
  const snapshotAiSignature = useMemo(
    () => serializeAiSettings(savedAiSettings),
    [savedAiSettings],
  )
  const currentAiSettings = aiDraft ?? savedAiSettings
  const localizedAiIntegrationPreview = useMemo(
    () =>
      aiIntegrationPreview
        ? localizeAiIntegrationPreview(aiIntegrationPreview, settingsNs)
        : null,
    [aiIntegrationPreview, settingsNs],
  )
  const aiIndexMeta = snapshot
    ? aiStatusMeta(snapshot.aiStatus, intelligenceT)
    : null
  const aiConfigDirty =
    snapshotAiSignature !== null &&
    currentAiSettings !== null &&
    serializeAiSettings(currentAiSettings) !== snapshotAiSignature
  const persistedProviderIds = useMemo(
    () =>
      new Set(
        [
          ...(snapshot?.config.ai.llmProviders ?? []),
          ...(snapshot?.config.ai.embeddingProviders ?? []),
        ].map((provider) => provider.id),
      ),
    [snapshot?.config.ai.embeddingProviders, snapshot?.config.ai.llmProviders],
  )
  const noAiProviders =
    (currentAiSettings?.llmProviders.length ?? 0) === 0 &&
    (currentAiSettings?.embeddingProviders.length ?? 0) === 0

  useEffect(() => {
    if (!savedAiSettings || snapshotAiSignature === null) {
      return
    }

    const draftSignature = serializeAiSettings(aiDraft)
    const draftMatchesSnapshot = draftSignature === snapshotAiSignature
    const shouldSync =
      aiDraft === null ||
      draftMatchesSnapshot ||
      draftSignature === lastSyncedAiSignatureRef.current

    if (shouldSync && !draftMatchesSnapshot) {
      setAiDraft(cloneAiSettings(savedAiSettings))
    }

    if (shouldSync) {
      lastSyncedAiSignatureRef.current = snapshotAiSignature
    }
  }, [aiDraft, savedAiSettings, snapshotAiSignature])

  useEffect(() => {
    if (!enableIntegrationPreview) {
      setAiIntegrationPreview(null)
      setAiIntegrationError(null)
      setAiIntegrationCopyFeedback(null)
      return
    }

    if (snapshotAiSignature === null) {
      return
    }

    let cancelled = false

    const loadPreview = async () => {
      try {
        const preview = await backend.previewAiIntegrations()
        if (!cancelled) {
          setAiIntegrationPreview(preview)
          setAiIntegrationError(null)
          setAiIntegrationCopyFeedback(null)
        }
      } catch (error) {
        if (!cancelled) {
          setAiIntegrationPreview(null)
          setAiIntegrationError(describeError(error, 'preview_ai_integrations'))
          setAiIntegrationCopyFeedback(null)
        }
      }
    }

    void loadPreview()
    return () => {
      cancelled = true
    }
  }, [enableIntegrationPreview, snapshotAiSignature, t])

  function updateAiDraft(updater: (current: AiSettings) => AiSettings) {
    if (!aiDraft) {
      return
    }

    setAiDraft((current) => updater(current as AiSettings))
  }

  function syncAiDraft(settings: AiSettings) {
    const nextDraft = cloneAiSettings(settings)
    setAiDraft(nextDraft)
    lastSyncedAiSignatureRef.current = serializeAiSettings(nextDraft)
  }

  function updateAiProviderSecretState(
    providerId: string,
    apiKeySaved: boolean,
  ) {
    updateAiDraft((current) =>
      mergeAiProviderSecretState(current, providerId, apiKeySaved),
    )
  }

  function handleAiToggle() {
    updateAiDraft((current) => ({
      ...current,
      enabled: !current.enabled,
    }))
  }

  // Granular consent: the assistant and semantic-search capabilities each have
  // their own opt-in below the master switch. They mutate ONLY the draft (the
  // master `enabled` is intentionally NOT cascaded) and never auto-start work —
  // semantic search still requires an explicit one-time index build elsewhere.
  function handleAssistantToggle() {
    updateAiDraft((current) => ({
      ...current,
      assistantEnabled: !current.assistantEnabled,
    }))
  }

  function handleSemanticIndexToggle() {
    updateAiDraft((current) => ({
      ...current,
      semanticIndexEnabled: !current.semanticIndexEnabled,
    }))
  }

  // Outward data-surface consent (W-AI-9 Sub-block B): turning this on lets the
  // worker expose a localhost-only, stdio MCP server so external AI tools you
  // connect can run the SAME bounded, read-only search the in-app agent uses —
  // every query audited, nothing exposed until enabled. Hard-default-OFF: it
  // mutates ONLY the draft, never cascades from the master, and the worker
  // still refuses to start unless this is saved-on AND the session is unlocked.
  function handleMcpToggle() {
    updateAiDraft((current) => ({
      ...current,
      mcpEnabled: !current.mcpEnabled,
    }))
  }

  // Skill / usage-guide consent (W-AI-9 Sub-block C): turning this on lets the
  // MCP server serve a built-in, machine-facing guide that teaches a connected
  // external agent HOW to query your history effectively (granularity, how the
  // search mode is chosen, citing evidence). It is guidance only — read-only,
  // exposes no new data, and is only reachable when the MCP server is also on.
  // Hard-default-OFF, mutates ONLY the draft, never cascades from the master.
  function handleSkillToggle() {
    updateAiDraft((current) => ({
      ...current,
      skillEnabled: !current.skillEnabled,
    }))
  }

  // Hybrid-search tuning knobs (W-AI-9 / W-AI-6). They mutate ONLY the draft —
  // persisted by the existing AI config Save, never auto-saved — and pass through
  // the same client-side clamp the backend enforces on load, so a slider/input can
  // never push an out-of-range or NaN value into the draft. `value` here is the raw
  // input (possibly NaN from an emptied number field); `applySearchTuningKnob`
  // sanitizes it per knob (RRF k floored to an integer ≥ 1; weights to [0, 100];
  // starred boost to [0, 0.5]; NaN → that knob's conservative default).
  function handleSearchTuningChange(knob: SearchTuningKnob, value: number) {
    updateAiDraft((current) => applySearchTuningKnob(current, knob, value))
  }

  // Restore all four knobs to their accepted defaults (60 / 1.0 / 1.0 / 0.15) on
  // the draft. Save still required to persist, consistent with every other knob.
  function handleResetSearchTuning() {
    updateAiDraft((current) => resetSearchTuningKnobs(current))
  }

  function handleAddProvider(
    purpose: 'llm' | 'embedding',
    format: AiRequestFormat = purpose === 'llm' ? 'lm-studio' : 'ollama',
  ) {
    const newProvider = makeDefaultAiProviderDraft(purpose, format)
    updateAiDraft((current) =>
      appendAiProviderDraft(current, purpose, newProvider),
    )
  }

  // Probe a configured provider's base URL so the user gets reachable / latency
  // / error feedback right in Settings instead of discovering a dead endpoint
  // only when a chat fails far away. Uses the persisted provider id, so it only
  // works once the provider exists in saved config (the button is gated on that
  // in the section). Failures degrade into a synthetic not-ok report rather than
  // throwing, so one unreachable endpoint never breaks the editor.
  async function handleProviderProbe(
    purpose: AiProviderPurpose,
    providerId: string,
  ) {
    setAiTestingProviderId(providerId)
    try {
      const report = await backend.testAiProviderConnection({
        providerId,
        purpose,
      })
      setAiProviderProbes((prev) => ({ ...prev, [providerId]: report }))
    } catch (error) {
      setAiProviderProbes((prev) => ({
        ...prev,
        [providerId]: {
          providerId,
          purpose,
          model: '',
          ok: false,
          latencyMs: 0,
          capabilities: {
            supportsChat: false,
            supportsEmbeddings: false,
            supportsStreaming: false,
            supportsToolUse: false,
            supportsStructuredOutput: false,
          },
          llmCapabilities: null,
          errorCode: null,
          actionHint: null,
          retryHint: null,
          warnings: [],
          message: describeError(error, 'test_ai_provider_connection'),
        },
      }))
    } finally {
      setAiTestingProviderId(null)
    }
  }

  function handleUpdateProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) {
    updateAiDraft((current) =>
      patchAiProviderDraft(current, purpose, providerId, patch),
    )
  }

  function handleRemoveProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) {
    updateAiDraft((current) =>
      removeAiProviderDraft(current, purpose, providerId),
    )
  }

  function handleSelectProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) {
    updateAiDraft((current) =>
      selectAiProviderDraft(current, purpose, providerId),
    )
  }

  function handleAiApiKeyChange(providerId: string, value: string) {
    setAiApiKeys((prev) => ({ ...prev, [providerId]: value }))
  }

  async function handleSaveAiConfig() {
    if (!snapshot || !aiDraft) {
      return
    }

    setSaving(true)
    try {
      const nextSnapshot = await saveConfig({
        ...snapshot.config,
        ai: aiDraft,
      })
      syncAiDraft(nextSnapshot.config.ai)
    } finally {
      setSaving(false)
    }
  }

  function handleResetAiConfig() {
    if (!snapshot?.config.ai) {
      return
    }

    syncAiDraft(snapshot.config.ai)
  }

  async function handleSaveAiApiKey(providerId: string) {
    const key = aiApiKeys[providerId]
    if (!key?.trim()) {
      return
    }

    setSaving(true)
    try {
      await backend.storeAiProviderApiKey({
        providerId,
        apiKey: key.trim(),
      })
      setAiApiKeys((prev) => ({ ...prev, [providerId]: '' }))
      updateAiProviderSecretState(providerId, true)
      await refreshAppData()
    } finally {
      setSaving(false)
    }
  }

  async function handleClearAiApiKey(providerId: string) {
    setSaving(true)
    try {
      await backend.clearAiProviderApiKey(providerId)
      updateAiProviderSecretState(providerId, false)
      await refreshAppData()
    } finally {
      setSaving(false)
    }
  }

  async function handleAiIntegrationCopy(key: string, value: string) {
    await copyReviewValue(value, {
      key,
      onFeedback: setAiIntegrationCopyFeedback,
    })
  }

  const aiProviderTranslations: {
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
  } = {
    providerName: t('settings.aiProviderName'),
    providerId: t('settings.aiProviderId'),
    requestFormat: t('settings.aiRequestFormat'),
    baseUrl: t('settings.aiBaseUrl'),
    baseUrlPlaceholder: t('settings.aiBaseUrlPlaceholder'),
    defaultModel: t('settings.aiDefaultModel'),
    modelCatalog: t('settings.aiModelCatalog'),
    modelCatalogHint: t('settings.aiModelCatalogHint'),
    enabled: t('settings.aiEnabled'),
    temperature: t('settings.aiTemperature'),
    maxTokens: t('settings.aiMaxTokens'),
    dimensions: t('settings.aiDimensions'),
    notes: t('settings.aiNotes'),
    apiKey: t('settings.aiApiKey'),
    apiKeyPlaceholder: t('settings.aiApiKeyPlaceholder'),
    keySaved: t('settings.aiKeySaved'),
    keyNotSaved: t('settings.aiKeyNotSaved'),
    saveKey: t('settings.aiSaveKey'),
    clearKey: t('settings.aiClearKey'),
    remove: t('settings.aiRemoveProvider'),
    testConnection: t('settings.aiTestConnection'),
    testingConnection: t('settings.aiTestingConnection'),
    probeReachable: t('settings.aiProbeReachable'),
    probeUnreachable: t('settings.aiProbeUnreachable'),
    requestFormatLabels: {
      openai: t('settings.aiRequestFormatOpenai'),
      anthropic: t('settings.aiRequestFormatAnthropic'),
      google: t('settings.aiRequestFormatGoogle'),
      ollama: t('settings.aiRequestFormatOllama'),
      'lm-studio': t('settings.aiRequestFormatLmStudio'),
    },
  }

  return {
    ai: {
      aiApiKeys,
      aiStatus: snapshot?.aiStatus ?? null,
      configDirty: aiConfigDirty,
      copyFeedback: aiIntegrationCopyFeedback,
      currentSettings: currentAiSettings,
      indexMeta: aiIndexMeta,
      integrationError: aiIntegrationError,
      integrationPreview: localizedAiIntegrationPreview,
      noProviders: noAiProviders,
      persistedProviderIds,
      providerProbes: aiProviderProbes,
      providerTranslations: aiProviderTranslations,
      saving,
      testingProviderId: aiTestingProviderId,
      onAddProvider: handleAddProvider,
      onApiKeyChange: handleAiApiKeyChange,
      onClearAiApiKey: handleClearAiApiKey,
      onCopyIntegrationValue: handleAiIntegrationCopy,
      onOpenPath: (path: string) => {
        void backend.openPathInFileManager(path)
      },
      onProviderProbe: handleProviderProbe,
      onRemoveProvider: handleRemoveProvider,
      onResetAiConfig: handleResetAiConfig,
      onResetSearchTuning: handleResetSearchTuning,
      onSaveAiApiKey: handleSaveAiApiKey,
      onSaveAiConfig: handleSaveAiConfig,
      onSearchTuningChange: handleSearchTuningChange,
      onSelectProvider: handleSelectProvider,
      onToggleAi: handleAiToggle,
      onToggleAssistant: handleAssistantToggle,
      onToggleMcp: handleMcpToggle,
      onToggleSkill: handleSkillToggle,
      onToggleSemanticIndex: handleSemanticIndexToggle,
      onUpdateProvider: handleUpdateProvider,
    },
  }
}
