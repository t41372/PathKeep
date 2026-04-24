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
import { useI18n } from '../../lib/i18n'
import { aiStatusMeta } from '../../lib/intelligence-ai-presentation'
import type {
  AiIntegrationPreview,
  AiProviderConfig,
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
  const [aiDraft, setAiDraft] = useState<AiSettings | null>(null)
  const [aiApiKeys, setAiApiKeys] = useState<Record<string, string>>({})
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
          setAiIntegrationError(
            error instanceof Error ? error.message : t('common.notAvailable'),
          )
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
    if (!snapshot?.config.ai && !aiDraft) {
      return
    }

    setAiDraft((current) =>
      updater(current ?? cloneAiSettings(snapshot!.config.ai)),
    )
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

  function handleAddProvider(purpose: 'llm' | 'embedding') {
    const newProvider = makeDefaultAiProviderDraft(purpose, 'ollama')
    updateAiDraft((current) =>
      appendAiProviderDraft(current, purpose, newProvider),
    )
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
      providerTranslations: aiProviderTranslations,
      saving,
      onAddProvider: handleAddProvider,
      onApiKeyChange: handleAiApiKeyChange,
      onClearAiApiKey: handleClearAiApiKey,
      onCopyIntegrationValue: handleAiIntegrationCopy,
      onOpenPath: (path: string) => {
        void backend.openPathInFileManager(path)
      },
      onRemoveProvider: handleRemoveProvider,
      onResetAiConfig: handleResetAiConfig,
      onSaveAiApiKey: handleSaveAiApiKey,
      onSaveAiConfig: handleSaveAiConfig,
      onSelectProvider: handleSelectProvider,
      onToggleAi: handleAiToggle,
      onUpdateProvider: handleUpdateProvider,
    },
  }
}
