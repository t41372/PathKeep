import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend'
import {
  READABLE_CONTENT_REFETCH_PLUGIN_ID,
  enrichmentPluginRegistry,
  enrichmentPluginState,
  resolveEnrichmentSettings,
} from '../../lib/enrichment'
import { languageLabel, supportedLanguages, useI18n } from '../../lib/i18n'
import {
  hasSafariAccessIssue,
  keyringNeedsReview,
  normalizePlatform,
  platformLabelKey,
  platformSummaryKey,
} from '../../lib/platform-guidance'
import type {
  AiProviderConfig,
  AiRequestFormat,
  AiSettings,
  ClearDerivedIntelligenceReport,
  RemoteBackupConfig,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
  RunInsightsReport,
  ScheduleStatus,
  SecurityStatus,
} from '../../lib/types'
import { LoadingState } from '../../components/primitives/loading-state'
import { AiProviderEditorList } from '../../components/ai-provider-editor'

interface SupportState {
  scheduleStatus: ScheduleStatus | null
  securityStatus: SecurityStatus | null
}

export function SettingsPage() {
  const { buildInfo, dashboard, refreshAppData, saveConfig, snapshot } =
    useShellData()
  const { language, setLanguagePreference, t } = useI18n()
  const [saving, setSaving] = useState(false)
  const [remoteTab, setRemoteTab] = useState<
    'preview' | 'manual' | 'execute' | 'verify'
  >('preview')
  const [remoteDraft, setRemoteDraft] = useState<RemoteBackupConfig | null>(
    null,
  )
  const [accessKeyId, setAccessKeyId] = useState('')
  const [secretAccessKey, setSecretAccessKey] = useState('')
  const [remotePreview, setRemotePreview] =
    useState<RemoteBackupPreview | null>(null)
  const [remoteResult, setRemoteResult] = useState<RemoteBackupResult | null>(
    null,
  )
  const [remoteVerification, setRemoteVerification] =
    useState<RemoteBackupVerification | null>(null)
  const [remoteAction, setRemoteAction] = useState<string | null>(null)
  const [rebuildReport, setRebuildReport] = useState<RunInsightsReport | null>(
    null,
  )
  const [clearReport, setClearReport] =
    useState<ClearDerivedIntelligenceReport | null>(null)
  const [derivedAction, setDerivedAction] = useState<string | null>(null)
  const [supportState, setSupportState] = useState<SupportState>({
    scheduleStatus: null,
    securityStatus: null,
  })
  const [aiApiKeys, setAiApiKeys] = useState<Record<string, string>>({})

  useEffect(() => {
    let cancelled = false
    const loadSupportState = async () => {
      try {
        const [scheduleStatus, securityStatus] = await Promise.all([
          backend.scheduleStatus(),
          backend.securityStatus(),
        ])

        if (!cancelled) {
          setSupportState({ scheduleStatus, securityStatus })
        }
      } catch {
        if (!cancelled) {
          setSupportState({ scheduleStatus: null, securityStatus: null })
        }
      }
    }

    void loadSupportState()
    return () => {
      cancelled = true
    }
  }, [snapshot?.config.preferredLanguage])

  useEffect(() => {
    if (!snapshot) {
      return
    }

    setRemoteDraft(snapshot.config.remoteBackup)
  }, [snapshot])

  const enrichmentSettings = useMemo(
    () => resolveEnrichmentSettings(snapshot?.config.enrichment),
    [snapshot?.config.enrichment],
  )

  if (!snapshot) {
    return (
      <section className="page-shell">
        <LoadingState label={t('settings.loadingModules')} />
      </section>
    )
  }

  const profiles = snapshot.browserProfiles
  const selectedIds = new Set(snapshot.config.selectedProfileIds)
  const safariNeedsAccess = hasSafariAccessIssue(profiles)
  const platform = normalizePlatform(supportState.scheduleStatus?.platform)
  const scheduleNeedsHelp =
    supportState.scheduleStatus?.installState === 'manual-review' ||
    supportState.scheduleStatus?.installState === 'mismatch' ||
    supportState.scheduleStatus?.installState === 'permission-warning' ||
    supportState.scheduleStatus?.installState === 'legacy-install-detected'
  const keyringWarning = keyringNeedsReview(supportState.securityStatus)
  const readableRefetchPlugin = enrichmentPluginState(
    enrichmentSettings,
    READABLE_CONTENT_REFETCH_PLUGIN_ID,
  )
  const readableRefetchMeta = enrichmentPluginRegistry.find(
    (plugin) => plugin.id === READABLE_CONTENT_REFETCH_PLUGIN_ID,
  )
  const remoteConfigured = Boolean(
    remoteDraft?.bucket.trim() && remoteDraft.region.trim(),
  )
  const latestRemoteBundlePath = remoteResult?.bundlePath ?? null

  async function persistRemoteConfig() {
    if (!snapshot || !remoteDraft) {
      return
    }

    const nextSnapshot = await saveConfig({
      ...snapshot.config,
      remoteBackup: {
        ...remoteDraft,
        credentialsSaved: snapshot.config.remoteBackup.credentialsSaved,
      },
    })
    setRemoteDraft(nextSnapshot.config.remoteBackup)
  }

  function browserIcon(profileId: string): string {
    const kind = profileId.split(':')[0]
    if (kind === 'chrome') return 'C'
    if (kind === 'arc') return 'A'
    if (kind === 'firefox') return 'F'
    if (kind === 'safari') return 'S'
    return kind[0]?.toUpperCase() ?? '?'
  }

  function browserIconClass(profileId: string): string {
    const kind = profileId.split(':')[0]
    return `browser-icon ${kind}`
  }

  async function toggleProfile(profileId: string) {
    if (saving || !snapshot) return
    setSaving(true)
    try {
      const next = selectedIds.has(profileId)
        ? snapshot.config.selectedProfileIds.filter((id) => id !== profileId)
        : [...snapshot.config.selectedProfileIds, profileId]
      await saveConfig({ ...snapshot.config, selectedProfileIds: next })
    } finally {
      setSaving(false)
    }
  }

  async function handleLanguageChange(nextLanguage: string) {
    if (!snapshot) {
      return
    }

    if (
      nextLanguage !== 'system' &&
      nextLanguage !== 'en' &&
      nextLanguage !== 'zh-CN' &&
      nextLanguage !== 'zh-TW'
    ) {
      return
    }

    setSaving(true)
    try {
      setLanguagePreference(nextLanguage)
      await saveConfig({
        ...snapshot.config,
        preferredLanguage: nextLanguage,
      })
    } finally {
      setSaving(false)
    }
  }

  async function handleSaveRemoteConfig() {
    if (!remoteDraft) {
      return
    }

    setRemoteAction(t('settings.savingRemoteSettings'))
    try {
      await persistRemoteConfig()
      setRemoteVerification(null)
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleStoreCredentials() {
    if (!accessKeyId.trim() || !secretAccessKey.trim()) {
      return
    }

    setRemoteAction(t('settings.storingRemoteCredentials'))
    try {
      await backend.storeS3Credentials({
        accessKeyId: accessKeyId.trim(),
        secretAccessKey: secretAccessKey.trim(),
      })
      setAccessKeyId('')
      setSecretAccessKey('')
      await refreshAppData()
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleClearCredentials() {
    setRemoteAction(t('settings.clearingRemoteCredentials'))
    try {
      await backend.clearS3Credentials()
      await refreshAppData()
    } finally {
      setRemoteAction(null)
    }
  }

  async function handlePreviewRemote() {
    setRemoteAction(t('settings.previewingRemoteBackup'))
    try {
      await persistRemoteConfig()
      const preview = await backend.previewRemoteBackup()
      setRemotePreview(preview)
      setRemoteTab('preview')
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleExecuteRemote() {
    setRemoteAction(t('settings.executingRemoteBackup'))
    try {
      await persistRemoteConfig()
      const result = await backend.runRemoteBackup()
      setRemoteResult(result)
      setRemoteTab('execute')
      await refreshAppData()
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleVerifyRemote() {
    if (!latestRemoteBundlePath) {
      return
    }

    setRemoteAction(t('settings.verifyingRemoteBackup'))
    try {
      const verification = await backend.verifyRemoteBackup(
        latestRemoteBundlePath,
      )
      setRemoteVerification(verification)
      setRemoteTab('verify')
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleReadableRefetchToggle() {
    if (!snapshot) {
      return
    }

    const nextPlugins = enrichmentSettings.plugins.map((plugin) =>
      plugin.id === READABLE_CONTENT_REFETCH_PLUGIN_ID
        ? { ...plugin, enabled: !plugin.enabled }
        : plugin,
    )
    setDerivedAction(t('settings.savingEnrichmentSettings'))
    try {
      await saveConfig({
        ...snapshot.config,
        enrichment: { plugins: nextPlugins },
      })
      await refreshAppData()
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleRebuildDerivedState() {
    setDerivedAction(t('settings.rebuildingDerivedState'))
    try {
      const report = await backend.runInsightsNow({ fullRebuild: true })
      setRebuildReport(report)
      setClearReport(null)
      await refreshAppData()
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleClearDerivedState() {
    setDerivedAction(t('settings.clearingDerivedState'))
    try {
      const report = await backend.clearDerivedIntelligence()
      setClearReport(report)
      setRebuildReport(null)
      await refreshAppData()
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleAiToggle() {
    if (!snapshot) return
    setSaving(true)
    try {
      await saveConfig({
        ...snapshot.config,
        ai: { ...snapshot.config.ai, enabled: !snapshot.config.ai.enabled },
      })
    } finally {
      setSaving(false)
    }
  }

  function makeDefaultProvider(
    purpose: 'llm' | 'embedding',
    format: AiRequestFormat,
  ): AiProviderConfig {
    const presets: Record<
      AiRequestFormat,
      { name: string; baseUrl: string; model: string; embModel: string }
    > = {
      ollama: {
        name: 'Ollama',
        baseUrl: 'http://localhost:11434',
        model: 'llama3.2:8b',
        embModel: 'nomic-embed-text',
      },
      'lm-studio': {
        name: 'LM Studio',
        baseUrl: 'http://localhost:1234/v1',
        model: 'local-model',
        embModel: 'local-embed',
      },
      openai: {
        name: 'OpenAI',
        baseUrl: 'https://api.openai.com/v1',
        model: 'gpt-4o',
        embModel: 'text-embedding-3-small',
      },
      anthropic: {
        name: 'Anthropic',
        baseUrl: 'https://api.anthropic.com',
        model: 'claude-sonnet-4-6',
        embModel: 'voyage-3',
      },
      google: {
        name: 'Google',
        baseUrl: 'https://generativelanguage.googleapis.com',
        model: 'gemini-2.0-flash',
        embModel: 'text-embedding-004',
      },
    }
    const p = presets[format]
    return {
      id: `${format}-${purpose}-${Date.now()}`,
      name: p.name,
      purpose,
      requestFormat: format,
      enabled: true,
      baseUrl: p.baseUrl,
      apiKeySaved: false,
      defaultModel: purpose === 'llm' ? p.model : p.embModel,
      modelCatalog: [],
      temperature: purpose === 'llm' ? 0.7 : null,
      maxTokens: purpose === 'llm' ? 1200 : null,
      dimensions: purpose === 'embedding' ? 1536 : null,
      notes: null,
    }
  }

  async function handleAiConfigSave(patch: Partial<AiSettings>) {
    if (!snapshot) return
    setSaving(true)
    try {
      await saveConfig({
        ...snapshot.config,
        ai: { ...snapshot.config.ai, ...patch },
      })
    } finally {
      setSaving(false)
    }
  }

  function handleAddProvider(purpose: 'llm' | 'embedding') {
    if (!snapshot) return
    const newProvider = makeDefaultProvider(purpose, 'ollama')
    const key = purpose === 'llm' ? 'llmProviders' : 'embeddingProviders'
    void handleAiConfigSave({
      [key]: [...snapshot.config.ai[key], newProvider],
    })
  }

  function handleUpdateProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) {
    if (!snapshot) return
    const key = purpose === 'llm' ? 'llmProviders' : 'embeddingProviders'
    void handleAiConfigSave({
      [key]: snapshot.config.ai[key].map((p) =>
        p.id === providerId ? { ...p, ...patch } : p,
      ),
    })
  }

  function handleRemoveProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) {
    if (!snapshot) return
    const key = purpose === 'llm' ? 'llmProviders' : 'embeddingProviders'
    const idKey = purpose === 'llm' ? 'llmProviderId' : 'embeddingProviderId'
    const nextProviders = snapshot.config.ai[key].filter(
      (p) => p.id !== providerId,
    )
    const nextIdPatch =
      snapshot.config.ai[idKey] === providerId ? { [idKey]: null } : {}
    void handleAiConfigSave({
      [key]: nextProviders,
      ...nextIdPatch,
    })
  }

  function handleSelectProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) {
    const idKey = purpose === 'llm' ? 'llmProviderId' : 'embeddingProviderId'
    void handleAiConfigSave({ [idKey]: providerId })
  }

  async function handleSaveAiApiKey(providerId: string) {
    const key = aiApiKeys[providerId]
    if (!key?.trim()) return
    setSaving(true)
    try {
      await backend.storeAiProviderApiKey({
        providerId,
        apiKey: key.trim(),
      })
      setAiApiKeys((prev) => ({ ...prev, [providerId]: '' }))
      await refreshAppData()
    } finally {
      setSaving(false)
    }
  }

  async function handleClearAiApiKey(providerId: string) {
    setSaving(true)
    try {
      await backend.clearAiProviderApiKey(providerId)
      await refreshAppData()
    } finally {
      setSaving(false)
    }
  }

  const aiProviderTranslations = {
    providerName: t('settings.aiProviderName'),
    providerId: t('settings.aiProviderId'),
    requestFormat: t('settings.aiRequestFormat'),
    baseUrl: t('settings.aiBaseUrl'),
    defaultModel: t('settings.aiDefaultModel'),
    modelCatalog: t('settings.aiModelCatalog'),
    modelCatalogHint: t('settings.aiModelCatalogHint'),
    enabled: t('settings.aiEnabled'),
    temperature: t('settings.aiTemperature'),
    maxTokens: t('settings.aiMaxTokens'),
    dimensions: t('settings.aiDimensions'),
    notes: t('settings.aiNotes'),
    apiKey: t('settings.aiApiKey'),
    keyStored: t('settings.aiKeyStored'),
    saveKey: t('settings.aiSaveKey'),
    clearKey: t('settings.aiClearKey'),
    remove: t('settings.aiRemoveProvider'),
  }

  return (
    <section className="page-shell settings-page" data-testid="settings-page">
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.browserProfiles')}</span>
          <span className="panel-action">{t('common.rescanAction')}</span>
        </div>
        <div className="panel-body">
          <div className="profile-list">
            {profiles.map((profile) => {
              const checked = selectedIds.has(profile.profileId)
              return (
                <button
                  key={profile.profileId}
                  className={`profile-item ${checked ? 'checked' : ''}`}
                  type="button"
                  onClick={() => {
                    void toggleProfile(profile.profileId)
                  }}
                >
                  <div className="profile-check">
                    <div className={`checkbox ${checked ? 'active' : ''}`}>
                      {checked ? '✓' : ''}
                    </div>
                  </div>
                  <div className="profile-icon">
                    <div className={browserIconClass(profile.profileId)}>
                      {browserIcon(profile.profileId)}
                    </div>
                  </div>
                  <div className="profile-info">
                    <div className="profile-name">
                      {profile.browserName} / {profile.profileName}
                    </div>
                    <div className="profile-path dim mono">
                      {profile.profilePath}
                    </div>
                  </div>
                  <div className="profile-stats mono dim">
                    {profile.historyExists
                      ? `${t('settings.historyFound')} · ${profile.browserVersion ?? t('common.notAvailable')}`
                      : t('settings.noHistoryDetected')}
                  </div>
                </button>
              )
            })}
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.remoteBackup')}</span>
          <span className="panel-badge">{t('settings.s3Compatible')}</span>
        </div>
        <div className="panel-body settings-remote-grid">
          <StatusCallout
            tone={remoteConfigured ? 'info' : 'warning'}
            title={t('settings.remoteBackupSummary')}
            body={t('settings.remoteBackupBody')}
          />

          <div className="settings-field-grid">
            <label className="checkbox-row">
              <input
                aria-label={t('settings.remoteEnabled')}
                checked={remoteDraft?.enabled ?? false}
                type="checkbox"
                onChange={(event) => {
                  setRemoteDraft((current) =>
                    current
                      ? { ...current, enabled: event.target.checked }
                      : current,
                  )
                }}
              />
              <span>{t('settings.remoteEnabled')}</span>
            </label>
            <label className="checkbox-row">
              <input
                aria-label={t('settings.pathStyleLabel')}
                checked={remoteDraft?.pathStyle ?? true}
                type="checkbox"
                onChange={(event) => {
                  setRemoteDraft((current) =>
                    current
                      ? { ...current, pathStyle: event.target.checked }
                      : current,
                  )
                }}
              />
              <span>{t('settings.pathStyleLabel')}</span>
            </label>
            <label className="checkbox-row">
              <input
                aria-label={t('settings.uploadAfterBackup')}
                checked={remoteDraft?.uploadAfterBackup ?? false}
                type="checkbox"
                onChange={(event) => {
                  setRemoteDraft((current) =>
                    current
                      ? { ...current, uploadAfterBackup: event.target.checked }
                      : current,
                  )
                }}
              />
              <span>{t('settings.uploadAfterBackup')}</span>
            </label>
            <label className="field-stack">
              <span>{t('settings.bucketLabel')}</span>
              <input
                aria-label={t('settings.bucketLabel')}
                value={remoteDraft?.bucket ?? ''}
                onChange={(event) => {
                  setRemoteDraft((current) =>
                    current
                      ? { ...current, bucket: event.target.value }
                      : current,
                  )
                }}
              />
            </label>
            <label className="field-stack">
              <span>{t('settings.regionLabel')}</span>
              <input
                aria-label={t('settings.regionLabel')}
                value={remoteDraft?.region ?? ''}
                onChange={(event) => {
                  setRemoteDraft((current) =>
                    current
                      ? { ...current, region: event.target.value }
                      : current,
                  )
                }}
              />
            </label>
            <label className="field-stack">
              <span>{t('settings.endpointLabel')}</span>
              <input
                aria-label={t('settings.endpointLabel')}
                placeholder={t('settings.endpointPlaceholder')}
                value={remoteDraft?.endpoint ?? ''}
                onChange={(event) => {
                  setRemoteDraft((current) =>
                    current
                      ? {
                          ...current,
                          endpoint: event.target.value || null,
                        }
                      : current,
                  )
                }}
              />
            </label>
            <label className="field-stack">
              <span>{t('settings.prefixLabel')}</span>
              <input
                aria-label={t('settings.prefixLabel')}
                value={remoteDraft?.prefix ?? ''}
                onChange={(event) => {
                  setRemoteDraft((current) =>
                    current
                      ? { ...current, prefix: event.target.value }
                      : current,
                  )
                }}
              />
            </label>
          </div>

          <div className="settings-action-row">
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(remoteAction)}
              onClick={() => {
                void handleSaveRemoteConfig()
              }}
            >
              {t('settings.saveRemoteSettings')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(remoteAction) || !remoteConfigured}
              onClick={() => {
                void handlePreviewRemote()
              }}
            >
              {t('settings.previewRemoteBackup')}
            </button>
            <button
              className="btn-primary"
              type="button"
              disabled={
                Boolean(remoteAction) ||
                !remoteConfigured ||
                !snapshot.config.remoteBackup.credentialsSaved
              }
              onClick={() => {
                void handleExecuteRemote()
              }}
            >
              {t('settings.executeRemoteBackup')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(remoteAction) || !latestRemoteBundlePath}
              onClick={() => {
                void handleVerifyRemote()
              }}
            >
              {t('settings.verifyRemoteBackup')}
            </button>
          </div>

          <div className="settings-remote-columns">
            <div className="field-stack">
              <span>{t('settings.credentialsStatus')}</span>
              <strong>
                {snapshot.config.remoteBackup.credentialsSaved
                  ? t('settings.credentialsSaved')
                  : t('settings.credentialsMissing')}
              </strong>
              <span className="dim">
                {snapshot.config.remoteBackup.lastUploadedAt
                  ? `${t('settings.lastUploadedAt')}: ${snapshot.config.remoteBackup.lastUploadedAt}`
                  : t('settings.remoteNoUploadYet')}
              </span>
              {snapshot.config.remoteBackup.lastUploadedObjectKey ? (
                <span className="dim mono">
                  {snapshot.config.remoteBackup.lastUploadedObjectKey}
                </span>
              ) : null}
              {snapshot.config.remoteBackup.lastError ? (
                <span className="dim">
                  {snapshot.config.remoteBackup.lastError}
                </span>
              ) : null}
            </div>

            <div className="settings-field-grid">
              <label className="field-stack">
                <span>{t('settings.accessKeyId')}</span>
                <input
                  aria-label={t('settings.accessKeyId')}
                  value={accessKeyId}
                  onChange={(event) => {
                    setAccessKeyId(event.target.value)
                  }}
                />
              </label>
              <label className="field-stack">
                <span>{t('settings.secretAccessKey')}</span>
                <input
                  aria-label={t('settings.secretAccessKey')}
                  type="password"
                  value={secretAccessKey}
                  onChange={(event) => {
                    setSecretAccessKey(event.target.value)
                  }}
                />
              </label>
              <div className="settings-action-row">
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={
                    Boolean(remoteAction) ||
                    !accessKeyId.trim() ||
                    !secretAccessKey.trim()
                  }
                  onClick={() => {
                    void handleStoreCredentials()
                  }}
                >
                  {t('settings.storeRemoteCredentials')}
                </button>
                <button
                  className="btn-danger"
                  type="button"
                  disabled={
                    Boolean(remoteAction) ||
                    !snapshot.config.remoteBackup.credentialsSaved
                  }
                  onClick={() => {
                    void handleClearCredentials()
                  }}
                >
                  {t('settings.clearRemoteCredentials')}
                </button>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">{t('settings.remotePme')}</span>
            </div>
            <div className="panel-body">
              <div className="pme-tabs">
                {(['preview', 'manual', 'execute', 'verify'] as const).map(
                  (tab) => (
                    <button
                      key={tab}
                      className={`pme-tab ${remoteTab === tab ? 'active' : ''}`}
                      type="button"
                      onClick={() => {
                        setRemoteTab(tab)
                      }}
                    >
                      {tab === 'preview'
                        ? t('common.previewTab')
                        : tab === 'manual'
                          ? t('common.manualTab')
                          : tab === 'execute'
                            ? t('common.executeTab')
                            : t('common.verifyTab')}
                    </button>
                  ),
                )}
              </div>

              {remoteAction ? (
                <StatusCallout tone="info" title={remoteAction} body="" />
              ) : null}

              {remoteTab === 'preview' ? (
                <div className="settings-result-list">
                  <StatusCallout
                    tone={remotePreview ? 'info' : 'warning'}
                    title={t('settings.previewBoundaryTitle')}
                    body={
                      remotePreview
                        ? t('settings.previewBoundaryReady')
                        : t('settings.previewBoundaryBody')
                    }
                  />
                  {remotePreview ? (
                    <>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.bundlePath')}
                        </span>
                        <span className="config-value mono">
                          {remotePreview.bundlePath}
                        </span>
                      </div>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.objectKey')}
                        </span>
                        <span className="config-value mono">
                          {remotePreview.objectKey}
                        </span>
                      </div>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.uploadUrl')}
                        </span>
                        <span className="config-value mono">
                          {remotePreview.uploadUrl}
                        </span>
                      </div>
                      <div className="inline-note-list">
                        {remotePreview.warnings.map((warning) => (
                          <div key={warning} className="result-row">
                            <p>{warning}</p>
                          </div>
                        ))}
                      </div>
                    </>
                  ) : null}
                </div>
              ) : null}

              {remoteTab === 'manual' ? (
                <div className="settings-result-list">
                  <StatusCallout
                    tone="info"
                    title={t('settings.manualBoundaryTitle')}
                    body={t('settings.manualBoundaryBody')}
                  />
                  {remotePreview ? (
                    <>
                      <div className="code-panel">
                        <span>{t('settings.previewCommand')}</span>
                        <pre>{remotePreview.previewCommand}</pre>
                      </div>
                      <div className="inline-note-list">
                        {remotePreview.manualSteps.map((step) => (
                          <div key={step} className="result-row">
                            <p>{step}</p>
                          </div>
                        ))}
                        <div className="result-row">
                          <p>{t('settings.retentionGuidance')}</p>
                        </div>
                      </div>
                    </>
                  ) : (
                    <StatusCallout
                      tone="warning"
                      title={t('settings.previewFirstTitle')}
                      body={t('settings.previewFirstBody')}
                    />
                  )}
                </div>
              ) : null}

              {remoteTab === 'execute' ? (
                <div className="settings-result-list">
                  <StatusCallout
                    tone={remoteResult?.uploaded ? 'success' : 'warning'}
                    title={t('settings.executeBoundaryTitle')}
                    body={t('settings.executeBoundaryBody')}
                  />
                  {remoteResult ? (
                    <>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.bundlePath')}
                        </span>
                        <span className="config-value mono">
                          {remoteResult.bundlePath}
                        </span>
                      </div>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.objectKey')}
                        </span>
                        <span className="config-value mono">
                          {remoteResult.objectKey}
                        </span>
                      </div>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.executeMessage')}
                        </span>
                        <span className="config-value">
                          {remoteResult.message}
                        </span>
                      </div>
                    </>
                  ) : (
                    <StatusCallout
                      tone="info"
                      title={t('settings.executeNotRunTitle')}
                      body={t('settings.executeNotRunBody')}
                    />
                  )}
                </div>
              ) : null}

              {remoteTab === 'verify' ? (
                <div className="settings-result-list">
                  <StatusCallout
                    tone={
                      remoteVerification?.restoreReady ? 'success' : 'warning'
                    }
                    title={t('settings.verifyBoundaryTitle')}
                    body={t('settings.verifyBoundaryBody')}
                  />
                  {remoteVerification ? (
                    <>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.bundleVersion')}
                        </span>
                        <span className="config-value mono">
                          {remoteVerification.bundleVersion}
                        </span>
                      </div>
                      <div className="config-row">
                        <span className="config-label">
                          {t('settings.restoreReady')}
                        </span>
                        <span className="config-value">
                          {remoteVerification.restoreReady
                            ? t('common.statusClear')
                            : t('common.statusNeedsAttention')}
                        </span>
                      </div>
                      <div className="inline-note-list">
                        {remoteVerification.checks.map((check) => (
                          <div key={check.name} className="result-row">
                            <div className="result-row__header">
                              <strong>{check.name}</strong>
                              <span className="mono">{check.status}</span>
                            </div>
                            <p>{check.message}</p>
                          </div>
                        ))}
                        {remoteVerification.restoreSteps.map((step) => (
                          <div key={step} className="result-row">
                            <p>{step}</p>
                          </div>
                        ))}
                      </div>
                    </>
                  ) : (
                    <StatusCallout
                      tone="info"
                      title={t('settings.verifyNotRunTitle')}
                      body={t('settings.verifyNotRunBody')}
                    />
                  )}
                </div>
              ) : null}
            </div>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">
            {t('settings.enrichmentDerivedState')}
          </span>
          <span className="panel-badge">{t('settings.derivedOnly')}</span>
        </div>
        <div className="panel-body settings-remote-grid">
          <StatusCallout
            tone="info"
            title={t('settings.derivedStateBoundaryTitle')}
            body={t('settings.derivedStateBoundaryBody')}
            actions={
              <div className="settings-action-row">
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={Boolean(derivedAction)}
                  onClick={() => {
                    void handleRebuildDerivedState()
                  }}
                >
                  {t('settings.rebuildDerivedState')}
                </button>
                <button
                  className="btn-danger"
                  type="button"
                  disabled={Boolean(derivedAction)}
                  onClick={() => {
                    void handleClearDerivedState()
                  }}
                >
                  {t('settings.clearDerivedState')}
                </button>
              </div>
            }
          />

          <div className="result-row result-row--active">
            <div className="result-row__header">
              <strong>{t('settings.readableContentRefetch')}</strong>
              <span className="mono">
                {readableRefetchPlugin.enabled
                  ? t('settings.enabled')
                  : t('settings.disabled')}
              </span>
            </div>
            <p>{t('settings.readableContentRefetchBody')}</p>
            <div className="config-row">
              <span className="config-label">
                {t('settings.pluginVersion')}
              </span>
              <span className="config-value mono">
                {readableRefetchPlugin.version}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('settings.pluginQueue')}</span>
              <span className="config-value mono">
                {readableRefetchMeta?.queue ?? t('common.notAvailable')}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('settings.pluginFreshness')}
              </span>
              <span className="config-value mono">
                {readableRefetchMeta?.freshnessDays
                  ? t('settings.daysFreshness', {
                      days: readableRefetchMeta.freshnessDays,
                    })
                  : t('common.notAvailable')}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('settings.pluginDerivedTables')}
              </span>
              <span className="config-value mono">
                {readableRefetchMeta?.derivedTables.join(', ')}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('settings.pluginStorageImpact')}
              </span>
              <span className="config-value">
                {t('settings.readableContentRefetchImpact')}
              </span>
            </div>
            <div className="settings-action-row">
              <button
                className="btn-secondary"
                type="button"
                disabled={Boolean(derivedAction)}
                onClick={() => {
                  void handleReadableRefetchToggle()
                }}
              >
                {readableRefetchPlugin.enabled
                  ? t('settings.disablePlugin')
                  : t('settings.enablePlugin')}
              </button>
            </div>
          </div>

          <div className="settings-result-list">
            {dashboard?.recentRuns[0] ? (
              <div className="result-row">
                <div className="result-row__header">
                  <strong>{t('settings.latestGrowthSignal')}</strong>
                  <Link
                    className="btn-tiny"
                    to={`/audit?run=${dashboard.recentRuns[0].id}`}
                  >
                    {t('settings.openAuditRun')}
                  </Link>
                </div>
                <p>
                  {t('settings.latestGrowthSignalBody', {
                    runId: dashboard.recentRuns[0].id,
                    visits: dashboard.recentRuns[0].newVisits,
                    urls: dashboard.recentRuns[0].newUrls,
                    downloads: dashboard.recentRuns[0].newDownloads,
                  })}
                </p>
              </div>
            ) : null}
            {rebuildReport ? (
              <div className="result-row">
                <div className="result-row__header">
                  <strong>{t('settings.rebuildCompletedTitle')}</strong>
                  <span className="mono">#{rebuildReport.runId}</span>
                </div>
                <p>
                  {t('settings.rebuildCompletedBody', {
                    visits: rebuildReport.processedVisits,
                    enriched: rebuildReport.enrichedVisits,
                    cards: rebuildReport.cardCount,
                  })}
                </p>
              </div>
            ) : null}
            {clearReport ? (
              <div className="result-row">
                <div className="result-row__header">
                  <strong>{t('settings.clearCompletedTitle')}</strong>
                  <span className="mono">
                    {clearReport.clearedCardRows + clearReport.clearedTopicRows}
                  </span>
                </div>
                <p>
                  {t('settings.clearCompletedBody', {
                    enrichments: clearReport.clearedEnrichmentRows,
                    features: clearReport.clearedFeatureRows,
                    cards: clearReport.clearedCardRows,
                  })}
                </p>
              </div>
            ) : null}
            {derivedAction ? (
              <StatusCallout tone="info" title={derivedAction} body="" />
            ) : null}
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.aiProvider')}</span>
          <span className="panel-badge">{t('settings.optional')}</span>
        </div>
        <div className="panel-body">
          <label className="checkbox-row">
            <input
              aria-label={t('settings.aiMasterToggle')}
              checked={snapshot.config.ai.enabled}
              type="checkbox"
              disabled={saving}
              onChange={() => {
                void handleAiToggle()
              }}
            />
            <span>{t('settings.aiMasterToggle')}</span>
          </label>

          <AiProviderEditorList
            addLabel={t('settings.aiAddLlmProvider')}
            apiKeys={aiApiKeys}
            onAdd={() => handleAddProvider('llm')}
            onApiKeyChange={(id, value) =>
              setAiApiKeys((prev) => ({ ...prev, [id]: value }))
            }
            onClearKey={(id) => {
              void handleClearAiApiKey(id)
            }}
            onRemove={(id) => handleRemoveProvider('llm', id)}
            onSaveKey={(id) => {
              void handleSaveAiApiKey(id)
            }}
            onSelect={(id) => handleSelectProvider('llm', id)}
            onUpdate={(id, patch) => handleUpdateProvider('llm', id, patch)}
            providers={snapshot.config.ai.llmProviders}
            purpose="llm"
            selectedProviderId={snapshot.config.ai.llmProviderId ?? null}
            title={t('settings.aiLlmProviders')}
            translations={aiProviderTranslations}
          />

          <AiProviderEditorList
            addLabel={t('settings.aiAddEmbeddingProvider')}
            apiKeys={aiApiKeys}
            onAdd={() => handleAddProvider('embedding')}
            onApiKeyChange={(id, value) =>
              setAiApiKeys((prev) => ({ ...prev, [id]: value }))
            }
            onClearKey={(id) => {
              void handleClearAiApiKey(id)
            }}
            onRemove={(id) => handleRemoveProvider('embedding', id)}
            onSaveKey={(id) => {
              void handleSaveAiApiKey(id)
            }}
            onSelect={(id) => handleSelectProvider('embedding', id)}
            onUpdate={(id, patch) =>
              handleUpdateProvider('embedding', id, patch)
            }
            providers={snapshot.config.ai.embeddingProviders}
            purpose="embedding"
            selectedProviderId={snapshot.config.ai.embeddingProviderId ?? null}
            title={t('settings.aiEmbeddingProviders')}
            translations={aiProviderTranslations}
          />

          <div className="config-row" style={{ marginTop: 'var(--space-4)' }}>
            <span className="config-label">
              {t('settings.aiActiveLlmProvider')}
            </span>
            <span className="config-value mono">
              {snapshot.config.ai.llmProviders.find(
                (p) => p.id === snapshot.config.ai.llmProviderId,
              )?.name ?? t('settings.aiNoneSelected')}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.aiActiveEmbeddingProvider')}
            </span>
            <span className="config-value mono">
              {snapshot.config.ai.embeddingProviders.find(
                (p) => p.id === snapshot.config.ai.embeddingProviderId,
              )?.name ?? t('settings.aiNoneSelected')}
            </span>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.general')}</span>
        </div>
        <div className="panel-body">
          <div className="config-row">
            <span className="config-label">
              {t('settings.interfaceLanguage')}
            </span>
            <select
              aria-label={t('settings.interfaceLanguage')}
              className="settings-select"
              disabled={saving}
              value={snapshot.config.preferredLanguage}
              onChange={(event) => {
                void handleLanguageChange(event.target.value)
              }}
            >
              <option value="system">{t('common.followSystem')}</option>
              {supportedLanguages.map((entry) => (
                <option key={entry} value={entry}>
                  {languageLabel(entry, language)}
                </option>
              ))}
            </select>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.currentLanguage')}
            </span>
            <span className="config-value">
              {languageLabel(language, language)}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.dataDirectory')}</span>
            <span className="config-value mono">
              {snapshot.directories.appRoot}
            </span>
            <button
              className="btn-tiny"
              type="button"
              onClick={() => {
                void backend.openPathInFileManager(snapshot.directories.appRoot)
              }}
            >
              {t('settings.openDirectory')}
            </button>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.archiveDatabase')}
            </span>
            <span className="config-value mono">
              {snapshot.directories.archiveDatabasePath}
            </span>
            <button
              className="btn-tiny"
              type="button"
              onClick={() => {
                void backend.openPathInFileManager(
                  snapshot.directories.archiveDatabasePath,
                )
              }}
            >
              {t('settings.openDirectory')}
            </button>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.auditRepository')}
            </span>
            <span className="config-value mono">
              {snapshot.directories.auditRepoPath}
            </span>
            <button
              className="btn-tiny"
              type="button"
              onClick={() => {
                void backend.openPathInFileManager(
                  snapshot.directories.auditRepoPath,
                )
              }}
            >
              {t('settings.openDirectory')}
            </button>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.mcpServer')}</span>
            <span className="config-value">
              {snapshot.config.ai.mcpEnabled
                ? t('settings.enabled')
                : t('settings.disabled')}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.version')}</span>
            <span className="config-value mono">
              {buildInfo?.version ?? t('common.notAvailable')}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.gitCommit')}</span>
            <span className="config-value mono">
              {buildInfo?.gitCommitShort ?? t('common.notAvailable')}
            </span>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">
            {t('settings.platformTroubleshooting')}
          </span>
        </div>
        <div className="panel-body settings-support-grid">
          <StatusCallout
            tone={scheduleNeedsHelp ? 'warning' : 'info'}
            title={t(platformLabelKey(platform))}
            body={t(platformSummaryKey(platform))}
            actions={
              <Link className="btn-secondary" to="/schedule">
                {t('settings.reviewSchedule')}
              </Link>
            }
          />
          {safariNeedsAccess ? (
            <StatusCallout
              tone="blocked"
              title={t('platform.safariAccessTitle')}
              body={t('platform.safariAccessBody')}
              actions={
                <Link className="btn-secondary" to="/import">
                  {t('settings.reviewImports')}
                </Link>
              }
            />
          ) : null}
          {keyringWarning ? (
            <StatusCallout
              tone="warning"
              title={t('platform.keyringTitle')}
              body={t('platform.keyringBody')}
              actions={
                <Link className="btn-secondary" to="/security">
                  {t('settings.reviewSecurity')}
                </Link>
              }
            />
          ) : null}
          {scheduleNeedsHelp ? (
            <StatusCallout
              tone="blocked"
              title={t('platform.schedulerMismatchTitle')}
              body={t('platform.schedulerMismatchBody')}
              actions={
                <Link className="btn-secondary" to="/schedule">
                  {t('settings.reviewSchedule')}
                </Link>
              }
            />
          ) : null}
        </div>
      </div>
    </section>
  )
}
