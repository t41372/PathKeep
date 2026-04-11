/**
 * This module renders the Settings route, the app-level control tower for diagnostics, app lock, analytics consent, updater review, retention, remote backup PME, and derived runtime review.
 *
 * Why this file exists:
 * - Settings carries a lot of front-end surface area, so the file needs narrative comments that explain why each helper and section exists instead of reading like an accidental mega-component.
 * - When the app adds a new repair or review surface, this route is often where it lands first, which makes readability and guardrails especially important.
 *
 * Main declarations:
 * - `SettingsPage`
 *
 * Source-of-truth notes:
 * - The route purpose and navigation grammar come from `docs/design/screens-and-nav.md`.
 * - Preview / Manual / Execute / Verify behavior, warning grammar, and loading honesty come from `docs/design/ux-principles.md`.
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  CONFIGURED_ANALYTICS_ENDPOINT,
  trackAnalyticsEvent,
} from '../../lib/analytics'
import { backend } from '../../lib/backend-client'
import {
  READABLE_CONTENT_REFETCH_PLUGIN_ID,
  enrichmentPluginRegistry,
  enrichmentPluginState,
  resolveEnrichmentSettings,
} from '../../lib/enrichment'
import {
  deterministicModuleDescription,
  deterministicModuleLabel,
  deterministicModuleStatusLabel,
  enrichmentPluginBoundaryLabel,
  enrichmentPluginDescription,
  enrichmentPluginLabel,
  intelligenceRuntimeJobStateLabel,
  upsertDeterministicModuleState,
  upsertEnrichmentPluginPreference,
} from '../../lib/intelligence-runtime'
import { languageLabel, supportedLanguages, useI18n } from '../../lib/i18n'
import { aiStatusMeta } from '../../lib/intelligence'
import { formatBytes, formatDateTime } from '../../lib/format'
import {
  hasSafariAccessIssue,
  keyringNeedsReview,
  normalizePlatform,
  platformLabelKey,
  platformSummaryKey,
} from '../../lib/platform-guidance'
import {
  RELEASES_PAGE_URL,
  checkForAppUpdate,
  downloadAndInstallAppUpdate,
  initialUpdateInstallState,
  relaunchAfterUpdate,
  type PendingAppUpdate,
} from '../../lib/update'
import type {
  AnalyticsConfig,
  AiIntegrationPreview,
  AiProviderConfig,
  AiRequestFormat,
  AiSettings,
  AppLockConfig,
  ClearDerivedIntelligenceReport,
  IntelligenceRuntimeSnapshot,
  RemoteBackupConfig,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
  RetentionPreview,
  RetentionPruneResult,
  RunInsightsReport,
  UpdateAvailability,
  UpdateInstallState,
} from '../../lib/types'
import { LoadingState } from '../../components/primitives/loading-state'
import { AiProviderEditorList } from '../../components/ai-provider-editor'
import {
  appendAiProviderDraft,
  browserIcon,
  browserIconClass,
  buildRetentionSelection,
  cloneAiSettings,
  mergeAiProviderSecretState,
  patchAiProviderDraft,
  removeAiProviderDraft,
  selectAiProviderDraft,
  serializeAiSettings,
  type SupportState,
} from './helpers'

/**
 * Renders the Settings route.
 *
 * This page is intentionally explicit because it acts as PathKeep's control
 * tower: diagnostics, app lock, updater review, analytics consent, AI
 * providers, retention, and remote-backup PME all converge here.
 */
