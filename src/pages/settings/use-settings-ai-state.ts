/**
 * @file use-settings-ai-state.ts
 * @description Owns the Settings AI provider draft, API key, and integration-preview workflows.
 * @module pages/settings
 *
 * ## 職責
 * - 管理 AI provider 編輯 buffer、API key review、與 integration preview。
 * - 所有 toggle / selection / tuning / GPU / add / remove 立即 auto-save；provider 文字欄位在 blur 時 commit。
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
  // The snapshot signature we last adopted into the draft. We only re-adopt the
  // saved snapshot when THIS changes — i.e. on a genuine external snapshot
  // update — so our own just-persisted auto-save (which puts the draft AHEAD of a
  // not-yet-refreshed snapshot prop) is never clobbered back to the stale prop.
  const lastSnapshotSignatureRef = useRef<string | null>(null)
  // Mirror of the latest draft so back-to-back auto-saves (or a save fired before
  // the next render commits) compute from the freshest settings instead of a
  // stale render closure — without this two rapid toggles would clobber each other.
  const aiDraftRef = useRef<AiSettings | null>(aiDraft)
  aiDraftRef.current = aiDraft
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
  const noAiProviders =
    (currentAiSettings?.llmProviders.length ?? 0) === 0 &&
    (currentAiSettings?.embeddingProviders.length ?? 0) === 0

  useEffect(() => {
    if (!savedAiSettings || snapshotAiSignature === null) {
      return
    }

    // Seed the draft the first time a snapshot is available.
    if (aiDraft === null) {
      const seeded = cloneAiSettings(savedAiSettings)
      aiDraftRef.current = seeded
      setAiDraft(seeded)
      lastSyncedAiSignatureRef.current = snapshotAiSignature
      lastSnapshotSignatureRef.current = snapshotAiSignature
      return
    }

    const draftSignature = serializeAiSettings(aiDraft)
    const draftMatchesSnapshot = draftSignature === snapshotAiSignature

    // When the draft already equals the snapshot there are no local edits, so
    // record that we are in sync. This keeps `lastSyncedAiSignatureRef` truthful
    // on the initial non-seed render so a later external change is recognised.
    if (draftMatchesSnapshot) {
      lastSyncedAiSignatureRef.current = snapshotAiSignature
      lastSnapshotSignatureRef.current = snapshotAiSignature
      return
    }

    // Only react to a GENUINE external snapshot change. Our own auto-save already
    // synced the draft + refs to the value it wrote, so without this guard the
    // effect would clobber the just-persisted draft back to a stale snapshot prop
    // that hasn't refreshed yet.
    if (snapshotAiSignature === lastSnapshotSignatureRef.current) {
      return
    }

    // Adopt the external change unless the user has local uncommitted edits beyond
    // what we last synced (then we keep their in-progress edits).
    const hasLocalEdits = draftSignature !== lastSyncedAiSignatureRef.current

    if (!hasLocalEdits) {
      const adopted = cloneAiSettings(savedAiSettings)
      aiDraftRef.current = adopted
      setAiDraft(adopted)
      lastSyncedAiSignatureRef.current = snapshotAiSignature
    }
    lastSnapshotSignatureRef.current = snapshotAiSignature
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

    setAiDraft((current) => {
      const nextDraft = updater(current as AiSettings)
      aiDraftRef.current = nextDraft
      return nextDraft
    })
  }

  function syncAiDraft(settings: AiSettings) {
    const nextDraft = cloneAiSettings(settings)
    aiDraftRef.current = nextDraft
    setAiDraft(nextDraft)
    lastSyncedAiSignatureRef.current = serializeAiSettings(nextDraft)
  }

  // The freshest known settings: the live draft ref (kept current across batched
  // updates) falling back to the saved snapshot before any draft exists.
  function latestAiSettings(): AiSettings | null {
    return aiDraftRef.current ?? savedAiSettings
  }

  // Persist a fully-computed next AI settings object immediately — the page is
  // all-auto-save, so every toggle / selection / tuning / add / remove writes
  // through here rather than into a staged draft. We optimistically reflect the
  // value (so the control stays responsive), then re-sync to the backend's truth
  // on success. Returns true only when the write landed so the section flashes
  // the quiet "Saved" chip; a no-op (settings unchanged) or a snapshot-less call
  // returns false and stays silent. On failure the optimistic draft is kept (so
  // the control still shows what the user chose) and `saveConfig` re-throws — the
  // shell surfaces the error banner and the caller swallows the rejection. The
  // next external snapshot reconciles the draft back to the persisted truth.
  async function persistAi(next: AiSettings): Promise<boolean> {
    if (!snapshot) {
      return false
    }
    // No-op when `next` already matches the last value we persisted (the sync
    // effect seeds this ref to the loaded snapshot on first render). This keeps
    // commit-on-blur from firing a redundant write — and a misleading "Saved" —
    // when nothing changed.
    if (serializeAiSettings(next) === lastSyncedAiSignatureRef.current) {
      return false
    }

    aiDraftRef.current = next
    setAiDraft(next)
    setSaving(true)
    try {
      const nextSnapshot = await saveConfig({
        ...snapshot.config,
        ai: next,
      })
      syncAiDraft(nextSnapshot.config.ai)
      return true
    } finally {
      setSaving(false)
    }
  }

  // Persist by applying a mutation on top of the latest settings. The structural
  // controls (toggles, selection, tuning, gpu, add/remove) all route here so they
  // auto-save without the old staged-draft + Save button.
  async function persistAiMutation(
    mutate: (current: AiSettings) => AiSettings,
  ): Promise<boolean> {
    const current = latestAiSettings()
    if (!current) {
      return false
    }
    return persistAi(mutate(cloneAiSettings(current)))
  }

  function updateAiProviderSecretState(
    providerId: string,
    apiKeySaved: boolean,
  ) {
    updateAiDraft((current) =>
      mergeAiProviderSecretState(current, providerId, apiKeySaved),
    )
  }

  function handleAiToggle(): Promise<boolean> {
    return persistAiMutation((current) => ({
      ...current,
      enabled: !current.enabled,
    }))
  }

  // Granular consent: the assistant and semantic-search capabilities each have
  // their own opt-in below the master switch. They auto-save (the master
  // `enabled` is intentionally NOT cascaded) and never auto-start work —
  // semantic search still requires an explicit one-time index build elsewhere.
  function handleAssistantToggle(): Promise<boolean> {
    return persistAiMutation((current) => ({
      ...current,
      assistantEnabled: !current.assistantEnabled,
    }))
  }

  function handleSemanticIndexToggle(): Promise<boolean> {
    return persistAiMutation((current) => ({
      ...current,
      semanticIndexEnabled: !current.semanticIndexEnabled,
    }))
  }

  // Outward data-surface consent (W-AI-9 Sub-block B): turning this on lets the
  // worker expose a localhost-only, stdio MCP server so external AI tools you
  // connect can run the SAME bounded, read-only search the in-app agent uses —
  // every query audited, nothing exposed until enabled. Hard-default-OFF: it
  // auto-saves, never cascades from the master, and the worker still refuses to
  // start unless this is saved-on AND the session is unlocked.
  function handleMcpToggle(): Promise<boolean> {
    return persistAiMutation((current) => ({
      ...current,
      mcpEnabled: !current.mcpEnabled,
    }))
  }

  // Skill / usage-guide consent (W-AI-9 Sub-block C): turning this on lets the
  // MCP server serve a built-in, machine-facing guide that teaches a connected
  // external agent HOW to query your history effectively (granularity, how the
  // search mode is chosen, citing evidence). It is guidance only — read-only,
  // exposes no new data, and is only reachable when the MCP server is also on.
  // Hard-default-OFF, auto-saves, never cascades from the master.
  function handleSkillToggle(): Promise<boolean> {
    return persistAiMutation((current) => ({
      ...current,
      skillEnabled: !current.skillEnabled,
    }))
  }

  // Hybrid-search tuning knobs (W-AI-9 / W-AI-6). They auto-save and pass through
  // the same client-side clamp the backend enforces on load, so a slider/input can
  // never push an out-of-range or NaN value into the persisted config. `value`
  // here is the raw input (possibly NaN from an emptied number field);
  // `applySearchTuningKnob` sanitizes it per knob (RRF k floored to an integer ≥ 1;
  // weights to [0, 100]; starred boost to [0, 0.5]; NaN → that knob's default).
  function handleSearchTuningChange(
    knob: SearchTuningKnob,
    value: number,
  ): Promise<boolean> {
    return persistAiMutation((current) =>
      applySearchTuningKnob(current, knob, value),
    )
  }

  // Restore all four knobs to their accepted defaults (60 / 1.0 / 1.0 / 0.15) and
  // auto-save, consistent with every other knob.
  function handleResetSearchTuning(): Promise<boolean> {
    return persistAiMutation((current) => resetSearchTuningKnobs(current))
  }

  // GPU heavy-tier opt-in (W-AI-9 Sub-block D). Hard-default-OFF; auto-saves,
  // never cascades from the master. We persist REGARDLESS of whether this binary
  // is a Metal build, so a future Metal build honors the saved preference — the
  // section renders the honest "needs a Metal build" state when the backend
  // reports `gpuAvailable: false`, never a green toggle that lies.
  function handleGpuEnabledToggle(): Promise<boolean> {
    return persistAiMutation((current) => ({
      ...current,
      gpuEnabled: !current.gpuEnabled,
    }))
  }

  // Adding a provider auto-persists immediately, so a freshly-added provider is
  // saved config the moment it appears — Test connection and Save key work right
  // away, with no "save settings first" step.
  function handleAddProvider(
    purpose: 'llm' | 'embedding',
    format: AiRequestFormat = purpose === 'llm' ? 'lm-studio' : 'ollama',
  ): Promise<boolean> {
    const newProvider = makeDefaultAiProviderDraft(purpose, format)
    return persistAiMutation((current) =>
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

  // Provider FIELD edits (name, base URL, model, etc.) update the local editing
  // buffer ONLY while the user types, keeping saveConfig off the keystroke hot
  // path. The finished value is persisted by handleCommitProviders on blur.
  function handleUpdateProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) {
    updateAiDraft((current) =>
      patchAiProviderDraft(current, purpose, providerId, patch),
    )
  }

  // Commit any in-progress provider field edits when focus leaves a card. No-ops
  // (returns false, no chip) when the buffer already matches saved config, so a
  // blur without an edit never fires a redundant write.
  function handleCommitProviders(): Promise<boolean> {
    const current = latestAiSettings()
    if (!current) {
      return Promise.resolve(false)
    }
    return persistAi(cloneAiSettings(current))
  }

  // Removing a provider is structural, so it auto-saves immediately.
  function handleRemoveProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ): Promise<boolean> {
    return persistAiMutation((current) =>
      removeAiProviderDraft(current, purpose, providerId),
    )
  }

  // Selecting the active provider auto-saves immediately.
  function handleSelectProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ): Promise<boolean> {
    return persistAiMutation((current) =>
      selectAiProviderDraft(current, purpose, providerId),
    )
  }

  function handleAiApiKeyChange(providerId: string, value: string) {
    setAiApiKeys((prev) => ({ ...prev, [providerId]: value }))
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
      copyFeedback: aiIntegrationCopyFeedback,
      currentSettings: currentAiSettings,
      indexMeta: aiIndexMeta,
      integrationError: aiIntegrationError,
      integrationPreview: localizedAiIntegrationPreview,
      noProviders: noAiProviders,
      providerProbes: aiProviderProbes,
      providerTranslations: aiProviderTranslations,
      saving,
      testingProviderId: aiTestingProviderId,
      onAddProvider: handleAddProvider,
      onApiKeyChange: handleAiApiKeyChange,
      onClearAiApiKey: handleClearAiApiKey,
      onCommitProviders: handleCommitProviders,
      onCopyIntegrationValue: handleAiIntegrationCopy,
      onOpenPath: (path: string) => {
        void backend.openPathInFileManager(path)
      },
      onProviderProbe: handleProviderProbe,
      onRemoveProvider: handleRemoveProvider,
      onResetSearchTuning: handleResetSearchTuning,
      onSaveAiApiKey: handleSaveAiApiKey,
      onSearchTuningChange: handleSearchTuningChange,
      onSelectProvider: handleSelectProvider,
      onToggleAi: handleAiToggle,
      onToggleAssistant: handleAssistantToggle,
      onToggleGpu: handleGpuEnabledToggle,
      onToggleMcp: handleMcpToggle,
      onToggleSkill: handleSkillToggle,
      onToggleSemanticIndex: handleSemanticIndexToggle,
      onUpdateProvider: handleUpdateProvider,
    },
  }
}