export function SettingsPage() {
  const {
    appLockStatus,
    buildInfo,
    clearAppLockPasscode,
    dashboard,
    lockAppSession,
    refreshKey,
    refreshAppData,
    saveConfig,
    setAppLockPasscode,
    snapshot,
  } = useShellData()
  const { language, setLanguagePreference, t, ns } = useI18n()
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
  const [intelligenceRuntime, setIntelligenceRuntime] =
    useState<IntelligenceRuntimeSnapshot | null>(null)
  const [intelligenceRuntimeError, setIntelligenceRuntimeError] = useState<
    string | null
  >(null)
  const [supportState, setSupportState] = useState<SupportState>({
    scheduleStatus: null,
    securityStatus: null,
  })
  const [appLockDraft, setAppLockDraft] = useState<AppLockConfig | null>(null)
  const [analyticsDraft, setAnalyticsDraft] = useState<AnalyticsConfig | null>(
    null,
  )
  const [appLockPasscode, setAppLockPasscodeDraft] = useState('')
  const [appLockRecoveryHint, setAppLockRecoveryHint] = useState('')
  const [appLockAction, setAppLockAction] = useState<string | null>(null)
  const [analyticsAction, setAnalyticsAction] = useState<string | null>(null)
  const [updateAvailability, setUpdateAvailability] =
    useState<UpdateAvailability | null>(null)
  const [pendingUpdate, setPendingUpdate] = useState<PendingAppUpdate | null>(
    null,
  )
  const [updateInstallState, setUpdateInstallState] =
    useState<UpdateInstallState>(initialUpdateInstallState)
  const [aiDraft, setAiDraft] = useState<AiSettings | null>(null)
  const [aiApiKeys, setAiApiKeys] = useState<Record<string, string>>({})
  const [retentionPreview, setRetentionPreview] =
    useState<RetentionPreview | null>(null)
  const [retentionSelection, setRetentionSelection] = useState<
    Record<string, boolean>
  >({})
  const [retentionResult, setRetentionResult] =
    useState<RetentionPruneResult | null>(null)
  const [retentionAction, setRetentionAction] = useState<string | null>(null)
  const [retentionError, setRetentionError] = useState<string | null>(null)
  const [aiIntegrationPreview, setAiIntegrationPreview] =
    useState<AiIntegrationPreview | null>(null)
  const [aiIntegrationError, setAiIntegrationError] = useState<string | null>(
    null,
  )
  const [selectedAiGeneratedFileIndex, setSelectedAiGeneratedFileIndex] =
    useState(0)
  const lastSyncedAiSignatureRef = useRef<string | null>(null)

  /**
   * Reloads the derived intelligence runtime snapshot shown by the Settings
   * review panels.
   *
   * Multiple actions depend on the same runtime truth, so we keep the refresh
   * path named instead of duplicating it across button handlers.
   */
  async function refreshIntelligenceRuntimeState() {
    try {
      const runtime = await backend.loadIntelligenceRuntime()
      setIntelligenceRuntime(runtime)
      setIntelligenceRuntimeError(null)
    } catch (error) {
      setIntelligenceRuntime(null)
      setIntelligenceRuntimeError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    }
  }

  useEffect(() => {
    let cancelled = false

    /**
     * Loads the schedule/security support state that feeds Settings callouts.
     *
     * The page treats these as review surfaces, not hidden diagnostics, so the
     * fetch lives near route state instead of being buried in JSX.
     */
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
    let cancelled = false

    /**
     * Loads the derived intelligence runtime snapshot during hydration and
     * whenever a shell refresh invalidates the current review data.
     */
    const loadRuntime = async () => {
      try {
        const runtime = await backend.loadIntelligenceRuntime()
        if (!cancelled) {
          setIntelligenceRuntime(runtime)
          setIntelligenceRuntimeError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setIntelligenceRuntime(null)
          setIntelligenceRuntimeError(
            error instanceof Error ? error.message : t('common.notAvailable'),
          )
        }
      }
    }

    void loadRuntime()
    return () => {
      cancelled = true
    }
  }, [refreshKey, snapshot?.config.initialized, t])

  useEffect(() => {
    if (!snapshot) {
      return
    }

    setRemoteDraft(snapshot.config.remoteBackup)
  }, [snapshot])

  useEffect(() => {
    if (!snapshot) {
      return
    }

    setAppLockDraft(snapshot.config.appLock)
    setAppLockRecoveryHint(snapshot.config.appLock.recoveryHint ?? '')
  }, [snapshot])

  useEffect(() => {
    if (!snapshot) {
      return
    }

    setAnalyticsDraft(snapshot.config.analytics)
  }, [snapshot])

  useEffect(() => {
    let cancelled = false

    /**
     * Loads the local retention preview so the prune UI always starts from a
     * truthful view of reclaimable artifacts.
     */
    const loadRetentionPreview = async () => {
      try {
        const preview = await backend.previewRetentionPrune()
        if (!cancelled) {
          setRetentionPreview(preview)
          setRetentionSelection((current) =>
            buildRetentionSelection(preview, current),
          )
          setRetentionError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setRetentionPreview(null)
          setRetentionError(
            error instanceof Error ? error.message : t('common.notAvailable'),
          )
        }
      }
    }

    void loadRetentionPreview()
    return () => {
      cancelled = true
    }
  }, [refreshAppData, snapshot?.config.initialized, t])

  const savedAiSettings = snapshot?.config.ai
  const snapshotAiSignature = useMemo(
    () => serializeAiSettings(savedAiSettings),
    [savedAiSettings],
  )

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
    if (snapshotAiSignature === null) {
      return
    }

    let cancelled = false

    /**
     * Explains how load preview works.
     *
     * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
     */

    const loadPreview = async () => {
      try {
        const preview = await backend.previewAiIntegrations()
        if (!cancelled) {
          setAiIntegrationPreview(preview)
          setAiIntegrationError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setAiIntegrationPreview(null)
          setAiIntegrationError(
            error instanceof Error ? error.message : t('common.notAvailable'),
          )
        }
      }
    }

    void loadPreview()
    return () => {
      cancelled = true
    }
  }, [snapshotAiSignature, t])

  useEffect(() => {
    setSelectedAiGeneratedFileIndex(0)
  }, [aiIntegrationPreview?.generatedFiles.length])

  const enrichmentSettings = useMemo(
    () => resolveEnrichmentSettings(snapshot?.config.enrichment),
    [snapshot?.config.enrichment],
  )
  const commonNs = ns('common')
  const settingsNs = ns('settings')
  const intelligenceT = ns('intelligence')
  const runtimePluginsById = useMemo(
    () =>
      new Map(
        (intelligenceRuntime?.plugins ?? []).map((plugin) => [
          plugin.pluginId,
          plugin,
        ]),
      ),
    [intelligenceRuntime?.plugins],
  )
  const reviewableEnrichmentPlugins = useMemo(() => {
    const registryIds = enrichmentPluginRegistry.map((plugin) => plugin.id)
    const extraIds = enrichmentSettings.plugins
      .map((plugin) => plugin.id)
      .filter((pluginId) => !registryIds.includes(pluginId))

    return [...registryIds, ...extraIds].map((pluginId) => ({
      definition: enrichmentPluginRegistry.find(
        (plugin) => plugin.id === pluginId,
      ),
      runtime: runtimePluginsById.get(pluginId),
      state: enrichmentPluginState(enrichmentSettings, pluginId),
    }))
  }, [enrichmentSettings, runtimePluginsById])
  const runtimeModulesById = useMemo(
    () =>
      new Map(
        (intelligenceRuntime?.modules ?? []).map((module) => [
          module.moduleId,
          module,
        ]),
      ),
    [intelligenceRuntime?.modules],
  )
  const reviewableDeterministicModules = useMemo(() => {
    const configuredModules = snapshot?.config.deterministic.modules ?? []
    const configIds = configuredModules.map((module) => module.id)
    const extraIds = [...runtimeModulesById.keys()].filter(
      (moduleId) => !configIds.includes(moduleId),
    )

    return [...configIds, ...extraIds].map((moduleId) => ({
      runtime: runtimeModulesById.get(moduleId),
      state: configuredModules.find((module) => module.id === moduleId) ?? {
        id: moduleId,
        enabled: true,
        version: 'm5b-v1',
      },
    }))
  }, [runtimeModulesById, snapshot?.config.deterministic.modules])

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
  const remoteConfigured = Boolean(
    remoteDraft?.bucket.trim() && remoteDraft.region.trim(),
  )
  const latestRemoteBundlePath = remoteResult?.bundlePath ?? null
  const selectedRetentionBuckets = retentionPreview
    ? retentionPreview.buckets.filter((bucket) => retentionSelection[bucket.id])
    : []
  const selectedRetentionBytes = selectedRetentionBuckets.reduce(
    (total, bucket) => total + bucket.bytes,
    0,
  )
  const retentionNeedsUnlock =
    supportState.securityStatus?.encrypted === true &&
    supportState.securityStatus.unlocked === false
  const currentAiSettings = aiDraft ?? snapshot.config.ai
  const currentAppLockSettings = appLockDraft ?? snapshot.config.appLock
  const currentAnalyticsSettings = analyticsDraft ?? snapshot.config.analytics
  const aiIndexMeta = aiStatusMeta(snapshot.aiStatus, intelligenceT)
  const selectedAiGeneratedFile =
    aiIntegrationPreview?.generatedFiles[selectedAiGeneratedFileIndex] ??
    aiIntegrationPreview?.generatedFiles[0] ??
    null
  const aiConfigDirty =
    snapshotAiSignature !== null &&
    serializeAiSettings(currentAiSettings) !== snapshotAiSignature
  const appLockConfigDirty =
    JSON.stringify(currentAppLockSettings) !==
    JSON.stringify(snapshot.config.appLock)
  const persistedProviderIds = new Set(
    [
      ...snapshot.config.ai.llmProviders,
      ...snapshot.config.ai.embeddingProviders,
    ].map((provider) => provider.id),
  )
  const appLockCanEnable =
    currentAppLockSettings.passcodeConfigured ||
    Boolean(appLockStatus?.passcodeConfigured)
  const analyticsConfigDirty =
    JSON.stringify(currentAnalyticsSettings) !==
    JSON.stringify(snapshot.config.analytics)
  const analyticsEndpointConfigured = Boolean(CONFIGURED_ANALYTICS_ENDPOINT)
  const biometricUsesTouchId =
    appLockStatus?.biometricState === 'touch-id-available' ||
    appLockStatus?.biometricState === 'touch-id-unavailable'

  /**
   * Applies an updater to the working AI settings draft.
   *
   * The Settings route keeps AI configuration editable for a while before it is
   * persisted, so this helper centralizes the "start from the current draft or
   * clone the saved config" rule.
   */
  function updateAiDraft(updater: (current: AiSettings) => AiSettings) {
    setAiDraft((current) =>
      updater(current ?? cloneAiSettings(snapshot!.config.ai)),
    )
  }

  /**
   * Replaces the AI draft with a clean clone of a known-good settings object
   * and updates the dirty-check signature at the same time.
   */
  function syncAiDraft(settings: AiSettings) {
    const nextDraft = cloneAiSettings(settings)
    setAiDraft(nextDraft)
    lastSyncedAiSignatureRef.current = serializeAiSettings(nextDraft)
  }

  /**
   * Mirrors secret-storage changes back into the in-memory provider draft so
   * the UI immediately reflects whether a provider now has a saved API key.
   */
  function updateAiProviderSecretState(
    providerId: string,
    apiKeySaved: boolean,
  ) {
    updateAiDraft((current) =>
      mergeAiProviderSecretState(current, providerId, apiKeySaved),
    )
  }

  /**
   * Persists the remote-backup draft while preserving the credential-saved flag
   * that only the backend can authoritatively decide.
   */
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

  /**
   * Reloads the retention preview after a prune or config refresh.
   *
   * This helper also reapplies the selection merge rule so newly discovered
   * buckets default from their size while existing choices stay intact.
   */
  async function refreshRetentionPreview() {
    try {
      const preview = await backend.previewRetentionPrune()
      setRetentionPreview(preview)
      setRetentionSelection((current) =>
        buildRetentionSelection(preview, current),
      )
      setRetentionError(null)
    } catch (error) {
      setRetentionPreview(null)
      setRetentionError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    }
  }

  /**
   * Handles retention prune.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleRetentionPrune() {
    if (selectedRetentionBuckets.length === 0) {
      setRetentionError(t('settings.retentionNothingSelected'))
      return
    }

    setRetentionAction(t('settings.retentionExecute'))
    setRetentionError(null)
    try {
      const result = await backend.runRetentionPrune({
        bucketIds: selectedRetentionBuckets.map((bucket) => bucket.id),
      })
      setRetentionResult(result)
      await refreshAppData()
      await refreshRetentionPreview()
    } catch (error) {
      setRetentionError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    } finally {
      setRetentionAction(null)
    }
  }

  /**
   * Adds or removes a browser profile from the archive-selection list shown in
   * Settings.
   */
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

  /**
   * Handles language change.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  /**
   * Handles save remote config.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  /**
   * Handles store credentials.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  /**
   * Handles clear credentials.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleClearCredentials() {
    setRemoteAction(t('settings.clearingRemoteCredentials'))
    try {
      await backend.clearS3Credentials()
      await refreshAppData()
    } finally {
      setRemoteAction(null)
    }
  }

  /**
   * Handles preview remote.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  /**
   * Handles execute remote.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  /**
   * Handles verify remote.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  /**
   * Handles enrichment plugin toggle.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleEnrichmentPluginToggle(pluginId: string) {
    if (!snapshot) {
      return
    }

    const currentPlugin = enrichmentPluginState(enrichmentSettings, pluginId)
    const nextEnabled = !currentPlugin.enabled
    const nextPlugins = resolveEnrichmentSettings(
      snapshot.config.enrichment,
    ).plugins.map((plugin) =>
      plugin.id === pluginId ? { ...plugin, enabled: nextEnabled } : plugin,
    )
    setDerivedAction(t('settings.savingEnrichmentSettings'))
    try {
      await saveConfig({
        ...snapshot.config,
        enrichment: { plugins: nextPlugins },
        ai: {
          ...snapshot.config.ai,
          enrichmentPlugins: upsertEnrichmentPluginPreference(
            snapshot.config.ai.enrichmentPlugins,
            pluginId,
            nextEnabled,
          ),
        },
      })
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  /**
   * Handles deterministic module toggle.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleDeterministicModuleToggle(moduleId: string) {
    if (!snapshot) {
      return
    }

    const currentModule =
      snapshot.config.deterministic.modules.find(
        (module) => module.id === moduleId,
      ) ?? null
    const nextEnabled = !(currentModule?.enabled ?? true)
    setDerivedAction(t('settings.savingDeterministicModules'))
    try {
      await saveConfig({
        ...snapshot.config,
        deterministic: {
          modules: upsertDeterministicModuleState(
            snapshot.config.deterministic.modules,
            moduleId,
            nextEnabled,
          ),
        },
      })
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  /**
   * Handles rebuild derived state.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleRebuildDerivedState() {
    setDerivedAction(t('settings.rebuildingDerivedState'))
    try {
      const report = await backend.runInsightsNow({ fullRebuild: true })
      setRebuildReport(report)
      setClearReport(null)
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  /**
   * Handles clear derived state.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleClearDerivedState() {
    setDerivedAction(t('settings.clearingDerivedState'))
    try {
      const report = await backend.clearDerivedIntelligence()
      setClearReport(report)
      setRebuildReport(null)
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  /**
   * Handles retry intelligence runtime job.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleRetryIntelligenceRuntimeJob(jobId: number) {
    setDerivedAction(settingsNs('retryRuntimeJob'))
    try {
      const runtime = await backend.retryIntelligenceJob(jobId)
      setIntelligenceRuntime(runtime)
      setIntelligenceRuntimeError(null)
    } catch (error) {
      setIntelligenceRuntimeError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    } finally {
      setDerivedAction(null)
    }
  }

  /**
   * Handles cancel intelligence runtime job.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleCancelIntelligenceRuntimeJob(jobId: number) {
    setDerivedAction(settingsNs('cancelRuntimeJob'))
    try {
      const runtime = await backend.cancelIntelligenceJob(jobId)
      setIntelligenceRuntime(runtime)
      setIntelligenceRuntimeError(null)
    } catch (error) {
      setIntelligenceRuntimeError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    } finally {
      setDerivedAction(null)
    }
  }

  /**
   * Handles save app lock config.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleSaveAppLockConfig() {
    if (!snapshot || !appLockDraft) {
      return
    }

    setAppLockAction(t('settings.appLockSaving'))
    try {
      const nextSnapshot = await saveConfig({
        ...snapshot.config,
        appLock: {
          ...appLockDraft,
          biometricEnabled:
            appLockDraft.biometricEnabled &&
            Boolean(appLockStatus?.biometricAvailable),
          passcodeEnabled: true,
          passcodeConfigured:
            appLockStatus?.passcodeConfigured ??
            appLockDraft.passcodeConfigured,
          recoveryHint: appLockRecoveryHint.trim() || null,
        },
      })
      setAppLockDraft(nextSnapshot.config.appLock)
      setAppLockRecoveryHint(nextSnapshot.config.appLock.recoveryHint ?? '')
    } finally {
      setAppLockAction(null)
    }
  }

  /**
   * Handles set app lock passcode.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleSetAppLockPasscode() {
    setAppLockAction(t('settings.appLockSavingPasscode'))
    try {
      await setAppLockPasscode({
        passcode: appLockPasscode,
        recoveryHint: appLockRecoveryHint.trim() || null,
      })
      setAppLockPasscodeDraft('')
    } finally {
      setAppLockAction(null)
    }
  }

  /**
   * Handles clear app lock passcode.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleClearAppLockPasscode() {
    setAppLockAction(t('settings.appLockClearingPasscode'))
    try {
      await clearAppLockPasscode()
      setAppLockPasscodeDraft('')
      setAppLockRecoveryHint('')
    } finally {
      setAppLockAction(null)
    }
  }

  /**
   * Handles lock now.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleLockNow() {
    setAppLockAction(t('settings.appLockLockingNow'))
    try {
      await lockAppSession('manual')
    } finally {
      setAppLockAction(null)
    }
  }

  /**
   * Handles save analytics consent.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleSaveAnalyticsConsent() {
    if (!snapshot) {
      return
    }

    setAnalyticsAction(t('settings.analyticsSaving'))
    try {
      const nextEnabled = currentAnalyticsSettings.enabled
      const nextSnapshot = await saveConfig({
        ...snapshot.config,
        analytics: {
          enabled: nextEnabled,
          consentGrantedAt: nextEnabled ? new Date().toISOString() : null,
        },
      })
      setAnalyticsDraft(nextSnapshot.config.analytics)
      if (nextEnabled) {
        await trackAnalyticsEvent(
          nextSnapshot.config.analytics,
          {
            type: 'cta-click',
            screen: 'settings',
            action: 'save-consent',
            feature: 'analytics',
          },
          buildInfo,
        )
      }
    } finally {
      setAnalyticsAction(null)
    }
  }

  /**
   * Handles check for updates.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleCheckForUpdates() {
    if (!snapshot) {
      return
    }

    setUpdateInstallState({
      phase: 'checking',
      downloadedBytes: null,
      contentLength: null,
      message: t('settings.updateChecking'),
    })
    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'cta-click',
        screen: 'settings',
        action: 'check-for-updates',
        feature: 'updater',
      },
      buildInfo,
    )
    try {
      const result = await checkForAppUpdate(buildInfo?.version)
      setUpdateAvailability(result.availability)
      setPendingUpdate(result.pendingUpdate)
      if (!result.availability.supported) {
        setUpdateInstallState({
          phase: 'unsupported',
          downloadedBytes: null,
          contentLength: null,
          message:
            result.availability.error ?? t('settings.updateUnsupportedBody'),
        })
      } else if (result.availability.error) {
        setUpdateInstallState({
          phase: 'error',
          downloadedBytes: null,
          contentLength: null,
          message: result.availability.error,
        })
      } else if (result.availability.available) {
        setUpdateInstallState({
          phase: 'available',
          downloadedBytes: null,
          contentLength: null,
          message: t('settings.updateAvailableBody', {
            version: result.availability.version ?? t('common.notAvailable'),
          }),
        })
      } else {
        setUpdateInstallState({
          phase: 'uptodate',
          downloadedBytes: null,
          contentLength: null,
          message: t('settings.updateUpToDateBody'),
        })
      }
      await trackAnalyticsEvent(
        snapshot.config.analytics,
        {
          type: 'update-lifecycle',
          screen: 'settings',
          action: 'check',
          status: result.availability.available
            ? 'available'
            : result.availability.error
              ? 'error'
              : result.availability.supported
                ? 'uptodate'
                : 'unsupported',
          version: result.availability.version ?? null,
        },
        buildInfo,
      )
    } catch (error) {
      setUpdateAvailability(null)
      setPendingUpdate(null)
      setUpdateInstallState({
        phase: 'error',
        downloadedBytes: null,
        contentLength: null,
        message:
          error instanceof Error ? error.message : t('common.unavailable'),
      })
    }
  }

  /**
   * Handles download and install update.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleDownloadAndInstallUpdate() {
    if (!snapshot || !pendingUpdate) {
      return
    }

    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'cta-click',
        screen: 'settings',
        action: 'download-and-install',
        feature: 'updater',
      },
      buildInfo,
    )
    const result = await downloadAndInstallAppUpdate(
      pendingUpdate,
      setUpdateInstallState,
    )
    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'update-lifecycle',
        screen: 'settings',
        action: 'download-and-install',
        status: result.phase,
        version: pendingUpdate.version,
      },
      buildInfo,
    )
  }

  /**
   * Handles relaunch for update.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleRelaunchForUpdate() {
    if (!snapshot) {
      return
    }

    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'cta-click',
        screen: 'settings',
        action: 'restart-after-update',
        feature: 'updater',
      },
      buildInfo,
    )
    await relaunchAfterUpdate()
  }

  /**
   * Renders the handle open release route.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleOpenReleasePage() {
    await backend.openExternalUrl(
      updateAvailability?.downloadUrl ?? RELEASES_PAGE_URL,
    )
  }

  /**
   * Handles ai toggle.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  function handleAiToggle() {
    updateAiDraft((current) => ({
      ...current,
      enabled: !current.enabled,
    }))
  }

  /**
   * Provides make default to descendant components.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  /**
   * Handles save ai config.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  async function handleSaveAiConfig() {
    if (!snapshot || !aiDraft) return
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

  /**
   * Handles reset ai config.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

  function handleResetAiConfig() {
    syncAiDraft(snapshot!.config.ai)
  }

  /**
   * Appends a new provider draft to the correct provider list.
   */
  function handleAddProvider(purpose: 'llm' | 'embedding') {
    const newProvider = makeDefaultProvider(purpose, 'ollama')
    updateAiDraft((current) =>
      appendAiProviderDraft(current, purpose, newProvider),
    )
  }

  /**
   * Applies a partial patch to one provider draft without mutating unrelated
   * entries.
   */
  function handleUpdateProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) {
    updateAiDraft((current) =>
      patchAiProviderDraft(current, purpose, providerId, patch),
    )
  }

  /**
   * Removes a provider draft and clears the selected provider ID if that draft
   * was currently active.
   */
  function handleRemoveProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) {
    updateAiDraft((current) =>
      removeAiProviderDraft(current, purpose, providerId),
    )
  }

  /**
   * Marks one provider draft as the currently selected provider for its
   * purpose.
   */
  function handleSelectProvider(
    purpose: 'llm' | 'embedding',
    providerId: string,
  ) {
    updateAiDraft((current) =>
      selectAiProviderDraft(current, purpose, providerId),
    )
  }

  /**
   * Handles save ai api key.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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
      updateAiProviderSecretState(providerId, true)
      await refreshAppData()
    } finally {
      setSaving(false)
    }
  }

  /**
   * Handles clear ai api key.
   *
   * Keeping this as a named declaration makes the giant Settings route easier to review, test, and evolve without burying intent in another anonymous callback.
   */

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

  const aiProviderTranslations = {
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

  const noAiProviders =
    currentAiSettings.llmProviders.length === 0 &&
    currentAiSettings.embeddingProviders.length === 0

  return (
    <section className="page-shell settings-page" data-testid="settings-page">
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.general')}</span>
        </div>
        <div className="panel-body">
          <p className="dashboard-next-action">
            {t('settings.generalDescription')}
          </p>
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
            <span className="config-label">{t('settings.logsDirectory')}</span>
            <span className="config-value mono">
              {snapshot.directories.logsDir}
            </span>
            <button
              className="btn-tiny"
              type="button"
              onClick={() => {
                void backend.openPathInFileManager(snapshot.directories.logsDir)
              }}
            >
              {t('settings.openDirectory')}
            </button>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.crashReports')}</span>
            <span className="config-value mono">
              {snapshot.directories.crashReportsDir}
            </span>
            <button
              className="btn-tiny"
              type="button"
              onClick={() => {
                void backend.openPathInFileManager(
                  snapshot.directories.crashReportsDir,
                )
              }}
            >
              {t('settings.openDirectory')}
            </button>
          </div>
          {snapshot.runtimeDiagnostics.latestCrashReport ? (
            <StatusCallout
              tone="warning"
              title={t('settings.latestCrashTitle')}
              body={t('settings.latestCrashBody', {
                source:
                  snapshot.runtimeDiagnostics.latestCrashReport.source ===
                  'rust-panic'
                    ? t('settings.latestCrashSourceRust')
                    : t('settings.latestCrashSourceFrontend'),
                time:
                  formatDateTime(
                    snapshot.runtimeDiagnostics.latestCrashReport.recordedAt,
                    language,
                  ) ?? snapshot.runtimeDiagnostics.latestCrashReport.recordedAt,
                message: snapshot.runtimeDiagnostics.latestCrashReport.message,
              })}
              actions={
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => {
                    void backend.openPathInFileManager(
                      snapshot.runtimeDiagnostics.latestCrashReport?.path ??
                        snapshot.directories.crashReportsDir,
                    )
                  }}
                >
                  {t('settings.openCrashReport')}
                </button>
              }
            />
          ) : (
            <StatusCallout
              tone="info"
              title={t('settings.latestCrashClearTitle')}
              body={t('settings.latestCrashClearBody')}
            />
          )}
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
          <span className="panel-title">{t('settings.analyticsTitle')}</span>
          <span className="panel-badge">{t('settings.optional')}</span>
        </div>
        <div className="panel-body settings-remote-grid">
          <StatusCallout
            tone={currentAnalyticsSettings.enabled ? 'warning' : 'info'}
            title={t('settings.analyticsBoundaryTitle')}
            body={t('settings.analyticsBoundaryBody')}
          />

          {!analyticsEndpointConfigured ? (
            <StatusCallout
              tone="warning"
              title={t('settings.analyticsEndpointMissingTitle')}
              body={t('settings.analyticsEndpointMissingBody')}
            />
          ) : null}

          <div className="settings-field-grid">
            <label className="checkbox-row">
              <input
                aria-label={t('settings.analyticsEnabled')}
                checked={currentAnalyticsSettings.enabled}
                type="checkbox"
                onChange={(event) => {
                  setAnalyticsDraft((current) => ({
                    enabled: event.target.checked,
                    consentGrantedAt: current?.consentGrantedAt ?? null,
                  }))
                }}
              />
              <span>{t('settings.analyticsEnabled')}</span>
            </label>

            <div className="config-row">
              <span className="config-label">
                {t('settings.analyticsEndpoint')}
              </span>
              <span className="config-value mono">
                {analyticsEndpointConfigured
                  ? import.meta.env.VITE_ANALYTICS_ENDPOINT
                  : t('common.notAvailable')}
              </span>
            </div>

            <div className="config-row">
              <span className="config-label">
                {t('settings.analyticsConsentGrantedAt')}
              </span>
              <span className="config-value mono">
                {currentAnalyticsSettings.consentGrantedAt ??
                  t('common.notAvailable')}
              </span>
            </div>

            <p className="dashboard-next-action">
              {t('settings.analyticsStatusBody')}
            </p>

            <div className="settings-action-row">
              <button
                className="btn-primary"
                type="button"
                disabled={Boolean(analyticsAction) || !analyticsConfigDirty}
                onClick={() => {
                  void handleSaveAnalyticsConsent()
                }}
              >
                {analyticsAction ?? t('settings.analyticsSave')}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.updateTitle')}</span>
          <span className="panel-action mono">
            {buildInfo?.version ?? t('common.notAvailable')}
          </span>
        </div>
        <div className="panel-body settings-remote-grid">
          <StatusCallout
            tone={
              updateInstallState.phase === 'error'
                ? 'danger'
                : updateInstallState.phase === 'available' ||
                    updateInstallState.phase === 'installed'
                  ? 'warning'
                  : 'info'
            }
            title={t('settings.updateBoundaryTitle')}
            body={
              updateInstallState.message ?? t('settings.updateBoundaryBody')
            }
          />

          <div className="settings-field-grid">
            <div className="config-row">
              <span className="config-label">
                {t('settings.updateCurrentVersion')}
              </span>
              <span className="config-value mono">
                {buildInfo?.version ?? t('common.notAvailable')}
              </span>
            </div>

            <div className="config-row">
              <span className="config-label">
                {t('settings.updateLatestVersion')}
              </span>
              <span className="config-value mono">
                {updateAvailability?.version ?? t('common.notAvailable')}
              </span>
            </div>

            <div className="config-row">
              <span className="config-label">
                {t('settings.updatePublishedAt')}
              </span>
              <span className="config-value mono">
                {updateAvailability?.publishedAt ?? t('common.notAvailable')}
              </span>
            </div>

            <div className="config-row">
              <span className="config-label">
                {t('settings.updateCheckedAt')}
              </span>
              <span className="config-value mono">
                {updateAvailability?.checkedAt ?? t('common.notAvailable')}
              </span>
            </div>

            {updateInstallState.contentLength ? (
              <p className="dashboard-next-action">
                {t('settings.updateProgress', {
                  downloaded: formatBytes(
                    updateInstallState.downloadedBytes ?? 0,
                    language,
                  ),
                  total: formatBytes(
                    updateInstallState.contentLength,
                    language,
                  ),
                })}
              </p>
            ) : null}

            {updateAvailability?.notes ? (
              <div className="fieldBlock">
                <span className="config-label">
                  {t('settings.updateReleaseNotes')}
                </span>
                <pre className="code-block">{updateAvailability.notes}</pre>
              </div>
            ) : null}

            <div className="settings-action-row">
              <button
                className="btn-primary"
                type="button"
                disabled={updateInstallState.phase === 'checking'}
                onClick={() => {
                  void handleCheckForUpdates()
                }}
              >
                {t('settings.updateCheckNow')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={
                  !pendingUpdate ||
                  updateInstallState.phase === 'downloading' ||
                  updateInstallState.phase === 'installing'
                }
                onClick={() => {
                  void handleDownloadAndInstallUpdate()
                }}
              >
                {t('settings.updateDownloadAndInstall')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={updateInstallState.phase !== 'installed'}
                onClick={() => {
                  void handleRelaunchForUpdate()
                }}
              >
                {t('settings.updateRestartNow')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={() => {
                  void handleOpenReleasePage()
                }}
              >
                {t('settings.updateOpenReleasePage')}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.retentionTitle')}</span>
          <span className="panel-action">
            {t('settings.retentionSelected', {
              size: formatBytes(selectedRetentionBytes, language),
            })}
          </span>
        </div>
        <div className="panel-body">
          <p className="dashboard-next-action">
            {t('settings.retentionDescription')}
          </p>

          {retentionNeedsUnlock ? (
            <StatusCallout
              tone="warning"
              title={t('settings.retentionUnlockTitle')}
              body={t('settings.retentionUnlockBody')}
              actions={
                <Link className="btn-secondary" to="/security">
                  {t('navigation.securityLabel')}
                </Link>
              }
            />
          ) : null}

          {retentionPreview ? (
            <div className="settings-field-grid">
              {retentionPreview.buckets.map((bucket) => (
                <label key={bucket.id} className="checkbox-row">
                  <input
                    checked={Boolean(retentionSelection[bucket.id])}
                    type="checkbox"
                    onChange={(event) => {
                      setRetentionSelection((current) => ({
                        ...current,
                        [bucket.id]: event.target.checked,
                      }))
                    }}
                  />
                  <span>
                    {bucket.id === 'snapshots'
                      ? t('settings.retentionSnapshots')
                      : bucket.id === 'exports'
                        ? t('settings.retentionExports')
                        : bucket.id === 'staging'
                          ? t('settings.retentionStaging')
                          : t('settings.retentionQuarantine')}
                    {` · ${formatBytes(bucket.bytes, language)} · ${bucket.itemCount.toLocaleString(language)} ${t('settings.retentionItems')}`}
                  </span>
                </label>
              ))}
            </div>
          ) : (
            <StatusCallout
              tone="info"
              title={t('settings.retentionLoadingTitle')}
              body={t('common.loading')}
            />
          )}

          {retentionPreview?.warnings.map((warning) => (
            <div key={warning} className="warning-box">
              <div className="warning-icon">⚠</div>
              <div className="warning-text">{warning}</div>
            </div>
          ))}

          <div className="wizard-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => {
                void refreshRetentionPreview()
              }}
            >
              {t('settings.retentionRefresh')}
            </button>
            <button
              className="btn-danger"
              type="button"
              disabled={
                retentionNeedsUnlock ||
                retentionAction !== null ||
                selectedRetentionBuckets.length === 0
              }
              onClick={() => {
                void handleRetentionPrune()
              }}
            >
              {retentionAction ?? t('settings.retentionExecute')}
            </button>
          </div>

          {retentionResult ? (
            <div
              className="inline-note-list"
              style={{ marginTop: 'var(--space-3)' }}
            >
              <div className="result-row">
                <p>
                  {t('settings.retentionDeletedBytes', {
                    size: formatBytes(retentionResult.deletedBytes, language),
                  })}
                </p>
              </div>
              <div className="result-row">
                <p>
                  {t('settings.retentionDeletedFiles', {
                    count: retentionResult.deletedFiles,
                  })}
                </p>
              </div>
              {retentionResult.runId ? (
                <div className="result-row">
                  <Link
                    className="btn-secondary"
                    to={`/audit?run=${retentionResult.runId}`}
                  >
                    {t('settings.retentionOpenAudit')}
                  </Link>
                </div>
              ) : null}
            </div>
          ) : null}

          {retentionError ? (
            <p className="inline-error" role="alert">
              {retentionError}
            </p>
          ) : null}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.appLock')}</span>
          <span className="panel-badge">{t('settings.optional')}</span>
        </div>
        <div className="panel-body settings-remote-grid">
          <StatusCallout
            tone={currentAppLockSettings.enabled ? 'warning' : 'info'}
            title={t('settings.appLockBoundaryTitle')}
            body={t('settings.appLockBoundaryBody')}
          />

          <div className="settings-field-grid">
            <label className="checkbox-row">
              <input
                aria-label={t('settings.appLockEnabled')}
                checked={currentAppLockSettings.enabled}
                type="checkbox"
                onChange={(event) => {
                  setAppLockDraft((current) =>
                    current
                      ? { ...current, enabled: event.target.checked }
                      : current,
                  )
                }}
              />
              <span>{t('settings.appLockEnabled')}</span>
            </label>

            <div className="config-row">
              <span className="config-label">
                {t('settings.appLockStatus')}
              </span>
              <span className="config-value mono">
                {appLockStatus?.locked
                  ? t('settings.appLockStatusLocked')
                  : t('settings.appLockStatusUnlocked')}
              </span>
            </div>

            <div className="config-row">
              <span className="config-label">
                {t('settings.appLockIdleTimeout')}
              </span>
              <select
                aria-label={t('settings.appLockIdleTimeout')}
                className="settings-select"
                value={currentAppLockSettings.idleTimeoutMinutes}
                onChange={(event) => {
                  const idleTimeoutMinutes = Number(event.target.value)
                  setAppLockDraft((current) =>
                    current ? { ...current, idleTimeoutMinutes } : current,
                  )
                }}
              >
                {[1, 5, 10, 15, 30, 60].map((minutes) => (
                  <option key={minutes} value={minutes}>
                    {t('settings.appLockMinutes', { count: minutes })}
                  </option>
                ))}
              </select>
            </div>

            <label className="checkbox-row">
              <input
                aria-label={
                  biometricUsesTouchId
                    ? t('settings.appLockTouchId')
                    : t('settings.appLockBiometric')
                }
                checked={currentAppLockSettings.biometricEnabled}
                disabled={!appLockStatus?.biometricAvailable}
                type="checkbox"
                onChange={(event) => {
                  setAppLockDraft((current) =>
                    current
                      ? { ...current, biometricEnabled: event.target.checked }
                      : current,
                  )
                }}
              />
              <span>
                {biometricUsesTouchId
                  ? t('settings.appLockTouchId')
                  : t('settings.appLockBiometric')}
              </span>
            </label>

            {!appLockStatus?.biometricAvailable ? (
              <p className="dashboard-next-action">
                {biometricUsesTouchId
                  ? t('settings.appLockTouchIdUnavailable')
                  : t('settings.appLockBiometricUnavailable')}
              </p>
            ) : null}

            <label className="fieldBlock">
              <span className="config-label">
                {t('settings.appLockRecoveryHint')}
              </span>
              <input
                aria-label={t('settings.appLockRecoveryHint')}
                className="settings-input"
                placeholder={t('settings.appLockRecoveryHintPlaceholder')}
                type="text"
                value={appLockRecoveryHint}
                onChange={(event) => {
                  const recoveryHint = event.target.value
                  setAppLockRecoveryHint(recoveryHint)
                  setAppLockDraft((current) =>
                    current ? { ...current, recoveryHint } : current,
                  )
                }}
              />
            </label>

            <label className="fieldBlock">
              <span className="config-label">
                {t('settings.appLockPasscode')}
              </span>
              <input
                aria-label={t('settings.appLockPasscode')}
                className="settings-input"
                placeholder={t('settings.appLockPasscodePlaceholder')}
                type="password"
                value={appLockPasscode}
                onChange={(event) =>
                  setAppLockPasscodeDraft(event.target.value)
                }
              />
            </label>

            <div className="settings-action-row">
              <button
                className="btn-primary"
                type="button"
                disabled={
                  Boolean(appLockAction) ||
                  !appLockConfigDirty ||
                  (currentAppLockSettings.enabled && !appLockCanEnable)
                }
                onClick={() => {
                  void handleSaveAppLockConfig()
                }}
              >
                {appLockAction ?? t('settings.appLockSave')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={
                  Boolean(appLockAction) || appLockPasscode.trim().length < 4
                }
                onClick={() => {
                  void handleSetAppLockPasscode()
                }}
              >
                {appLockStatus?.passcodeConfigured
                  ? t('settings.appLockUpdatePasscode')
                  : t('settings.appLockSetPasscode')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={
                  Boolean(appLockAction) || !appLockStatus?.passcodeConfigured
                }
                onClick={() => {
                  void handleClearAppLockPasscode()
                }}
              >
                {t('settings.appLockClearPasscode')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={Boolean(appLockAction) || !appLockStatus?.enabled}
                onClick={() => {
                  void handleLockNow()
                }}
              >
                {t('settings.appLockLockNow')}
              </button>
            </div>

            {!appLockCanEnable ? (
              <StatusCallout
                tone="warning"
                title={t('settings.appLockNeedsPasscodeTitle')}
                body={t('settings.appLockNeedsPasscodeBody')}
              />
            ) : null}

            {appLockStatus?.degradationNotes.map((note) => (
              <p key={note} className="dashboard-next-action">
                {note}
              </p>
            ))}

            <div className="config-row">
              <span className="config-label">
                {t('settings.appLockConfigPath')}
              </span>
              <span className="config-value mono">
                {appLockStatus?.configPath ?? t('common.notAvailable')}
              </span>
              <button
                className="btn-tiny"
                type="button"
                onClick={() => {
                  if (appLockStatus?.configPath) {
                    void backend.openPathInFileManager(appLockStatus.configPath)
                  }
                }}
              >
                {t('settings.openDirectory')}
              </button>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('settings.appLockLastUnlocked')}
              </span>
              <span className="config-value mono">
                {appLockStatus?.lastUnlockedAt ?? t('common.notAvailable')}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.browserProfiles')}</span>
          <span className="panel-action">{t('common.rescanAction')}</span>
        </div>
        <div className="panel-body">
          <p className="dashboard-next-action">
            {t('settings.browserProfilesBody')}
          </p>
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
          <span className="panel-title">{t('settings.aiProvider')}</span>
          <span className="panel-badge">{t('settings.optional')}</span>
        </div>
        <div className="panel-body">
          <p className="dashboard-next-action">
            {t('settings.aiProviderBody')}
          </p>
          {noAiProviders ? (
            <StatusCallout
              tone="info"
              title={t('settings.aiGettingStartedTitle')}
              body={t('settings.aiGettingStartedBody')}
            />
          ) : null}
          <StatusCallout
            tone={aiConfigDirty ? 'warning' : 'info'}
            title={
              aiConfigDirty
                ? t('settings.aiUnsavedChanges')
                : t('settings.aiDraftSaved')
            }
            body={t('settings.aiDraftBoundaryBody')}
            actions={
              <div className="settings-action-row">
                <button
                  className="btn-primary"
                  type="button"
                  disabled={saving || !aiConfigDirty}
                  onClick={() => {
                    void handleSaveAiConfig()
                  }}
                >
                  {saving
                    ? t('settings.aiSavingConfig')
                    : t('settings.aiSaveConfig')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={saving || !aiConfigDirty}
                  onClick={handleResetAiConfig}
                >
                  {t('settings.aiResetDraft')}
                </button>
              </div>
            }
          />

          <label className="checkbox-row">
            <input
              aria-label={t('settings.aiMasterToggle')}
              checked={currentAiSettings.enabled}
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
            disabled={saving}
            onAdd={() => handleAddProvider('llm')}
            onApiKeyChange={(id, value) =>
              setAiApiKeys((prev) => ({ ...prev, [id]: value }))
            }
            onClearKey={(id) => {
              void handleClearAiApiKey(id)
            }}
            onClearKeyDisabled={(providerId) =>
              saving || !persistedProviderIds.has(providerId)
            }
            onRemove={(id) => handleRemoveProvider('llm', id)}
            onSaveKey={(id) => {
              void handleSaveAiApiKey(id)
            }}
            onSaveKeyDisabled={(providerId) =>
              saving ||
              !persistedProviderIds.has(providerId) ||
              !aiApiKeys[providerId]?.trim()
            }
            onSelect={(id) => handleSelectProvider('llm', id)}
            onUpdate={(id, patch) => handleUpdateProvider('llm', id, patch)}
            providers={currentAiSettings.llmProviders}
            purpose="llm"
            selectedProviderId={currentAiSettings.llmProviderId ?? null}
            title={t('settings.aiLlmProviders')}
            translations={aiProviderTranslations}
          />

          <AiProviderEditorList
            addLabel={t('settings.aiAddEmbeddingProvider')}
            apiKeys={aiApiKeys}
            disabled={saving}
            onAdd={() => handleAddProvider('embedding')}
            onApiKeyChange={(id, value) =>
              setAiApiKeys((prev) => ({ ...prev, [id]: value }))
            }
            onClearKey={(id) => {
              void handleClearAiApiKey(id)
            }}
            onClearKeyDisabled={(providerId) =>
              saving || !persistedProviderIds.has(providerId)
            }
            onRemove={(id) => handleRemoveProvider('embedding', id)}
            onSaveKey={(id) => {
              void handleSaveAiApiKey(id)
            }}
            onSaveKeyDisabled={(providerId) =>
              saving ||
              !persistedProviderIds.has(providerId) ||
              !aiApiKeys[providerId]?.trim()
            }
            onSelect={(id) => handleSelectProvider('embedding', id)}
            onUpdate={(id, patch) =>
              handleUpdateProvider('embedding', id, patch)
            }
            providers={currentAiSettings.embeddingProviders}
            purpose="embedding"
            selectedProviderId={currentAiSettings.embeddingProviderId ?? null}
            title={t('settings.aiEmbeddingProviders')}
            translations={aiProviderTranslations}
          />

          <div className="config-row" style={{ marginTop: 'var(--space-4)' }}>
            <span className="config-label">
              {t('settings.aiActiveLlmProvider')}
            </span>
            <span className="config-value mono">
              {currentAiSettings.llmProviders.find(
                (p) => p.id === currentAiSettings.llmProviderId,
              )?.name ?? t('settings.aiNoneSelected')}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.aiActiveEmbeddingProvider')}
            </span>
            <span className="config-value mono">
              {currentAiSettings.embeddingProviders.find(
                (p) => p.id === currentAiSettings.embeddingProviderId,
              )?.name ?? t('settings.aiNoneSelected')}
            </span>
          </div>

          <StatusCallout
            tone={
              aiIndexMeta.tone === 'success'
                ? 'success'
                : aiIndexMeta.tone === 'warning'
                  ? 'warning'
                  : aiIndexMeta.tone === 'blocked'
                    ? 'blocked'
                    : 'info'
            }
            title={t('settings.aiIndexHealthTitle', {
              status: aiIndexMeta.label,
            })}
            body={aiIndexMeta.description}
          />

          <div className="settings-field-grid">
            <div className="config-row">
              <span className="config-label">
                {t('settings.aiIndexedRows')}
              </span>
              <span className="config-value mono">
                {snapshot.aiStatus.indexedItems.toLocaleString(language)}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('settings.aiSemanticSidecar')}
              </span>
              <span className="config-value mono">
                {formatBytes(snapshot.aiStatus.semanticSidecarBytes, language)}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('settings.aiSemanticMirror')}
              </span>
              <span className="config-value mono">
                {formatBytes(snapshot.aiStatus.semanticMirrorBytes, language)}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">
                {t('settings.aiEstimatedTokens')}
              </span>
              <span className="config-value mono">
                {snapshot.aiStatus.estimatedEmbeddingTokens.toLocaleString(
                  language,
                )}
              </span>
            </div>
          </div>

          {snapshot.aiStatus.warning ? (
            <div className="result-row">
              <div className="result-row__header">
                <strong>{t('settings.aiIndexWarning')}</strong>
              </div>
              <p>{snapshot.aiStatus.warning}</p>
            </div>
          ) : null}

          <div className="settings-result-list">
            {aiIntegrationError ? (
              <StatusCallout
                tone="warning"
                title={t('settings.aiIntegrationUnavailable')}
                body={aiIntegrationError}
              />
            ) : aiIntegrationPreview ? (
              <>
                <StatusCallout
                  tone={
                    aiIntegrationPreview.warnings.length > 0
                      ? 'warning'
                      : 'info'
                  }
                  title={t('settings.aiIntegrationReview')}
                  body={aiIntegrationPreview.consentSummary}
                />
                <div className="code-panel">
                  <span>{t('settings.aiMcpCommand')}</span>
                  <pre>{aiIntegrationPreview.mcpCommand}</pre>
                </div>
                <div className="result-row">
                  <div className="result-row__header">
                    <strong>{t('settings.aiCapabilityNotes')}</strong>
                  </div>
                  {aiIntegrationPreview.capabilityNotes.map((note) => (
                    <p key={note}>{note}</p>
                  ))}
                </div>
                <div className="result-row">
                  <div className="result-row__header">
                    <strong>{t('settings.aiScopeBoundary')}</strong>
                  </div>
                  {aiIntegrationPreview.scopeBoundary.map((note) => (
                    <p key={note}>{note}</p>
                  ))}
                </div>
                <div className="result-row">
                  <div className="result-row__header">
                    <strong>{t('settings.aiAuditTrace')}</strong>
                  </div>
                  {aiIntegrationPreview.auditTrace.map((note) => (
                    <p key={note}>{note}</p>
                  ))}
                </div>
                <div className="result-row">
                  <div className="result-row__header">
                    <strong>{t('settings.aiGeneratedFiles')}</strong>
                  </div>
                  {aiIntegrationPreview.generatedFiles.length > 0 ? (
                    <>
                      <div
                        className="generated-file-tabs"
                        style={{ marginBottom: 'var(--space-3)' }}
                      >
                        {aiIntegrationPreview.generatedFiles.map(
                          (file, index) => (
                            <button
                              key={file.relativePath}
                              className={`chip-button ${
                                selectedAiGeneratedFileIndex === index
                                  ? 'chip-button--active'
                                  : ''
                              }`}
                              type="button"
                              onClick={() =>
                                setSelectedAiGeneratedFileIndex(index)
                              }
                            >
                              {file.relativePath}
                            </button>
                          ),
                        )}
                      </div>
                      {selectedAiGeneratedFile ? (
                        <div className="code-panel">
                          <div className="row-between">
                            <strong>{selectedAiGeneratedFile.purpose}</strong>
                            <span className="mono dim">
                              {selectedAiGeneratedFile.relativePath}
                            </span>
                          </div>
                          <pre className="code-block">
                            <code>{selectedAiGeneratedFile.contents}</code>
                          </pre>
                          {selectedAiGeneratedFile.absolutePath ? (
                            <div className="code-actions">
                              <button
                                className="btn-tiny"
                                type="button"
                                onClick={() => {
                                  void backend.openPathInFileManager(
                                    selectedAiGeneratedFile.absolutePath ??
                                      selectedAiGeneratedFile.relativePath,
                                  )
                                }}
                              >
                                {t('common.openPath')}
                              </button>
                            </div>
                          ) : null}
                        </div>
                      ) : null}
                    </>
                  ) : null}
                </div>
                <div className="result-row">
                  <div className="result-row__header">
                    <strong>{t('settings.aiManualSteps')}</strong>
                  </div>
                  {aiIntegrationPreview.manualSteps.map((step) => (
                    <p key={step}>{step}</p>
                  ))}
                  {aiIntegrationPreview.warnings.map((warning) => (
                    <p key={warning}>{warning}</p>
                  ))}
                </div>
              </>
            ) : null}
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
          <StatusCallout
            tone="info"
            title={settingsNs('firstPartyRuntimeTitle')}
            body={settingsNs('firstPartyRuntimeBody')}
          />
          <StatusCallout
            tone={
              intelligenceRuntimeError || intelligenceRuntime?.queue.failed
                ? 'warning'
                : 'info'
            }
            title={
              intelligenceRuntimeError
                ? settingsNs('runtimeUnavailableTitle')
                : settingsNs('runtimeQueueTitle')
            }
            body={intelligenceRuntimeError ?? settingsNs('runtimeQueueBody')}
            actions={
              intelligenceRuntimeError ? undefined : (
                <div className="settings-action-row">
                  <span className="mono">
                    {settingsNs('runtimeQueueSummary', {
                      queued: intelligenceRuntime?.queue.queued ?? 0,
                      running: intelligenceRuntime?.queue.running ?? 0,
                      failed: intelligenceRuntime?.queue.failed ?? 0,
                    })}
                  </span>
                </div>
              )
            }
          />

          {reviewableDeterministicModules.map((module) => (
            <div
              key={module.state.id}
              className="result-row result-row--active"
            >
              <div className="result-row__header">
                <strong>
                  {deterministicModuleLabel(module.state.id, settingsNs)}
                </strong>
                <span className="mono">
                  {module.runtime
                    ? deterministicModuleStatusLabel(
                        module.runtime.status,
                        settingsNs,
                      )
                    : module.state.enabled
                      ? settingsNs('deterministicModuleIdle')
                      : settingsNs('deterministicModuleDisabled')}
                </span>
              </div>
              <p>
                {deterministicModuleDescription(module.state.id, settingsNs)}
              </p>
              <div className="config-row">
                <span className="config-label">
                  {settingsNs('deterministicModuleVersion')}
                </span>
                <span className="config-value mono">
                  {module.state.version}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">
                  {settingsNs('deterministicModuleDependsOn')}
                </span>
                <span className="config-value mono">
                  {module.runtime?.dependsOn.length
                    ? module.runtime.dependsOn
                        .map((moduleId) =>
                          deterministicModuleLabel(moduleId, settingsNs),
                        )
                        .join(', ')
                    : commonNs('notAvailable')}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">
                  {settingsNs('deterministicModuleTables')}
                </span>
                <span className="config-value mono">
                  {module.runtime?.derivedTables.join(', ') ??
                    commonNs('notAvailable')}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">
                  {settingsNs('deterministicModuleLastBuilt')}
                </span>
                <span className="config-value mono">
                  {module.runtime?.lastBuiltAt
                    ? (formatDateTime(module.runtime.lastBuiltAt, language) ??
                      module.runtime.lastBuiltAt)
                    : commonNs('notAvailable')}
                </span>
              </div>
              {module.runtime?.staleReason ? (
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('deterministicModuleStaleReason')}
                  </span>
                  <span className="config-value">
                    {module.runtime.staleReason}
                  </span>
                </div>
              ) : null}
              {module.runtime?.notes.length ? (
                <div className="intelligence-note-list">
                  {module.runtime.notes.map((note) => (
                    <p
                      key={`${module.state.id}-${note}`}
                      className="mono-support"
                    >
                      {note}
                    </p>
                  ))}
                </div>
              ) : null}
              <div className="settings-action-row">
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={Boolean(derivedAction)}
                  onClick={() => {
                    void handleDeterministicModuleToggle(module.state.id)
                  }}
                >
                  {module.state.enabled
                    ? t('settings.disablePlugin')
                    : t('settings.enablePlugin')}
                </button>
              </div>
            </div>
          ))}

          {reviewableEnrichmentPlugins.map((plugin) => {
            const sourceKind =
              plugin.runtime?.sourceKind ??
              (plugin.state.id === READABLE_CONTENT_REFETCH_PLUGIN_ID
                ? 'network'
                : 'local')

            return (
              <div
                key={plugin.state.id}
                className="result-row result-row--active"
              >
                <div className="result-row__header">
                  <strong>
                    {enrichmentPluginLabel(plugin.state.id, settingsNs)}
                  </strong>
                  <span className="mono">
                    {plugin.state.enabled
                      ? t('settings.enabled')
                      : t('settings.disabled')}
                  </span>
                </div>
                <p>
                  {enrichmentPluginDescription(plugin.state.id, settingsNs)}
                </p>
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('pluginBoundary')}
                  </span>
                  <span className="config-value mono">
                    {enrichmentPluginBoundaryLabel(sourceKind, settingsNs)}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.pluginVersion')}
                  </span>
                  <span className="config-value mono">
                    {plugin.state.version}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.pluginQueue')}
                  </span>
                  <span className="config-value mono">
                    {plugin.runtime
                      ? settingsNs('pluginQueueCounts', {
                          queued: plugin.runtime.queuedJobs,
                          running: plugin.runtime.runningJobs,
                          failed: plugin.runtime.failedJobs,
                        })
                      : commonNs('notAvailable')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.pluginFreshness')}
                  </span>
                  <span className="config-value mono">
                    {plugin.definition?.freshnessDays
                      ? t('settings.daysFreshness', {
                          days: plugin.definition.freshnessDays,
                        })
                      : commonNs('notAvailable')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.pluginDerivedTables')}
                  </span>
                  <span className="config-value mono">
                    {plugin.definition?.derivedTables.join(', ') ??
                      commonNs('notAvailable')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('pluginStoredRecords')}
                  </span>
                  <span className="config-value mono">
                    {plugin.runtime?.storedRecords ?? 0}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('pluginLastCompleted')}
                  </span>
                  <span className="config-value mono">
                    {plugin.runtime?.lastCompletedAt
                      ? formatDateTime(plugin.runtime.lastCompletedAt, language)
                      : commonNs('notAvailable')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('pluginLastError')}
                  </span>
                  <span className="config-value">
                    {plugin.runtime?.lastError ?? commonNs('notAvailable')}
                  </span>
                </div>
                <div className="settings-action-row">
                  <button
                    className="btn-secondary"
                    type="button"
                    disabled={Boolean(derivedAction)}
                    onClick={() => {
                      void handleEnrichmentPluginToggle(plugin.state.id)
                    }}
                  >
                    {plugin.state.enabled
                      ? t('settings.disablePlugin')
                      : t('settings.enablePlugin')}
                  </button>
                </div>
              </div>
            )
          })}

          <div className="settings-result-list">
            <div className="result-row">
              <div className="result-row__header">
                <strong>{settingsNs('runtimeRecentJobs')}</strong>
              </div>
              {intelligenceRuntime?.recentJobs.length ? (
                intelligenceRuntime.recentJobs.map((job) => (
                  <div key={job.id} className="result-row">
                    <div className="result-row__header">
                      <strong>
                        {enrichmentPluginLabel(
                          job.pluginId ?? job.jobType,
                          settingsNs,
                        )}
                      </strong>
                      <span className="mono">
                        {intelligenceRuntimeJobStateLabel(
                          job.state,
                          settingsNs,
                        )}
                      </span>
                    </div>
                    <p>
                      {job.title ?? job.url ?? job.jobType} ·{' '}
                      {settingsNs('runtimeJobAttempt', {
                        attempt: job.attempt,
                      })}
                    </p>
                    {job.lastError ? <p>{job.lastError}</p> : null}
                    <div className="settings-action-row">
                      {job.retryable ? (
                        <button
                          className="btn-secondary"
                          type="button"
                          disabled={Boolean(derivedAction)}
                          onClick={() => {
                            void handleRetryIntelligenceRuntimeJob(job.id)
                          }}
                        >
                          {settingsNs('retryRuntimeJob')}
                        </button>
                      ) : null}
                      {job.cancellable ? (
                        <button
                          className="btn-secondary"
                          type="button"
                          disabled={Boolean(derivedAction)}
                          onClick={() => {
                            void handleCancelIntelligenceRuntimeJob(job.id)
                          }}
                        >
                          {settingsNs('cancelRuntimeJob')}
                        </button>
                      ) : null}
                    </div>
                  </div>
                ))
              ) : (
                <p>{settingsNs('runtimeNoJobs')}</p>
              )}
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
            {t('settings.platformTroubleshooting')}
          </span>
        </div>
        <div className="panel-body settings-support-grid">
          <p className="dashboard-next-action">
            {t('settings.platformDescription')}
          </p>
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
