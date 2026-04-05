/* eslint-disable react-refresh/only-export-components */
import {
  startTransition,
  useDeferredValue,
  useEffect,
  useState,
  type ReactNode,
} from 'react'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
import './App.css'
import { backend } from './lib/backend'
import { createTranslator, languageLabel, resolveLanguage } from './lib/i18n'
import { formatDateTime, formatDuration } from './lib/format'
export { formatDateTime, formatDuration } from './lib/format'
import {
  readDatabaseKeyStronghold,
  storeDatabaseKeyStronghold,
} from './lib/stronghold'
import { BrowserIcon, supportedBrowsers } from './lib/browser-icons'
import {
  DataRow,
  EmptyState,
  FieldBlock,
  Glyph,
  InfoStat,
  OperationWorkflow,
  PathRow,
  PreviewEntryList,
  StatusTag,
  Surface,
  ToggleRow,
  type WorkflowStep,
} from './components/ui'
import { AiProviderEditorList } from './components/ai-provider-editor'
export {
  DataRow,
  EmptyState,
  FieldBlock,
  Glyph,
  InfoStat,
  OperationWorkflow,
  PathRow,
  PreviewEntryList,
  StatusTag,
  Surface,
  ToggleRow,
  type WorkflowStep,
} from './components/ui'
export { AiProviderEditorList } from './components/ai-provider-editor'
import type {
  AiAssistantResponse,
  AiIndexReport,
  AiIntegrationPreview,
  AiProviderConfig,
  AiProviderPurpose,
  AiSearchResponse,
  AppBuildInfo,
  ApplyResult,
  AppConfig,
  AppSnapshot,
  ArchiveMode,
  BackupReport,
  ExplainInsightRequest,
  ExportFormat,
  HealthReport,
  HistoryQueryResponse,
  ImportBatchDetail,
  InsightExplanation,
  InsightSnapshot,
  InsightStatus,
  InsightThreadDetail,
  RemoteBackupConfig,
  RemoteBackupPreview,
  RunInsightsReport,
  SchedulePlan,
  TakeoutInspection,
} from './lib/types'

type ViewId =
  | 'setup'
  | 'explorer'
  | 'analysis'
  | 'backups'
  | 'import'
  | 'settings'

type PlatformId = 'macos' | 'windows' | 'linux'

const EMPTY_REMOTE_BACKUP: RemoteBackupConfig = {
  enabled: false,
  bucket: '',
  region: '',
  endpoint: null,
  prefix: '',
  pathStyle: true,
  uploadAfterBackup: false,
  credentialsSaved: false,
  lastUploadedAt: null,
  lastUploadedObjectKey: null,
  lastError: null,
}

const EMPTY_AI_SETTINGS: AppConfig['ai'] = {
  enabled: false,
  assistantEnabled: false,
  semanticIndexEnabled: false,
  mcpEnabled: false,
  skillEnabled: false,
  autoIndexAfterBackup: false,
  llmProviderId: null,
  embeddingProviderId: null,
  retrievalTopK: 8,
  assistantSystemPrompt: '',
  llmProviders: [],
  embeddingProviders: [],
}

const EMPTY_CONFIG: AppConfig = {
  initialized: false,
  archiveMode: 'Encrypted',
  preferredLanguage: 'system',
  dueAfterHours: 72,
  scheduleCheckIntervalHours: 6,
  checkpointDays: 90,
  captureFavicons: false,
  selectedProfileIds: [],
  gitEnabled: false,
  rememberDatabaseKeyInKeyring: false,
  appAutostart: false,
  remoteBackup: EMPTY_REMOTE_BACKUP,
  ai: EMPTY_AI_SETTINGS,
}

const EMPTY_DIRECTORIES: AppSnapshot['directories'] = {
  appRoot: '',
  configPath: '',
  archiveDatabasePath: '',
  auditRepoPath: '',
  manifestsDir: '',
  exportsDir: '',
  rawSnapshotsDir: '',
  stagingDir: '',
  quarantineDir: '',
  scheduleDir: '',
  strongholdPath: '',
  strongholdSaltPath: '',
}

const EMPTY_ARCHIVE_STATUS: AppSnapshot['archiveStatus'] = {
  initialized: false,
  encrypted: true,
  unlocked: false,
  databasePath: '',
  lastSuccessfulBackupAt: null,
  warning: null,
}

const EMPTY_KEYRING_STATUS: AppSnapshot['keyringStatus'] = {
  available: false,
  backend: '',
  storedSecret: false,
  message: null,
}

const EMPTY_AI_STATUS: AppSnapshot['aiStatus'] = {
  enabled: false,
  assistantEnabled: false,
  mcpEnabled: false,
  skillEnabled: false,
  ready: false,
  indexedItems: 0,
  lastIndexedAt: null,
  llmProviderId: null,
  embeddingProviderId: null,
  warning: null,
}

const EMPTY_INSIGHT_STATUS: InsightStatus = {
  ready: false,
  lastRunAt: null,
  runs: 0,
  cards: 0,
  topics: 0,
  threads: 0,
  contentCoverage: 0,
  warning: null,
}

function App() {
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null)
  const [draftConfig, setDraftConfig] = useState<AppConfig | null>(null)
  const [history, setHistory] = useState<HistoryQueryResponse | null>(null)
  const [schedulePlan, setSchedulePlan] = useState<SchedulePlan | null>(null)
  const [scheduleApplyResult, setScheduleApplyResult] =
    useState<ApplyResult | null>(null)
  const [doctorReport, setDoctorReport] = useState<HealthReport | null>(null)
  const [takeoutInspection, setTakeoutInspection] =
    useState<TakeoutInspection | null>(null)
  const [selectedImportBatchId, setSelectedImportBatchId] = useState<
    number | null
  >(null)
  const [importBatchDetail, setImportBatchDetail] =
    useState<ImportBatchDetail | null>(null)
  const [remotePreview, setRemotePreview] =
    useState<RemoteBackupPreview | null>(null)
  const [aiIntegrationPreview, setAiIntegrationPreview] =
    useState<AiIntegrationPreview | null>(null)
  const [aiIndexReport, setAiIndexReport] = useState<AiIndexReport | null>(null)
  const [aiSearchResult, setAiSearchResult] = useState<AiSearchResponse | null>(
    null,
  )
  const [aiAssistantResult, setAiAssistantResult] =
    useState<AiAssistantResponse | null>(null)
  const [insightSnapshot, setInsightSnapshot] =
    useState<InsightSnapshot | null>(null)
  const [insightRunReport, setInsightRunReport] =
    useState<RunInsightsReport | null>(null)
  const [selectedInsightThreadId, setSelectedInsightThreadId] = useState<
    string | null
  >(null)
  const [insightThreadDetail, setInsightThreadDetail] =
    useState<InsightThreadDetail | null>(null)
  const [insightExplanation, setInsightExplanation] =
    useState<InsightExplanation | null>(null)
  const [selectedInsightLabel, setSelectedInsightLabel] = useState<
    string | null
  >(null)
  const [insightWindowDays, setInsightWindowDays] = useState(30)
  const [lastBackupReport, setLastBackupReport] = useState<BackupReport | null>(
    null,
  )
  const [busyLabel, setBusyLabel] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [activeView, setActiveView] = useState<ViewId>('setup')
  const [sessionDatabaseKey, setSessionDatabaseKey] = useState<string | null>(
    null,
  )
  const [masterPassword, setMasterPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [unlockPassword, setUnlockPassword] = useState('')
  const [rekeyPassword, setRekeyPassword] = useState('')
  const [rememberKey, setRememberKey] = useState(false)
  const [historySearchInput, setHistorySearchInput] = useState('')
  const [historyDomain, setHistoryDomain] = useState('')
  const [historyProfile, setHistoryProfile] = useState('')
  const [selectedHistoryId, setSelectedHistoryId] = useState<number | null>(
    null,
  )
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null)
  const [schedulePlatform, setSchedulePlatform] = useState<PlatformId>('macos')
  const [takeoutPath, setTakeoutPath] = useState('')
  const [s3AccessKeyId, setS3AccessKeyId] = useState('')
  const [s3SecretAccessKey, setS3SecretAccessKey] = useState('')
  const [providerSecrets, setProviderSecrets] = useState<
    Record<string, string>
  >({})
  const [aiSearchInput, setAiSearchInput] = useState('')
  const [aiSearchDomain, setAiSearchDomain] = useState('')
  const [aiSearchProfile, setAiSearchProfile] = useState('')
  const [aiQuestionInput, setAiQuestionInput] = useState('')
  const [workflowChecks, setWorkflowChecks] = useState<Record<string, boolean>>(
    {},
  )
  const deferredSearch = useDeferredValue(historySearchInput)

  const resolvedLanguage = resolveLanguage(
    draftConfig?.preferredLanguage ?? snapshot?.config.preferredLanguage,
  )
  const t = createTranslator(resolvedLanguage)
  const initialized = snapshot?.config.initialized ?? false
  const unlocked = snapshot?.archiveStatus.unlocked ?? false
  const currentConfig = draftConfig ?? snapshot?.config ?? EMPTY_CONFIG
  const aiConfig = currentConfig.ai
  const remoteBackupConfig = currentConfig.remoteBackup
  const directories = snapshot?.directories ?? EMPTY_DIRECTORIES
  const archiveStatus = snapshot?.archiveStatus ?? EMPTY_ARCHIVE_STATUS
  const keyringStatus = snapshot?.keyringStatus ?? EMPTY_KEYRING_STATUS
  const aiStatus = snapshot?.aiStatus ?? EMPTY_AI_STATUS
  const insightStatus = snapshot?.insightStatus ?? EMPTY_INSIGHT_STATUS

  useEffect(() => {
    void (async () => {
      const [next, nextBuildInfo] = await Promise.all([
        backend.getAppSnapshot(),
        backend.getAppBuildInfo(),
      ])
      setBuildInfo(nextBuildInfo)
      const language = resolveLanguage(next.config.preferredLanguage)
      const translate = createTranslator(language)

      if (
        next.config.archiveMode === 'Encrypted' &&
        !next.archiveStatus.unlocked
      ) {
        const rememberedKey = await backend.keyringGetDatabaseKey()
        if (rememberedKey) {
          await backend.setSessionDatabaseKey(rememberedKey)
          const unlockedSnapshot = await backend.getAppSnapshot()
          setSessionDatabaseKey(rememberedKey)
          setSnapshot(unlockedSnapshot)
          setDraftConfig(unlockedSnapshot.config)
          setRememberKey(unlockedSnapshot.config.rememberDatabaseKeyInKeyring)
          setActiveView(
            unlockedSnapshot.config.initialized ? 'explorer' : 'setup',
          )
          startTransition(() => setNotice(translate('autoUnlockedNotice')))
          return
        }
      }

      setSnapshot(next)
      setDraftConfig(next.config)
      setRememberKey(next.config.rememberDatabaseKeyInKeyring)
      setActiveView(next.config.initialized ? 'explorer' : 'setup')
    })()
  }, [])

  useEffect(() => {
    if (activeView === 'explorer' && unlocked) {
      void (async () => {
        const response = await backend.queryHistory({
          q: deferredSearch || null,
          domain: historyDomain || null,
          profileId: historyProfile || null,
          limit: 160,
        })
        setHistory(response)
      })()
    }
  }, [activeView, deferredSearch, historyDomain, historyProfile, unlocked])

  useEffect(() => {
    if (!snapshot?.recentRuns.length) {
      setSelectedRunId(null)
      return
    }

    const stillExists = snapshot.recentRuns.some(
      (run) => run.id === selectedRunId,
    )
    if (!stillExists) {
      setSelectedRunId(snapshot.recentRuns[0].id)
    }
  }, [selectedRunId, snapshot?.recentRuns])

  useEffect(() => {
    if (!history?.items.length) {
      setSelectedHistoryId(null)
      return
    }

    const stillExists = history.items.some(
      (item) => item.id === selectedHistoryId,
    )
    if (!stillExists) {
      setSelectedHistoryId(history.items[0].id)
    }
  }, [history, selectedHistoryId])

  useEffect(() => {
    if (!snapshot?.recentImportBatches.length) {
      setSelectedImportBatchId(null)
      setImportBatchDetail(null)
      return
    }

    const stillExists = snapshot.recentImportBatches.some(
      (batch) => batch.id === selectedImportBatchId,
    )
    if (!stillExists) {
      setSelectedImportBatchId(snapshot.recentImportBatches[0].id)
    }
  }, [selectedImportBatchId, snapshot?.recentImportBatches])

  useEffect(() => {
    if (activeView !== 'import' || selectedImportBatchId == null) {
      return
    }

    let cancelled = false

    void (async () => {
      try {
        const detail = await backend.previewImportBatch(selectedImportBatchId)
        /* v8 ignore next 3 -- only relevant during effect teardown races */
        if (!cancelled) {
          setImportBatchDetail(detail)
        }
      } catch (taskError) {
        /* v8 ignore next 5 -- only relevant during effect teardown races */
        if (!cancelled) {
          /* v8 ignore next -- non-Error throws are a defensive fallback */
          setError(
            taskError instanceof Error ? taskError.message : String(taskError),
          )
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeView, selectedImportBatchId])

  useEffect(() => {
    if (activeView !== 'analysis' || !initialized || !unlocked) {
      return
    }

    let cancelled = false

    void (async () => {
      try {
        const next = await backend.loadInsights({
          profileId: aiSearchProfile || null,
          windowDays: insightWindowDays,
          fullRebuild: false,
          limit: null,
        })
        if (!cancelled) {
          setInsightSnapshot(next)
        }
      } catch (taskError) {
        if (!cancelled) {
          setError(
            taskError instanceof Error ? taskError.message : String(taskError),
          )
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeView, aiSearchProfile, initialized, insightWindowDays, unlocked])

  useEffect(() => {
    if (!insightSnapshot?.threads.length) {
      setSelectedInsightThreadId(null)
      setInsightThreadDetail(null)
      return
    }

    const stillExists = insightSnapshot.threads.some(
      (thread) => thread.threadId === selectedInsightThreadId,
    )
    if (!stillExists) {
      setSelectedInsightThreadId(insightSnapshot.threads[0].threadId)
    }
  }, [insightSnapshot, selectedInsightThreadId])

  useEffect(() => {
    if (
      activeView !== 'analysis' ||
      !initialized ||
      !unlocked ||
      !selectedInsightThreadId
    ) {
      return
    }

    let cancelled = false

    void (async () => {
      try {
        const detail = await backend.loadThreadDetail(selectedInsightThreadId)
        if (!cancelled) {
          setInsightThreadDetail(detail)
        }
      } catch (taskError) {
        if (!cancelled) {
          setError(
            taskError instanceof Error ? taskError.message : String(taskError),
          )
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeView, initialized, selectedInsightThreadId, unlocked])

  useEffect(() => {
    setInsightExplanation(null)
    setSelectedInsightLabel(null)
  }, [aiSearchProfile, insightWindowDays])

  const selectedHistory =
    history?.items.find((item) => item.id === selectedHistoryId) ??
    history?.items[0] ??
    null
  const selectedRun =
    snapshot?.recentRuns.find((run) => run.id === selectedRunId) ??
    snapshot?.recentRuns[0] ??
    null
  const selectedImportBatch =
    snapshot?.recentImportBatches.find(
      (batch) => batch.id === selectedImportBatchId,
    ) ??
    snapshot?.recentImportBatches[0] ??
    null
  const selectedInsightThread =
    insightSnapshot?.threads.find(
      (thread) => thread.threadId === selectedInsightThreadId,
    ) ??
    insightSnapshot?.threads[0] ??
    null
  const llmProviders = aiConfig.llmProviders
  const embeddingProviders = aiConfig.embeddingProviders
  const selectedHistoryInspectorTitle =
    selectedHistory?.title ?? selectedHistory?.url ?? t('notAvailable')
  const selectedHistoryTitle = selectedHistory?.title ?? t('notAvailable')
  const selectedHistoryTransition =
    selectedHistory?.transition == null
      ? t('notAvailable')
      : String(selectedHistory.transition)
  const remoteBackupLastUploaded =
    formatDateTime(remoteBackupConfig.lastUploadedAt, resolvedLanguage) ??
    t('noRemoteUploadYet')
  const remoteBackupInspectorLastUploaded =
    formatDateTime(remoteBackupConfig.lastUploadedAt, resolvedLanguage) ??
    t('notAvailable')
  const buildVersion = buildInfo?.version ?? t('notAvailable')
  const buildCommit = buildInfo?.gitCommitShort ?? t('notAvailable')
  const buildState = buildInfo
    ? buildInfo.gitDirty
      ? t('workingTreeDirty')
      : t('workingTreeClean')
    : t('notAvailable')
  const insightGeneratedAt =
    formatDateTime(
      insightSnapshot?.generatedAt ?? insightStatus.lastRunAt,
      resolvedLanguage,
    ) ?? t('notAvailable')
  const insightCoverage = `${Math.round(insightStatus.contentCoverage * 100)}%`

  const viewMeta: Record<
    ViewId,
    {
      label: string
      description: string
      icon: string
      disabled: boolean
    }
  > = {
    setup: {
      label: t('setupNav'),
      description: t('setupDescription'),
      icon: 'settings_input_component',
      disabled: false,
    },
    explorer: {
      label: t('explorerNav'),
      description: t('explorerDescription'),
      icon: 'search',
      disabled: !initialized,
    },
    analysis: {
      label: t('analysisNav'),
      description: t('analysisDescription'),
      icon: 'neurology',
      disabled: !initialized,
    },
    backups: {
      label: t('backupsNav'),
      description: t('backupsDescription'),
      icon: 'fact_check',
      disabled: !initialized,
    },
    import: {
      label: t('importNav'),
      description: t('importDescription'),
      icon: 'upload_file',
      disabled: !initialized,
    },
    settings: {
      label: t('settingsNav'),
      description: t('settingsDescription'),
      icon: 'settings',
      disabled: false,
    },
  }

  async function runTask(label: string, action: () => Promise<void>) {
    setBusyLabel(label)
    setError(null)
    try {
      await action()
    } catch (taskError) {
      /* v8 ignore next -- non-Error throws are a defensive fallback */
      setError(
        taskError instanceof Error ? taskError.message : String(taskError),
      )
    } finally {
      setBusyLabel(null)
    }
  }

  function setLocalizedNotice(message: string) {
    startTransition(() => setNotice(message))
  }

  function generateDatabaseKey() {
    const bytes = crypto.getRandomValues(new Uint8Array(32))
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join(
      '',
    )
  }

  function updateConfig(patch: Partial<AppConfig>) {
    /* v8 ignore next -- controls that use this only render after config loads */
    setDraftConfig((current) => (current ? { ...current, ...patch } : current))
  }

  function updateRemoteBackup(patch: Partial<RemoteBackupConfig>) {
    /* v8 ignore next -- controls that use this only render after config loads */
    setDraftConfig((current) =>
      current
        ? {
            ...current,
            remoteBackup: {
              ...current.remoteBackup,
              ...patch,
            },
          }
        : current,
    )
  }

  function updateAiSettings(patch: Partial<AppConfig['ai']>) {
    /* v8 ignore next -- controls that use this only render after config loads */
    setDraftConfig((current) =>
      current
        ? {
            ...current,
            ai: {
              ...current.ai,
              ...patch,
            },
          }
        : current,
    )
  }

  function updateAiProviderCollection(
    purpose: AiProviderPurpose,
    updater: (providers: AiProviderConfig[]) => AiProviderConfig[],
  ) {
    setDraftConfig((current) => {
      /* v8 ignore next -- provider editors only render after config loads */
      if (!current) {
        return current
      }
      const key =
        purpose === 'llm'
          ? ('llmProviders' as const)
          : ('embeddingProviders' as const)
      return {
        ...current,
        ai: {
          ...current.ai,
          [key]: updater(current.ai[key]),
        },
      }
    })
  }

  function addAiProvider(purpose: AiProviderPurpose) {
    const id = `${purpose}-${crypto.randomUUID().slice(0, 8)}`
    updateAiProviderCollection(purpose, (providers) => [
      ...providers,
      {
        id,
        name: purpose === 'llm' ? 'New LLM provider' : 'New embedding provider',
        purpose,
        requestFormat: 'openai',
        enabled: false,
        baseUrl: null,
        apiKeySaved: false,
        defaultModel: '',
        modelCatalog: [],
        temperature: purpose === 'llm' ? 0.2 : null,
        maxTokens: purpose === 'llm' ? 1200 : null,
        dimensions: purpose === 'embedding' ? 1536 : null,
        notes: null,
      },
    ])
  }

  function updateAiProvider(
    purpose: AiProviderPurpose,
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) {
    updateAiProviderCollection(purpose, (providers) =>
      providers.map((provider) =>
        provider.id === providerId ? { ...provider, ...patch } : provider,
      ),
    )
  }

  function removeAiProvider(purpose: AiProviderPurpose, providerId: string) {
    updateAiProviderCollection(purpose, (providers) =>
      providers.filter((provider) => provider.id !== providerId),
    )
    if (purpose === 'llm' && aiConfig.llmProviderId === providerId) {
      updateAiSettings({ llmProviderId: null })
    }
    if (
      purpose === 'embedding' &&
      aiConfig.embeddingProviderId === providerId
    ) {
      updateAiSettings({ embeddingProviderId: null })
    }
    setProviderSecrets((current) => {
      const next = { ...current }
      delete next[providerId]
      return next
    })
  }

  async function copyText(value: string) {
    await navigator.clipboard.writeText(value)
    setLocalizedNotice(t('copiedNotice'))
  }

  async function handleOpenPath(path: string | null | undefined) {
    if (!path) {
      return
    }

    await runTask(t('openAction'), async () => {
      const openedPath = await backend.openPathInFileManager(path)
      setLocalizedNotice(t('openedDirectoryNotice', { path: openedPath }))
    })
  }

  async function syncAppAutostart(nextConfig: AppConfig) {
    try {
      const enabledNow = await isEnabled()
      if (nextConfig.appAutostart && !enabledNow) {
        await enable()
      }
      if (!nextConfig.appAutostart && enabledNow) {
        await disable()
      }
    } catch {
      // Unsupported during web preview.
    }
  }

  async function reloadSnapshot() {
    const next = await backend.getAppSnapshot()
    setSnapshot(next)
    setDraftConfig(next.config)
    setRememberKey(next.config.rememberDatabaseKeyInKeyring)
  }

  async function reloadInsights() {
    const next = await backend.loadInsights({
      profileId: aiSearchProfile || null,
      windowDays: insightWindowDays,
      fullRebuild: false,
      limit: null,
    })
    setInsightSnapshot(next)
  }

  async function persistConfig(nextConfig: AppConfig) {
    await syncAppAutostart(nextConfig)
    const nextSnapshot = await backend.saveConfig(nextConfig)
    setSnapshot(nextSnapshot)
    setDraftConfig(nextSnapshot.config)
    setRememberKey(nextSnapshot.config.rememberDatabaseKeyInKeyring)
    setLocalizedNotice(t('preferencesSavedNotice'))
  }

  function strongholdPath() {
    return directories.strongholdPath
  }

  async function handleInitialize() {
    /* v8 ignore next -- the setup form is only actionable after config loads */
    if (!draftConfig) {
      return
    }

    await runTask(t('createArchive'), async () => {
      const nextConfig: AppConfig = {
        ...draftConfig,
        initialized: true,
        rememberDatabaseKeyInKeyring: rememberKey,
      }

      let databaseKey: string | null = null

      if (nextConfig.archiveMode === 'Encrypted') {
        if (!masterPassword || masterPassword !== confirmPassword) {
          throw new Error(t('matchingPasswordsRequired'))
        }
        databaseKey = generateDatabaseKey()
        await backend.resetLocalSecretVault()
        await storeDatabaseKeyStronghold(
          masterPassword,
          databaseKey,
          strongholdPath(),
        )
        if (rememberKey) {
          await backend.keyringStoreDatabaseKey(databaseKey)
        } else {
          await backend.keyringClearDatabaseKey()
        }
      } else {
        await backend.resetLocalSecretVault()
        await backend.keyringClearDatabaseKey()
      }

      await syncAppAutostart(nextConfig)
      const nextSnapshot = await backend.initializeArchive(
        nextConfig,
        databaseKey,
      )
      if (databaseKey) {
        await backend.setSessionDatabaseKey(databaseKey)
      }
      setSessionDatabaseKey(databaseKey)
      setSnapshot(nextSnapshot)
      setDraftConfig(nextSnapshot.config)
      setRememberKey(nextSnapshot.config.rememberDatabaseKeyInKeyring)
      setActiveView('explorer')
      setMasterPassword('')
      setConfirmPassword('')
      setLocalizedNotice(t('initializedNotice'))
    })
  }

  async function handleSaveSetup() {
    /* v8 ignore next -- the save action is only reachable after config loads */
    if (!draftConfig) {
      return
    }

    await runTask(t('saveSetup'), async () => {
      await persistConfig({
        ...draftConfig,
        rememberDatabaseKeyInKeyring: rememberKey,
      })
    })
  }

  async function handleSaveSettings() {
    /* v8 ignore next -- the save action is only reachable after config loads */
    if (!draftConfig) {
      return
    }

    await runTask(t('saveSettings'), async () => {
      await persistConfig({
        ...draftConfig,
        rememberDatabaseKeyInKeyring: rememberKey,
      })
    })
  }

  async function handleUnlockWithPassword() {
    if (!unlockPassword) {
      setError(t('enterMasterPassword'))
      return
    }

    await runTask(t('unlockArchive'), async () => {
      const databaseKey = await readDatabaseKeyStronghold(
        unlockPassword,
        strongholdPath(),
      )
      if (!databaseKey) {
        throw new Error(
          'No database key was found in the Stronghold snapshot for that password.',
        )
      }
      await backend.setSessionDatabaseKey(databaseKey)
      setSessionDatabaseKey(databaseKey)
      await reloadSnapshot()
      setUnlockPassword('')
      setLocalizedNotice(t('unlockSuccess'))
    })
  }

  async function handleRotateEncryption() {
    if (!sessionDatabaseKey) {
      setError(t('unlockBeforeRotate'))
      return
    }
    if (!rekeyPassword) {
      setError(t('enterNewMasterPassword'))
      return
    }

    await runTask(t('rotateKey'), async () => {
      const newDatabaseKey = generateDatabaseKey()
      await backend.rekeyArchive({
        newMode: 'Encrypted',
        newKey: newDatabaseKey,
      })
      await backend.resetLocalSecretVault()
      await storeDatabaseKeyStronghold(
        rekeyPassword,
        newDatabaseKey,
        strongholdPath(),
      )
      if (rememberKey) {
        await backend.keyringStoreDatabaseKey(newDatabaseKey)
      } else {
        await backend.keyringClearDatabaseKey()
      }
      await backend.setSessionDatabaseKey(newDatabaseKey)
      setSessionDatabaseKey(newDatabaseKey)
      setRekeyPassword('')
      await reloadSnapshot()
      setLocalizedNotice(t('rotateSuccess'))
    })
  }

  async function handleSwitchToPlaintext() {
    await runTask(t('convertToPlaintext'), async () => {
      await backend.rekeyArchive({
        newMode: 'Plaintext',
        newKey: null,
      })
      await backend.keyringClearDatabaseKey()
      await backend.resetLocalSecretVault()
      await backend.clearSessionDatabaseKey()
      setSessionDatabaseKey(null)
      await reloadSnapshot()
      setRememberKey(false)
      setLocalizedNotice(t('plaintextSuccess'))
    })
  }

  async function handleBackupRun() {
    await runTask(t('runBackupNow'), async () => {
      const report = await backend.runBackupNow(false)
      setLastBackupReport(report)
      await reloadSnapshot()
      setActiveView('backups')
      if (report.dueSkipped) {
        setLocalizedNotice(report.reason ?? t('backupComplete'))
        return
      }

      const baseNotice = report.manifestPath
        ? t('backupCompleteWithManifest', { path: report.manifestPath })
        : t('backupComplete')
      const nextNotice = report.remoteBackup
        ? `${baseNotice} ${report.remoteBackup.message}`
        : baseNotice
      setLocalizedNotice(nextNotice)
    })
  }

  async function handlePreviewSchedule() {
    await runTask(t('previewSchedule'), async () => {
      const plan = await backend.previewSchedule(schedulePlatform)
      setSchedulePlan(plan)
      setScheduleApplyResult(null)
      setLocalizedNotice(t('schedulePreviewReady'))
    })
  }

  async function handleApplySchedule() {
    if (!schedulePlan) {
      setError(t('generateSchedulePreviewFirst'))
      return
    }

    await runTask(t('applyPreview'), async () => {
      const result = await backend.applySchedule(schedulePlan)
      setScheduleApplyResult(result)
      await reloadSnapshot()
      setLocalizedNotice(result.message)
    })
  }

  async function handleExport(format: ExportFormat) {
    await runTask(`${t('exportLabel')} ${format.toUpperCase()}`, async () => {
      const result = await backend.exportHistory({
        format,
        query: {
          q: deferredSearch || null,
          domain: historyDomain || null,
          profileId: historyProfile || null,
          limit: 200,
        },
      })
      setLocalizedNotice(
        `${t('exportLabel')} ${result.count} -> ${result.path}`,
      )
    })
  }

  async function handleTakeout(dryRun: boolean) {
    if (!takeoutPath) {
      setError(t('enterTakeoutPath'))
      return
    }

    await runTask(dryRun ? t('dryRun') : t('importSupported'), async () => {
      const request = {
        sourcePath: takeoutPath,
        dryRun,
      }
      const response = dryRun
        ? await backend.inspectTakeout(request)
        : await backend.importTakeout(request)
      setTakeoutInspection(response)
      /* v8 ignore next -- tests cover both paths; this guard only skips follow-up preview work */
      if (!dryRun) {
        await reloadSnapshot()
        if (response.importBatch) {
          setSelectedImportBatchId(response.importBatch.id)
          const detail = await backend.previewImportBatch(
            response.importBatch.id,
          )
          setImportBatchDetail(detail)
        }
      }
      setLocalizedNotice(
        dryRun
          ? t('takeoutDryRunNotice')
          : t('takeoutImportNotice', { count: response.importedItems }),
      )
    })
  }

  async function handlePreviewImportBatch(batchId: number) {
    await runTask(t('previewBatch'), async () => {
      const detail = await backend.previewImportBatch(batchId)
      setSelectedImportBatchId(batchId)
      setImportBatchDetail(detail)
      setLocalizedNotice(t('previewBatchReady'))
    })
  }

  async function handleRevertImportBatch(batchId: number) {
    if (!window.confirm(t('revertBatchConfirm'))) {
      return
    }

    await runTask(t('revertBatch'), async () => {
      const detail = await backend.revertImportBatch(batchId)
      setImportBatchDetail(detail)
      /* v8 ignore next -- depends on whether the current preview matches the reverted batch */
      setTakeoutInspection((current) =>
        current && current.importBatch?.id === batchId
          ? { ...current, importBatch: detail.batch }
          : current,
      )
      await reloadSnapshot()
      setSelectedImportBatchId(batchId)
      setLocalizedNotice(t('revertBatchNotice'))
    })
  }

  async function handleDoctor() {
    await runTask(t('runDoctor'), async () => {
      const report = await backend.doctor()
      setDoctorReport(report)
      setLocalizedNotice(t('doctorUpdated'))
    })
  }

  async function handleRememberCurrentKey() {
    if (!sessionDatabaseKey) {
      setError(t('unlockBeforeRemember'))
      return
    }

    await runTask(t('storeRememberedKey'), async () => {
      const report = await backend.keyringStoreDatabaseKey(sessionDatabaseKey)
      /* v8 ignore next -- the keyring report only updates after snapshot exists */
      setSnapshot((current) =>
        current ? { ...current, keyringStatus: report } : current,
      )
      setLocalizedNotice(t('rememberStored'))
    })
  }

  async function handleClearRememberedKey() {
    await runTask(t('clearRememberedKey'), async () => {
      const report = await backend.keyringClearDatabaseKey()
      /* v8 ignore next -- the keyring report only updates after snapshot exists */
      setSnapshot((current) =>
        current ? { ...current, keyringStatus: report } : current,
      )
      setLocalizedNotice(t('rememberCleared'))
    })
  }

  async function handleStoreS3Credentials() {
    if (!s3AccessKeyId.trim() || !s3SecretAccessKey.trim()) {
      setError(t('enterS3Credentials'))
      return
    }

    await runTask(t('saveCredentials'), async () => {
      await backend.storeS3Credentials({
        accessKeyId: s3AccessKeyId.trim(),
        secretAccessKey: s3SecretAccessKey.trim(),
      })
      setS3AccessKeyId('')
      setS3SecretAccessKey('')
      await reloadSnapshot()
      setLocalizedNotice(t('s3CredentialsStored'))
    })
  }

  async function handleClearS3Credentials() {
    await runTask(t('clearCredentials'), async () => {
      await backend.clearS3Credentials()
      setS3AccessKeyId('')
      setS3SecretAccessKey('')
      await reloadSnapshot()
      setLocalizedNotice(t('s3CredentialsCleared'))
    })
  }

  async function handlePreviewRemoteBackup() {
    await runTask(t('previewUpload'), async () => {
      const preview = await backend.previewRemoteBackup()
      setRemotePreview(preview)
      setLocalizedNotice(t('s3PreviewReady'))
    })
  }

  async function handleRunRemoteBackup() {
    await runTask(t('uploadNow'), async () => {
      const result = await backend.runRemoteBackup()
      await reloadSnapshot()
      setLocalizedNotice(result.message)
    })
  }

  async function handleStoreProviderSecret(providerId: string) {
    const apiKey = providerSecrets[providerId]?.trim()
    if (!apiKey) {
      setError(t('enterProviderApiKey'))
      return
    }

    await runTask(t('saveProviderKey'), async () => {
      const nextSnapshot = await backend.storeAiProviderApiKey({
        providerId,
        apiKey,
      })
      setSnapshot(nextSnapshot)
      setDraftConfig(nextSnapshot.config)
      setProviderSecrets((current) => ({ ...current, [providerId]: '' }))
      setLocalizedNotice(t('providerKeyStored'))
    })
  }

  async function handleClearProviderSecret(providerId: string) {
    await runTask(t('clearProviderKey'), async () => {
      const nextSnapshot = await backend.clearAiProviderApiKey(providerId)
      setSnapshot(nextSnapshot)
      setDraftConfig(nextSnapshot.config)
      setLocalizedNotice(t('providerKeyCleared'))
    })
  }

  async function handleBuildAiIndex(fullRebuild: boolean) {
    await runTask(t('buildAiIndex'), async () => {
      const report = await backend.buildAiIndex({
        providerId: aiConfig.embeddingProviderId ?? null,
        fullRebuild,
        limit: null,
      })
      setAiIndexReport(report)
      await reloadSnapshot()
      setLocalizedNotice(
        t('aiIndexReadyNotice', {
          count: report.indexedItems + report.updatedItems,
        }),
      )
    })
  }

  async function handleRunAiSearch() {
    if (!aiSearchInput.trim()) {
      setError(t('enterAiSearchQuery'))
      return
    }

    await runTask(t('runSemanticSearch'), async () => {
      const response = await backend.searchAiHistory({
        query: aiSearchInput,
        domain: aiSearchDomain || null,
        profileId: aiSearchProfile || null,
        limit: 12,
      })
      setAiSearchResult(response)
    })
  }

  async function handleAskAiAssistant() {
    if (!aiQuestionInput.trim()) {
      setError(t('enterAiQuestion'))
      return
    }

    await runTask(t('askAssistant'), async () => {
      const response = await backend.askAiAssistant({
        question: aiQuestionInput,
        domain: aiSearchDomain || null,
        profileId: aiSearchProfile || null,
      })
      setAiAssistantResult(response)
    })
  }

  async function handlePreviewAiIntegrations() {
    await runTask(t('previewIntegrations'), async () => {
      const preview = await backend.previewAiIntegrations()
      setAiIntegrationPreview(preview)
      setLocalizedNotice(t('integrationPreviewReady'))
    })
  }

  async function handleRunInsights(fullRebuild: boolean) {
    await runTask(
      fullRebuild ? 'Rebuild insights' : 'Refresh insights',
      async () => {
        const report = await backend.runInsightsNow({
          profileId: aiSearchProfile || null,
          windowDays: insightWindowDays,
          fullRebuild,
          limit: null,
        })
        setInsightRunReport(report)
        await reloadSnapshot()
        await reloadInsights()
        setLocalizedNotice(
          fullRebuild ? 'Insights rebuilt.' : 'Insights refreshed.',
        )
      },
    )
  }

  async function handleExplainInsight(
    insightId: string,
    insightKind: ExplainInsightRequest['insightKind'],
    label: string,
  ) {
    await runTask('Explain insight', async () => {
      const explanation = await backend.explainInsight({
        insightId,
        insightKind,
        profileId: aiSearchProfile || null,
        windowDays: insightWindowDays,
      })
      setInsightExplanation(explanation)
      setSelectedInsightLabel(label)
    })
  }

  function toggleProfile(profileId: string) {
    /* v8 ignore next -- profile toggles only render after snapshot/config load */
    if (!draftConfig || !snapshot) {
      return
    }

    const allIds = snapshot.browserProfiles
      .filter((profile) => profile.historyExists)
      .map((profile) => profile.profileId)
    const selectedIds =
      draftConfig.selectedProfileIds.length > 0
        ? draftConfig.selectedProfileIds
        : allIds
    const nextSelected = selectedIds.includes(profileId)
      ? selectedIds.filter((id) => id !== profileId)
      : [...selectedIds, profileId]

    updateConfig({
      selectedProfileIds:
        nextSelected.length === allIds.length ? [] : nextSelected,
    })
  }

  function isProfileSelected(profileId: string) {
    /* v8 ignore next -- profile toggles only render after snapshot/config load */
    if (!draftConfig || !snapshot) {
      return false
    }
    return (
      draftConfig.selectedProfileIds.length === 0 ||
      draftConfig.selectedProfileIds.includes(profileId)
    )
  }

  function toggleWorkflowCheck(id: string) {
    setWorkflowChecks((current) => ({
      ...current,
      [id]: !current[id],
    }))
  }

  function workflowChecked(id: string) {
    return Boolean(workflowChecks[id])
  }

  function selectedProfileCount() {
    if (!snapshot) {
      return 0
    }
    return snapshot.browserProfiles.filter(
      (profile) =>
        profile.historyExists && isProfileSelected(profile.profileId),
    ).length
  }

  function selectedProfilePaths() {
    return (
      snapshot?.browserProfiles
        .filter((profile) => isProfileSelected(profile.profileId))
        .map((profile) => profile.historyPath ?? profile.profilePath) ?? []
    )
  }

  function selectedProfileCommands() {
    const stagingDir = directories.stagingDir || '/tmp'
    return (
      snapshot?.browserProfiles
        .filter((profile) => isProfileSelected(profile.profileId))
        .slice(0, 3)
        .map((profile) => {
          const source = profile.historyPath ?? profile.profilePath
          return `cp '${source}' '${stagingDir}/${profile.profileId.replaceAll(':', '-')}-${profile.historyFileName}'`
        }) ?? []
    )
  }

  function formatBatchStatus(status: string) {
    switch (status) {
      case 'imported':
        return t('importedStatus')
      case 'reverted':
        return t('revertedStatus')
      case 'running':
        return t('runningStatus')
      default:
        return status
    }
  }

  function currentImportSummary() {
    if (takeoutInspection) {
      return {
        candidateItems: takeoutInspection.candidateItems,
        importedItems: takeoutInspection.importedItems,
        duplicateItems: takeoutInspection.duplicateItems,
        visibleItems:
          takeoutInspection.importBatch?.visibleItems ??
          selectedImportBatch?.visibleItems ??
          0,
      }
    }

    if (selectedImportBatch) {
      return {
        candidateItems: selectedImportBatch.candidateItems,
        importedItems: selectedImportBatch.importedItems,
        duplicateItems: selectedImportBatch.duplicateItems,
        visibleItems: selectedImportBatch.visibleItems,
      }
    }

    return null
  }

  const statusItems = snapshot
    ? [
        t('localOnly'),
        archiveStatus.encrypted ? t('encrypted') : t('plaintext'),
        archiveStatus.unlocked ? t('unlocked') : t('locked'),
        t('profilesDetected', { count: snapshot.browserProfiles.length }),
        t('dueEveryHours', { hours: currentConfig.dueAfterHours }),
      ]
    : []
  const buildStampItems = buildInfo
    ? [
        t('versionValue', { value: buildInfo.version }),
        t('commitValue', { value: buildInfo.gitCommitShort }),
      ]
    : []
  const importSummary = currentImportSummary()

  const activeMeta = viewMeta[activeView]
  const selectedProfilesTotal = selectedProfileCount()
  const sourceFilePreview = selectedProfilePaths()
  const sourceManualCommands = selectedProfileCommands()
  const schedulePreviewReady = schedulePlan != null
  const scheduleVerified =
    Boolean(scheduleApplyResult?.applied) || workflowChecked('schedule-verify')
  const scheduleWorkflowSteps: WorkflowStep[] = [
    {
      id: 'schedule-preview',
      title: t('reviewPlan'),
      status: schedulePreviewReady ? 'complete' : 'pending',
      summary: schedulePlan
        ? schedulePlan.label
        : t('generateSchedulePreviewFirst'),
      reason: t('scheduleDescription'),
      files:
        schedulePlan?.generatedFiles.map(
          (file) => file.absolutePath || file.relativePath,
        ) ?? [],
      commands:
        schedulePlan?.applyCommands.map((command) => command.join(' ')) ?? [],
      actions: (
        <button
          className="secondaryButton"
          type="button"
          onClick={handlePreviewSchedule}
        >
          {t('previewSchedule')}
        </button>
      ),
    },
    {
      id: 'schedule-manual',
      title: t('manualPathTitle'),
      status: workflowChecked('schedule-manual') ? 'complete' : 'pending',
      summary: t('manualPathSummary'),
      reason: t('manualPathReason'),
      checklist: schedulePlan?.manualSteps ?? [],
      commands:
        schedulePlan?.generatedFiles.map(
          (file) => `cat <<'EOF' > ${file.relativePath}\n${file.contents}\nEOF`,
        ) ?? [],
      actions: (
        <button
          className="ghostButton"
          type="button"
          onClick={() => toggleWorkflowCheck('schedule-manual')}
        >
          {workflowChecked('schedule-manual')
            ? t('stepCompleted')
            : t('markStepComplete')}
        </button>
      ),
    },
    {
      id: 'schedule-apply',
      title: t('applyChanges'),
      status: scheduleApplyResult?.applied ? 'complete' : 'pending',
      summary: scheduleApplyResult?.message ?? t('applyChangesSummary'),
      reason: t('applyChangesReason'),
      files: scheduleApplyResult?.files ?? [],
      actions: (
        <button
          className="primaryButton"
          type="button"
          disabled={!schedulePlan}
          onClick={handleApplySchedule}
        >
          {t('applyPreview')}
        </button>
      ),
    },
    {
      id: 'schedule-verify',
      title: t('verifyOutcome'),
      status: scheduleVerified ? 'complete' : 'pending',
      summary: scheduleApplyResult?.auditPath ?? t('verifyOutcomeSummary'),
      reason: t('verifyOutcomeReason'),
      checklist:
        schedulePlan?.rollbackCommands.map((command) => command.join(' ')) ??
        [],
      actions: (
        <button
          className="ghostButton"
          type="button"
          onClick={() => toggleWorkflowCheck('schedule-verify')}
        >
          {workflowChecked('schedule-verify')
            ? t('stepCompleted')
            : t('markStepComplete')}
        </button>
      ),
    },
    {
      id: 'schedule-finish',
      title: t('finishStep'),
      status: workflowChecked('schedule-finish') ? 'complete' : 'pending',
      summary:
        scheduleApplyResult?.auditPath ??
        schedulePlan?.generatedFiles[0]?.absolutePath ??
        t('finishSummary'),
      reason: t('finishReason'),
      files: scheduleApplyResult?.files ?? [],
      actions: (
        <button
          className="ghostButton"
          type="button"
          onClick={() => toggleWorkflowCheck('schedule-finish')}
        >
          {workflowChecked('schedule-finish')
            ? t('stepCompleted')
            : t('markStepComplete')}
        </button>
      ),
    },
  ]
  const importManualCommands = takeoutPath
    ? takeoutPath.endsWith('.zip')
      ? [`unzip -l '${takeoutPath}' | head -n 60`]
      : [`find '${takeoutPath}' -maxdepth 4 -type f | sort | head -n 80`]
    : []
  const importPreviewReady = takeoutInspection != null
  const importApplied = Boolean(takeoutInspection?.importBatch)
  const importVerified =
    importBatchDetail != null || workflowChecked('import-verify')
  const importWorkflowSteps: WorkflowStep[] = [
    {
      id: 'import-preview',
      title: t('reviewPlan'),
      status: importPreviewReady ? 'complete' : 'pending',
      summary: takeoutInspection
        ? t('previewBatchReady')
        : t('noTakeoutInspection'),
      reason: t('previewBeforeImport'),
      files: takeoutInspection?.recognizedFiles.map((file) => file.path) ?? [],
      actions: (
        <button
          className="secondaryButton"
          type="button"
          onClick={() => handleTakeout(true)}
        >
          {t('dryRun')}
        </button>
      ),
    },
    {
      id: 'import-inspect',
      title: t('sourcePreview'),
      status: importPreviewReady ? 'complete' : 'pending',
      summary: takeoutInspection
        ? `${takeoutInspection.recognizedFiles.length} ${t('recognizedFiles')}`
        : t('noTakeoutInspection'),
      reason: t('previewBeforeImport'),
      files: [
        ...(takeoutInspection?.recognizedFiles.map((file) => file.path) ?? []),
        ...(takeoutInspection?.quarantinedFiles.map((file) => file.path) ?? []),
      ],
    },
    {
      id: 'import-manual',
      title: t('manualPathTitle'),
      status: workflowChecked('import-manual') ? 'complete' : 'pending',
      summary: t('manualImportSummary'),
      reason: t('manualImportReason'),
      commands: importManualCommands,
      checklist: [
        t('manualLocateStep'),
        t('manualInspectStep'),
        t('manualContinueStep'),
      ],
      actions: (
        <button
          className="ghostButton"
          type="button"
          onClick={() => toggleWorkflowCheck('import-manual')}
        >
          {workflowChecked('import-manual')
            ? t('stepCompleted')
            : t('markStepComplete')}
        </button>
      ),
    },
    {
      id: 'import-apply',
      title: t('applyChanges'),
      status: importApplied ? 'complete' : 'pending',
      summary: takeoutInspection?.importBatch
        ? t('takeoutImportNotice', { count: takeoutInspection.importedItems })
        : t('applyChangesSummary'),
      reason: t('applyImportReason'),
      actions: (
        <button
          className="primaryButton"
          type="button"
          onClick={() => handleTakeout(false)}
        >
          {t('importSupported')}
        </button>
      ),
    },
    {
      id: 'import-verify',
      title: t('finishStep'),
      status: workflowChecked('import-finish') ? 'complete' : 'pending',
      summary:
        importBatchDetail?.batch.auditPath ??
        takeoutInspection?.importBatch?.auditPath ??
        t('finishSummary'),
      reason: importVerified ? t('finishReason') : t('verifyImportReason'),
      files: [
        ...(importBatchDetail?.recognizedFiles.map((file) => file.path) ?? []),
        ...(importBatchDetail?.quarantinedFiles.map((file) => file.path) ?? []),
      ],
      actions: (
        <button
          className="ghostButton"
          type="button"
          onClick={() =>
            /* v8 ignore next -- workflow copy changes only after import verification state flips */
            toggleWorkflowCheck(
              importVerified ? 'import-finish' : 'import-verify',
            )
          }
        >
          {workflowChecked(importVerified ? 'import-finish' : 'import-verify')
            ? t('stepCompleted')
            : t('markStepComplete')}
        </button>
      ),
    },
  ]

  let mainContent: ReactNode
  let inspectorContent: ReactNode

  if (activeView === 'setup') {
    mainContent = (
      <>
        <Surface
          eyebrow={t('sourcesStep')}
          title={t('sourcesDescription')}
          icon="account_tree"
        >
          <div className="selectionSummaryBar">
            <InfoStat
              label={t('profilesDetected', {
                count: snapshot?.browserProfiles.length ?? 0,
              })}
              value={String(snapshot?.browserProfiles.length ?? 0)}
            />
            <InfoStat
              label={t('selectedProfilesSummary')}
              value={String(selectedProfilesTotal)}
            />
          </div>
          <div className="supportedBrowserBlock">
            <span className="profileMetaLabel">
              {t('supportedBrowsersTitle')}
            </span>
            <div className="supportedBrowserStrip">
              {supportedBrowsers.map((browser) => (
                <div key={browser.name} className="supportedBrowserChip">
                  <BrowserIcon browserName={browser.name} decorative />
                  <span>{browser.name}</span>
                </div>
              ))}
            </div>
          </div>
          <div className="profileList">
            {snapshot?.browserProfiles.map((profile) => (
              <label
                key={profile.profileId}
                className={`profileRow ${isProfileSelected(profile.profileId) ? 'selected' : ''} ${!profile.historyExists ? 'disabled' : ''}`}
              >
                <input
                  checked={isProfileSelected(profile.profileId)}
                  className="profileCheckbox"
                  disabled={!profile.historyExists}
                  type="checkbox"
                  onChange={() => toggleProfile(profile.profileId)}
                />
                <span className="profileCheckboxVisual" aria-hidden="true">
                  <Glyph filled icon="check" />
                </span>
                <div className="profileCardBody">
                  <div className="profileHeaderLine">
                    <span className="profileBrowserMark" aria-hidden="true">
                      <BrowserIcon
                        browserName={profile.browserName}
                        decorative
                      />
                    </span>
                    <div className="profileIdentity">
                      <div className="profileNameStack">
                        <span className="profileName">
                          {profile.profileName}
                        </span>
                        <span className="browserPill">
                          {profile.browserName}
                        </span>
                      </div>
                      <span className="profileId">{profile.profileId}</span>
                    </div>
                  </div>
                  <div className="profileMetaGrid">
                    <div className="profileMetaItem">
                      <span className="profileMetaLabel">
                        {t('accountLabel')}
                      </span>
                      <span className="profileMetaValue">
                        {profile.userName ?? t('noSignedInUser')}
                      </span>
                    </div>
                    <div className="profileMetaItem">
                      <span className="profileMetaLabel">
                        {t('statusLabel')}
                      </span>
                      <span className="profileMetaValue">
                        {profile.historyExists
                          ? t('historyDetected')
                          : t('historyMissing')}
                      </span>
                    </div>
                    <div className="profileMetaItem">
                      <span className="profileMetaLabel">
                        {t('versionLabel')}
                      </span>
                      <span className="profileMetaValue">
                        {profile.browserVersion ?? t('unknownBrowserVersion')}
                      </span>
                    </div>
                  </div>
                  <div className="profileSourceLine">
                    <span className="profileMetaLabel">
                      {t('sourcePathLabel')}
                    </span>
                    <p className="profilePathText">
                      {profile.historyPath ?? profile.profilePath}
                    </p>
                  </div>
                </div>
              </label>
            ))}
          </div>
        </Surface>

        <Surface
          eyebrow={t('archiveStep')}
          title={t('archiveDescription')}
          icon="database"
        >
          <div className="fieldGrid two">
            <div className="fieldBlock">
              <label className="fieldLabel">{t('archiveMode')}</label>
              <div className="segmented">
                {(['Encrypted', 'Plaintext'] satisfies ArchiveMode[]).map(
                  (mode) => (
                    <button
                      key={mode}
                      className={
                        draftConfig?.archiveMode === mode ? 'selected' : ''
                      }
                      disabled={initialized}
                      type="button"
                      onClick={() => updateConfig({ archiveMode: mode })}
                    >
                      {mode === 'Encrypted' ? t('encrypted') : t('plaintext')}
                    </button>
                  ),
                )}
              </div>
              {initialized ? (
                <p className="fieldHint">{t('useSettingsForEncryption')}</p>
              ) : null}
            </div>

            <FieldBlock
              label={t('dueAfterHours')}
              control={
                <input
                  min={24}
                  step={24}
                  type="number"
                  value={draftConfig?.dueAfterHours ?? 72}
                  onChange={(event) =>
                    updateConfig({ dueAfterHours: Number(event.target.value) })
                  }
                />
              }
            />
            <FieldBlock
              label={t('checkIntervalHours')}
              control={
                <input
                  min={1}
                  step={1}
                  type="number"
                  value={draftConfig?.scheduleCheckIntervalHours ?? 6}
                  onChange={(event) =>
                    updateConfig({
                      scheduleCheckIntervalHours: Number(event.target.value),
                    })
                  }
                />
              }
            />
          </div>

          <div className="toggleList">
            <ToggleRow
              checked={draftConfig?.captureFavicons ?? true}
              label={t('captureFavicons')}
              onChange={(checked) => updateConfig({ captureFavicons: checked })}
            />
            <ToggleRow
              checked={draftConfig?.gitEnabled ?? true}
              label={t('gitAudit')}
              onChange={(checked) => updateConfig({ gitEnabled: checked })}
            />
            <ToggleRow
              checked={rememberKey}
              label={t('rememberKey')}
              onChange={setRememberKey}
            />
          </div>

          {!initialized && draftConfig?.archiveMode === 'Encrypted' ? (
            <div className="fieldGrid two">
              <FieldBlock
                label={t('masterPassword')}
                control={
                  <input
                    placeholder={t('passwordPlaceholder')}
                    type="password"
                    value={masterPassword}
                    onChange={(event) => setMasterPassword(event.target.value)}
                  />
                }
              />
              <FieldBlock
                label={t('confirmPassword')}
                control={
                  <input
                    placeholder={t('passwordPlaceholder')}
                    type="password"
                    value={confirmPassword}
                    onChange={(event) => setConfirmPassword(event.target.value)}
                  />
                }
              />
            </div>
          ) : null}
        </Surface>

        <Surface
          eyebrow={t('scheduleStep')}
          title={t('scheduleDescription')}
          icon="schedule"
          actions={
            <>
              <button
                className="secondaryButton"
                type="button"
                onClick={handlePreviewSchedule}
              >
                {t('previewSchedule')}
              </button>
              <button
                className="primaryButton"
                type="button"
                onClick={handleApplySchedule}
              >
                {t('applyPreview')}
              </button>
            </>
          }
        >
          <div className="fieldGrid compactRow">
            <FieldBlock
              label={t('schedulePlatform')}
              control={
                <select
                  value={schedulePlatform}
                  onChange={(event) =>
                    setSchedulePlatform(event.target.value as PlatformId)
                  }
                >
                  <option value="macos">macOS launchd</option>
                  <option value="windows">Windows Task Scheduler</option>
                  <option value="linux">Linux systemd user timer</option>
                </select>
              }
            />
          </div>

          {schedulePlan ? (
            <OperationWorkflow
              actionLabel={t('automaticPath')}
              labels={{
                why: t('whyThisStepMatters'),
                files: t('dataFilesRead'),
                commands: t('previewCommand'),
                checklist: t('manualSteps'),
                copy: t('previewCommand'),
                current: t('currentStep'),
                complete: t('stepCompleted'),
                pending: t('pending'),
              }}
              language={resolvedLanguage}
              onCopy={copyText}
              steps={scheduleWorkflowSteps}
            />
          ) : (
            <EmptyState>{t('noSchedulePreview')}</EmptyState>
          )}
        </Surface>
      </>
    )

    inspectorContent = (
      <>
        <Surface
          eyebrow={t('reviewStep')}
          title={t('reviewDescription')}
          icon="warning"
        >
          <div className="subsection">
            <h3>{t('whyThisStepMatters')}</h3>
            <p>{t('profileSelectionReason')}</p>
          </div>
          <div className="subsection">
            <h3>{t('dataFilesRead')}</h3>
            <div className="artifactList">
              {sourceFilePreview.length ? (
                sourceFilePreview.map((path) => (
                  <article className="artifactCard compactCard" key={path}>
                    <strong>{path}</strong>
                  </article>
                ))
              ) : (
                <EmptyState>{t('noFilesSelectedYet')}</EmptyState>
              )}
            </div>
          </div>
          <div className="subsection">
            <h3>{t('manualAlternative')}</h3>
            <div className="generatedList">
              {sourceManualCommands.length ? (
                sourceManualCommands.map((command) => (
                  <article className="codeArtifact" key={command}>
                    <div className="artifactHeader">
                      <strong>{t('previewCommand')}</strong>
                      <button
                        className="ghostButton"
                        type="button"
                        onClick={() => void copyText(command)}
                      >
                        {t('previewCommand')}
                      </button>
                    </div>
                    <pre>{command}</pre>
                  </article>
                ))
              ) : (
                <EmptyState>{t('noFilesSelectedYet')}</EmptyState>
              )}
            </div>
          </div>
          <dl className="dataList">
            <DataRow label={t('storagePath')} value={directories.appRoot} />
            <DataRow
              label={t('archiveDatabase')}
              value={directories.archiveDatabasePath}
            />
            <DataRow
              label={t('auditRepository')}
              value={directories.auditRepoPath}
            />
          </dl>
          {draftConfig?.archiveMode === 'Encrypted' ? (
            <div className="warningPanel">
              <div className="inlineHeading">
                <Glyph icon="warning" />
                <strong>{t('encryptionWarningTitle')}</strong>
              </div>
              <p>{t('encryptionWarningBody')}</p>
            </div>
          ) : null}
          <div className="reviewActions">
            <button
              className="primaryButton fullWidth"
              type="button"
              onClick={initialized ? handleSaveSetup : handleInitialize}
            >
              {initialized ? t('saveSetup') : t('createArchive')}
            </button>
            {initialized ? (
              <button
                className="secondaryButton fullWidth"
                type="button"
                onClick={handleBackupRun}
              >
                {t('runBackupNow')}
              </button>
            ) : null}
          </div>
        </Surface>
      </>
    )
  } else if (activeView === 'explorer') {
    mainContent = (
      <>
        <Surface
          eyebrow={t('explorerTitle')}
          title={t('explorerDescription')}
          icon="search"
        >
          <div className="queryBar">
            <FieldBlock
              label={t('searchLabel')}
              control={
                <input
                  placeholder={t('searchPlaceholder')}
                  value={historySearchInput}
                  onChange={(event) =>
                    setHistorySearchInput(event.target.value)
                  }
                />
              }
            />
            <FieldBlock
              label={t('domainLabel')}
              control={
                <input
                  placeholder={t('domainPlaceholder')}
                  value={historyDomain}
                  onChange={(event) => setHistoryDomain(event.target.value)}
                />
              }
            />
            <FieldBlock
              label={t('profileLabel')}
              control={
                <select
                  value={historyProfile}
                  onChange={(event) => setHistoryProfile(event.target.value)}
                >
                  <option value="">{t('allProfiles')}</option>
                  {snapshot?.browserProfiles.map((profile) => (
                    <option key={profile.profileId} value={profile.profileId}>
                      {profile.profileName}
                    </option>
                  ))}
                </select>
              }
            />
          </div>

          <div className="toolbarLine">
            <span className="surfaceMeta">
              {history
                ? t('resultsCount', { count: history.total })
                : t('resultsCount', { count: 0 })}
            </span>
            <div className="toolbarActions">
              <button
                className="ghostButton"
                type="button"
                onClick={() => handleExport('html')}
              >
                HTML
              </button>
              <button
                className="ghostButton"
                type="button"
                onClick={() => handleExport('markdown')}
              >
                Markdown
              </button>
              <button
                className="ghostButton"
                type="button"
                onClick={() => handleExport('text')}
              >
                Text
              </button>
              <button
                className="secondaryButton"
                type="button"
                onClick={() => handleExport('jsonl')}
              >
                JSONL
              </button>
            </div>
          </div>
        </Surface>

        <Surface
          eyebrow={t('resultsCount', { count: history?.total ?? 0 })}
          title=""
          icon="search"
        >
          {history?.items.length ? (
            <div className="recordList">
              {history.items.map((item) => (
                <button
                  key={item.id}
                  className={`recordRow ${selectedHistory?.id === item.id ? 'selected' : ''}`}
                  type="button"
                  onClick={() => setSelectedHistoryId(item.id)}
                >
                  <div className="recordMeta">
                    <span>
                      {formatDateTime(item.visitedAt, resolvedLanguage)}
                    </span>
                    <span>{item.profileId}</span>
                    <span>{item.domain}</span>
                  </div>
                  <strong>{item.title || item.url}</strong>
                  <p>{item.url}</p>
                </button>
              ))}
            </div>
          ) : (
            <EmptyState>
              {unlocked ? t('noHistoryResults') : t('unlockToSearch')}
            </EmptyState>
          )}
        </Surface>
      </>
    )

    inspectorContent = (
      <Surface
        eyebrow={t('selectedVisit')}
        title={selectedHistoryInspectorTitle}
        icon="database"
      >
        {selectedHistory ? (
          <dl className="dataList">
            <DataRow
              label={t('visitedAt')}
              value={formatDateTime(
                selectedHistory.visitedAt,
                resolvedLanguage,
              )}
            />
            <DataRow label={t('titleLabel')} value={selectedHistoryTitle} />
            <DataRow label={t('urlLabel')} value={selectedHistory.url} />
            <DataRow label={t('domain')} value={selectedHistory.domain} />
            <DataRow label={t('profile')} value={selectedHistory.profileId} />
            <DataRow
              label={t('duration')}
              value={formatDuration(selectedHistory.durationMs)}
            />
            <DataRow
              label={t('transition')}
              value={selectedHistoryTransition}
            />
            <DataRow
              label={t('sourceVisitId')}
              value={String(selectedHistory.sourceVisitId)}
            />
            <DataRow
              label={t('appId')}
              value={selectedHistory.appId ?? t('notAvailable')}
            />
          </dl>
        ) : (
          <EmptyState>{t('unlockToSearch')}</EmptyState>
        )}
      </Surface>
    )
  } else if (activeView === 'analysis') {
    mainContent = (
      <>
        <Surface
          eyebrow="Insights"
          title="Attention and research insights"
          icon="insights"
          actions={
            <>
              <button
                className="secondaryButton"
                type="button"
                onClick={() => void handleRunInsights(false)}
              >
                Refresh insights
              </button>
              <button
                className="ghostButton"
                type="button"
                onClick={() => void handleRunInsights(true)}
              >
                Rebuild insights
              </button>
            </>
          }
        >
          <div className="infoGrid four">
            <InfoStat
              label="Insight status"
              value={insightStatus.ready ? 'Ready' : 'Needs a run'}
            />
            <InfoStat label="Cards" value={String(insightStatus.cards)} />
            <InfoStat label="Threads" value={String(insightStatus.threads)} />
            <InfoStat label="Content coverage" value={insightCoverage} />
          </div>

          <div className="segmented">
            {[7, 30, 90].map((windowDays) => (
              <button
                key={windowDays}
                className={insightWindowDays === windowDays ? 'selected' : ''}
                type="button"
                onClick={() => setInsightWindowDays(windowDays)}
              >
                {windowDays}d
              </button>
            ))}
          </div>

          <div className="selectionSummaryBar">
            <InfoStat label="Window" value={`${insightWindowDays} days`} />
            <InfoStat label="Generated at" value={insightGeneratedAt} />
          </div>

          {insightStatus.warning ? (
            <div className="warningPanel subtleWarning">
              <div className="inlineHeading">
                <Glyph icon="warning" />
                <strong>Insight warning</strong>
              </div>
              <p>{insightStatus.warning}</p>
            </div>
          ) : null}

          {(insightRunReport?.notes.length ||
            insightSnapshot?.notes.length) && (
            <ul className="plainList">
              {(insightRunReport?.notes.length
                ? insightRunReport.notes
                : (insightSnapshot?.notes ?? [])
              ).map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          )}
        </Surface>

        <Surface
          eyebrow="Overview cards"
          title="What stands out right now"
          icon="auto_awesome"
        >
          {insightSnapshot?.cards.length ? (
            <div className="artifactList">
              {insightSnapshot.cards.map((card) => (
                <article className="artifactCard" key={card.cardId}>
                  <div className="artifactHeader">
                    <div>
                      <strong>{card.title}</strong>
                      <p>{card.summary}</p>
                    </div>
                    <StatusTag
                      tone={card.chromiumEnhanced ? 'info' : 'neutral'}
                    >
                      {card.chromiumEnhanced
                        ? 'Chromium-first'
                        : 'Archive-wide'}
                    </StatusTag>
                  </div>
                  <div className="toolbarLine">
                    <span className="surfaceMeta">
                      Score {card.score.toFixed(2)} · {card.windowDays} day
                      window
                    </span>
                    <button
                      className="ghostButton"
                      type="button"
                      onClick={() =>
                        void handleExplainInsight(
                          card.cardId,
                          'card',
                          card.title,
                        )
                      }
                    >
                      Why this?
                    </button>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <EmptyState>
              Run insights to generate trend cards, open loops, and resurfacing
              suggestions.
            </EmptyState>
          )}
        </Surface>

        <div className="splitBody">
          <Surface
            eyebrow="Topic timeline"
            title="Rising, cooling, and stable topics"
            icon="show_chart"
          >
            {insightSnapshot?.topics.length ? (
              <div className="trendList">
                {insightSnapshot.topics.map((topic) => {
                  const barWidth = `${Math.max(
                    10,
                    Math.min(100, topic.trendSlope * 100 + 35),
                  )}%`

                  return (
                    <article className="surfaceInset" key={topic.topicId}>
                      <div className="artifactHeader">
                        <div>
                          <strong>{topic.label}</strong>
                          <p>
                            {topic.visitCount} visits · {topic.revisitCount}{' '}
                            revisits
                          </p>
                        </div>
                        <button
                          className="ghostButton"
                          type="button"
                          onClick={() =>
                            void handleExplainInsight(
                              topic.topicId,
                              'topic',
                              topic.label,
                            )
                          }
                        >
                          Why this?
                        </button>
                      </div>
                      <div className="trendBarTrack">
                        <div
                          className="trendBarFill"
                          style={{ width: barWidth }}
                        />
                      </div>
                      <div className="recordMeta">
                        <span>Trend {topic.trendSlope.toFixed(2)}</span>
                        <span>Burst {topic.burstScore.toFixed(2)}</span>
                        <span>
                          Last seen{' '}
                          {formatDateTime(topic.lastSeenAt, resolvedLanguage)}
                        </span>
                      </div>
                    </article>
                  )
                })}
              </div>
            ) : (
              <EmptyState>
                No topic trends are available for this window yet.
              </EmptyState>
            )}
          </Surface>

          <Surface
            eyebrow="Active threads"
            title="Research threads and open loops"
            icon="account_tree"
          >
            {insightSnapshot?.threads.length ? (
              <div className="recordList">
                {insightSnapshot.threads.map((thread) => (
                  <article
                    className={`recordRow ${
                      selectedInsightThread?.threadId === thread.threadId
                        ? 'selected'
                        : ''
                    }`}
                    key={thread.threadId}
                  >
                    <div className="recordMeta">
                      <span>{thread.profileId}</span>
                      <span>{thread.status}</span>
                      <span>{thread.visitCount} visits</span>
                    </div>
                    <div className="artifactHeader">
                      <strong>{thread.title}</strong>
                      {thread.chromiumEnhanced ? (
                        <StatusTag tone="info">Chromium-first</StatusTag>
                      ) : null}
                    </div>
                    <p>
                      Open-loop score {thread.openLoopScore.toFixed(2)} ·
                      reopened {thread.reopenCount} times
                    </p>
                    <div className="toolbarActions">
                      <button
                        className="secondaryButton"
                        type="button"
                        onClick={() =>
                          setSelectedInsightThreadId(thread.threadId)
                        }
                      >
                        Inspect thread
                      </button>
                      <button
                        className="ghostButton"
                        type="button"
                        onClick={() =>
                          void handleExplainInsight(
                            thread.threadId,
                            'thread',
                            thread.title,
                          )
                        }
                      >
                        Why this?
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            ) : (
              <EmptyState>
                Run a Chromium-first insight pass to recover tasks and reopen
                patterns.
              </EmptyState>
            )}
          </Surface>
        </div>

        <div className="splitBody">
          <Surface
            eyebrow="Query ladders"
            title="How search terms got refined"
            icon="timeline"
          >
            {insightSnapshot?.queryLadders.length ? (
              <div className="artifactList">
                {insightSnapshot.queryLadders.map((ladder) => (
                  <article
                    className="artifactCard compactCard"
                    key={ladder.rootTerm}
                  >
                    <div className="artifactHeader">
                      <strong>{ladder.rootTerm}</strong>
                      {ladder.chromiumOnly ? (
                        <StatusTag tone="info">Chromium-only</StatusTag>
                      ) : null}
                    </div>
                    <p>{ladder.steps.join(' → ')}</p>
                    <div className="tokenList">
                      {ladder.stages.map((stage, index) => (
                        <span
                          className="tokenChip"
                          key={`${ladder.rootTerm}:${index}:${stage}`}
                        >
                          {stage}
                        </span>
                      ))}
                    </div>
                  </article>
                ))}
              </div>
            ) : (
              <EmptyState>
                Query reformulation ladders only appear for Chromium profiles
                with search term evidence.
              </EmptyState>
            )}
          </Surface>

          <Surface
            eyebrow="Workflow map"
            title="Source roles and transitions"
            icon="hub"
          >
            {insightSnapshot ? (
              <>
                <div className="tokenList">
                  {insightSnapshot.workflowMap.roles.map((role) => (
                    <span className="tokenChip" key={role.role}>
                      {role.role} · {role.count}
                    </span>
                  ))}
                </div>
                {insightSnapshot.workflowMap.edges.length ? (
                  <ul className="plainList workflowEdgeList">
                    {insightSnapshot.workflowMap.edges.map((edge) => (
                      <li key={`${edge.fromRole}:${edge.toRole}`}>
                        {edge.fromRole} → {edge.toRole} · {edge.count}
                      </li>
                    ))}
                  </ul>
                ) : (
                  <EmptyState>
                    No stable source-role transitions were found yet.
                  </EmptyState>
                )}
                {insightSnapshot.workflowMap.chromiumEnhanced ? (
                  <StatusTag tone="info">
                    Chromium-enhanced signals available
                  </StatusTag>
                ) : null}
              </>
            ) : (
              <EmptyState>
                Workflow evidence will appear here after the first insight run.
              </EmptyState>
            )}
          </Surface>
        </div>

        <Surface
          eyebrow={t('analysisSection')}
          title={t('analysisDescription')}
          icon="neurology"
          actions={
            <button
              className="primaryButton"
              type="button"
              onClick={handleSaveSettings}
            >
              {t('saveAiSettings')}
            </button>
          }
        >
          <div className="toggleList">
            <ToggleRow
              checked={aiConfig.enabled}
              label={t('enableAiAnalysis')}
              onChange={(checked) => updateAiSettings({ enabled: checked })}
            />
            <ToggleRow
              checked={aiConfig.assistantEnabled}
              label={t('enableAssistant')}
              onChange={(checked) =>
                updateAiSettings({ assistantEnabled: checked })
              }
            />
            <ToggleRow
              checked={aiConfig.semanticIndexEnabled}
              label={t('enableSemanticIndex')}
              onChange={(checked) =>
                updateAiSettings({ semanticIndexEnabled: checked })
              }
            />
            <ToggleRow
              checked={aiConfig.autoIndexAfterBackup}
              label={t('autoIndexAfterBackupLabel')}
              onChange={(checked) =>
                updateAiSettings({ autoIndexAfterBackup: checked })
              }
            />
            <ToggleRow
              checked={aiConfig.mcpEnabled}
              label={t('enableMcp')}
              onChange={(checked) => updateAiSettings({ mcpEnabled: checked })}
            />
            <ToggleRow
              checked={aiConfig.skillEnabled}
              label={t('enableSkill')}
              onChange={(checked) =>
                updateAiSettings({ skillEnabled: checked })
              }
            />
          </div>

          <div className="fieldGrid two">
            <FieldBlock
              label={t('llmProviderLabel')}
              control={
                <select
                  value={aiConfig.llmProviderId ?? ''}
                  onChange={(event) =>
                    updateAiSettings({
                      llmProviderId: event.target.value || null,
                    })
                  }
                >
                  <option value="">{t('selectProvider')}</option>
                  {llmProviders.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                      {provider.name || provider.id}
                    </option>
                  ))}
                </select>
              }
            />
            <FieldBlock
              label={t('embeddingProviderLabel')}
              control={
                <select
                  value={aiConfig.embeddingProviderId ?? ''}
                  onChange={(event) =>
                    updateAiSettings({
                      embeddingProviderId: event.target.value || null,
                    })
                  }
                >
                  <option value="">{t('selectProvider')}</option>
                  {embeddingProviders.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                      {provider.name || provider.id}
                    </option>
                  ))}
                </select>
              }
            />
            <FieldBlock
              label={t('retrievalTopK')}
              control={
                <input
                  min={1}
                  max={25}
                  step={1}
                  type="number"
                  value={aiConfig.retrievalTopK}
                  onChange={(event) =>
                    updateAiSettings({
                      retrievalTopK: Number(event.target.value),
                    })
                  }
                />
              }
            />
            <FieldBlock
              label={t('analysisProfileFilter')}
              control={
                <select
                  value={aiSearchProfile}
                  onChange={(event) => setAiSearchProfile(event.target.value)}
                >
                  <option value="">{t('allProfiles')}</option>
                  {snapshot?.browserProfiles.map((profile) => (
                    <option key={profile.profileId} value={profile.profileId}>
                      {profile.profileName}
                    </option>
                  ))}
                </select>
              }
            />
          </div>

          <FieldBlock
            label={t('assistantSystemPrompt')}
            control={
              <textarea
                className="multilineInput"
                rows={5}
                value={aiConfig.assistantSystemPrompt}
                onChange={(event) =>
                  updateAiSettings({
                    assistantSystemPrompt: event.target.value,
                  })
                }
              />
            }
          />
        </Surface>

        <Surface
          eyebrow={t('providerLibrary')}
          title={t('providerLibraryDescription')}
          icon="model_training"
        >
          <div className="providerColumns">
            <AiProviderEditorList
              addLabel={t('addLlmProvider')}
              apiKeys={providerSecrets}
              providers={llmProviders}
              purpose="llm"
              title={t('llmProvidersTitle')}
              translations={{
                providerName: t('providerName'),
                providerId: t('providerId'),
                requestFormat: t('requestFormat'),
                baseUrl: t('baseUrl'),
                defaultModel: t('defaultModel'),
                modelCatalog: t('modelCatalog'),
                modelCatalogHint: t('modelCatalogHint'),
                enabled: t('providerEnabled'),
                temperature: t('temperature'),
                maxTokens: t('maxTokens'),
                dimensions: t('dimensions'),
                notes: t('notes'),
                apiKey: t('apiKey'),
                keyStored: t('providerKeyStoredState'),
                saveKey: t('saveProviderKey'),
                clearKey: t('clearProviderKey'),
                remove: t('removeProvider'),
              }}
              onAdd={() => addAiProvider('llm')}
              onApiKeyChange={(providerId, value) =>
                setProviderSecrets((current) => ({
                  ...current,
                  [providerId]: value,
                }))
              }
              onClearKey={(providerId) =>
                void handleClearProviderSecret(providerId)
              }
              onRemove={(providerId) => removeAiProvider('llm', providerId)}
              onSaveKey={(providerId) =>
                void handleStoreProviderSecret(providerId)
              }
              onSelect={(providerId) =>
                updateAiSettings({ llmProviderId: providerId })
              }
              onUpdate={(providerId, patch) =>
                updateAiProvider('llm', providerId, patch)
              }
              selectedProviderId={aiConfig.llmProviderId ?? null}
            />
            <AiProviderEditorList
              addLabel={t('addEmbeddingProvider')}
              apiKeys={providerSecrets}
              providers={embeddingProviders}
              purpose="embedding"
              title={t('embeddingProvidersTitle')}
              translations={{
                providerName: t('providerName'),
                providerId: t('providerId'),
                requestFormat: t('requestFormat'),
                baseUrl: t('baseUrl'),
                defaultModel: t('defaultModel'),
                modelCatalog: t('modelCatalog'),
                modelCatalogHint: t('modelCatalogHint'),
                enabled: t('providerEnabled'),
                temperature: t('temperature'),
                maxTokens: t('maxTokens'),
                dimensions: t('dimensions'),
                notes: t('notes'),
                apiKey: t('apiKey'),
                keyStored: t('providerKeyStoredState'),
                saveKey: t('saveProviderKey'),
                clearKey: t('clearProviderKey'),
                remove: t('removeProvider'),
              }}
              onAdd={() => addAiProvider('embedding')}
              onApiKeyChange={(providerId, value) =>
                setProviderSecrets((current) => ({
                  ...current,
                  [providerId]: value,
                }))
              }
              onClearKey={(providerId) =>
                void handleClearProviderSecret(providerId)
              }
              onRemove={(providerId) =>
                removeAiProvider('embedding', providerId)
              }
              onSaveKey={(providerId) =>
                void handleStoreProviderSecret(providerId)
              }
              onSelect={(providerId) =>
                updateAiSettings({ embeddingProviderId: providerId })
              }
              onUpdate={(providerId, patch) =>
                updateAiProvider('embedding', providerId, patch)
              }
              selectedProviderId={aiConfig.embeddingProviderId ?? null}
            />
          </div>
        </Surface>

        <Surface
          eyebrow={t('analysisWorkbench')}
          title={t('analysisWorkbenchDescription')}
          icon="manage_search"
          actions={
            <>
              <button
                className="secondaryButton"
                type="button"
                onClick={() => void handleBuildAiIndex(false)}
              >
                {t('buildAiIndex')}
              </button>
              <button
                className="ghostButton"
                type="button"
                onClick={() => void handleBuildAiIndex(true)}
              >
                {t('rebuildAiIndex')}
              </button>
              <button
                className="ghostButton"
                type="button"
                onClick={handlePreviewAiIntegrations}
              >
                {t('previewIntegrations')}
              </button>
            </>
          }
        >
          <div className="infoGrid four">
            <InfoStat
              label={t('aiReady')}
              value={aiStatus.ready ? t('yes') : t('no')}
            />
            <InfoStat
              label={t('indexedItems')}
              value={String(aiStatus.indexedItems)}
            />
            <InfoStat
              label={t('selectedLlm')}
              value={aiConfig.llmProviderId ?? t('notAvailable')}
            />
            <InfoStat
              label={t('selectedEmbedding')}
              value={aiConfig.embeddingProviderId ?? t('notAvailable')}
            />
          </div>

          <div className="queryBar">
            <FieldBlock
              label={t('semanticSearchLabel')}
              control={
                <input
                  placeholder={t('semanticSearchPlaceholder')}
                  value={aiSearchInput}
                  onChange={(event) => setAiSearchInput(event.target.value)}
                />
              }
            />
            <FieldBlock
              label={t('domainLabel')}
              control={
                <input
                  placeholder={t('domainPlaceholder')}
                  value={aiSearchDomain}
                  onChange={(event) => setAiSearchDomain(event.target.value)}
                />
              }
            />
            <FieldBlock
              label={t('profileLabel')}
              control={
                <select
                  value={aiSearchProfile}
                  onChange={(event) => setAiSearchProfile(event.target.value)}
                >
                  <option value="">{t('allProfiles')}</option>
                  {snapshot?.browserProfiles.map((profile) => (
                    <option key={profile.profileId} value={profile.profileId}>
                      {profile.profileName}
                    </option>
                  ))}
                </select>
              }
            />
          </div>

          <div className="toolbarActions">
            <button
              className="secondaryButton"
              type="button"
              onClick={handleRunAiSearch}
            >
              {t('runSemanticSearch')}
            </button>
          </div>

          <FieldBlock
            label={t('assistantQuestion')}
            control={
              <textarea
                className="multilineInput"
                placeholder={t('assistantQuestionPlaceholder')}
                rows={4}
                value={aiQuestionInput}
                onChange={(event) => setAiQuestionInput(event.target.value)}
              />
            }
          />

          <div className="toolbarActions">
            <button
              className="primaryButton"
              type="button"
              onClick={handleAskAiAssistant}
            >
              {t('askAssistant')}
            </button>
          </div>
        </Surface>

        <Surface eyebrow={t('semanticResults')} title="" icon="search">
          {aiSearchResult?.items.length ? (
            <div className="recordList">
              {aiSearchResult.items.map((item) => (
                <article className="recordRow selected" key={item.historyId}>
                  <div className="recordMeta">
                    <span>
                      {formatDateTime(item.visitedAt, resolvedLanguage)}
                    </span>
                    <span>{item.profileId}</span>
                    <span>{item.matchReason}</span>
                  </div>
                  <strong>{item.title || item.url}</strong>
                  <p>{item.url}</p>
                </article>
              ))}
            </div>
          ) : (
            <EmptyState>{t('noSemanticResults')}</EmptyState>
          )}
          {aiSearchResult?.notes.length ? (
            <ul className="plainList">
              {aiSearchResult.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          ) : null}
        </Surface>
      </>
    )

    inspectorContent = (
      <>
        <Surface
          eyebrow="Profile facets"
          title="Workstyle snapshot"
          icon="person_search"
        >
          {insightSnapshot?.profileFacets.length ? (
            <div className="artifactList">
              {insightSnapshot.profileFacets.map((facet) => (
                <article className="artifactCard compactCard" key={facet.key}>
                  <div className="artifactHeader">
                    <strong>{facet.label}</strong>
                    <span className="surfaceMeta">
                      {Math.round(facet.confidence * 100)}% confidence
                    </span>
                  </div>
                  <p>{facet.value}</p>
                </article>
              ))}
            </div>
          ) : (
            <EmptyState>
              Faceted profile cards appear after a successful insight run.
            </EmptyState>
          )}
        </Surface>

        <Surface
          eyebrow="Selected thread"
          title={selectedInsightThread?.title ?? 'Choose a thread'}
          icon="visibility"
        >
          {insightThreadDetail ? (
            <>
              <dl className="dataList">
                <DataRow
                  label="Status"
                  value={insightThreadDetail.summary.status}
                />
                <DataRow
                  label="Open-loop score"
                  value={insightThreadDetail.summary.openLoopScore.toFixed(2)}
                />
                <DataRow
                  label="Reopens"
                  value={String(insightThreadDetail.summary.reopenCount)}
                />
                <DataRow
                  label="Dominant topic"
                  value={
                    insightThreadDetail.summary.dominantTopicId ??
                    t('notAvailable')
                  }
                />
              </dl>
              <div className="subsection">
                <h3>Evidence</h3>
                <div className="artifactList">
                  {insightThreadDetail.visits.map((visit) => (
                    <article
                      className="artifactCard compactCard"
                      key={`${visit.historyId}:${visit.visitedAt}`}
                    >
                      <strong>{visit.title || visit.url}</strong>
                      <p>{visit.url}</p>
                      <small>
                        {visit.profileId} ·{' '}
                        {formatDateTime(visit.visitedAt, resolvedLanguage)}
                      </small>
                    </article>
                  ))}
                </div>
              </div>
            </>
          ) : (
            <EmptyState>
              Pick a thread to inspect its evidence trail and reopen behavior.
            </EmptyState>
          )}
        </Surface>

        <Surface
          eyebrow="Why this"
          title={selectedInsightLabel ?? 'Choose an insight'}
          icon="help"
        >
          {insightExplanation ? (
            <>
              <p>{insightExplanation.explanation}</p>
              {insightExplanation.citations.length ? (
                <div className="artifactList">
                  {insightExplanation.citations.map((citation) => (
                    <article
                      className="artifactCard compactCard"
                      key={`${citation.historyId}:${citation.url}`}
                    >
                      <strong>{citation.title || citation.url}</strong>
                      <p>{citation.url}</p>
                      <small>
                        {citation.profileId} ·{' '}
                        {formatDateTime(citation.visitedAt, resolvedLanguage)}
                      </small>
                    </article>
                  ))}
                </div>
              ) : null}
              {insightExplanation.notes.length ? (
                <ul className="plainList">
                  {insightExplanation.notes.map((note) => (
                    <li key={note}>{note}</li>
                  ))}
                </ul>
              ) : null}
            </>
          ) : (
            <EmptyState>
              Use a card, topic, or thread’s “Why this?” action to inspect its
              evidence.
            </EmptyState>
          )}
        </Surface>

        <Surface eyebrow={t('assistantSection')} title="" icon="smart_toy">
          {aiAssistantResult ? (
            <>
              <div className="subsection">
                <h3>{t('assistantAnswer')}</h3>
                <p>{aiAssistantResult.answer}</p>
              </div>
              {aiAssistantResult.citations.length ? (
                <div className="subsection">
                  <h3>{t('assistantCitations')}</h3>
                  <div className="artifactList">
                    {aiAssistantResult.citations.map((citation) => (
                      <article
                        className="artifactCard compactCard"
                        key={`${citation.historyId}:${citation.url}`}
                      >
                        <strong>{citation.title || citation.url}</strong>
                        <p>{citation.url}</p>
                        <small>
                          {citation.profileId} ·{' '}
                          {formatDateTime(citation.visitedAt, resolvedLanguage)}
                        </small>
                      </article>
                    ))}
                  </div>
                </div>
              ) : null}
              {aiAssistantResult.notes.length ? (
                <ul className="plainList">
                  {aiAssistantResult.notes.map((note) => (
                    <li key={note}>{note}</li>
                  ))}
                </ul>
              ) : null}
            </>
          ) : (
            <EmptyState>{t('assistantEmptyState')}</EmptyState>
          )}
        </Surface>

        <Surface
          eyebrow={t('manualPathTitle')}
          title={t('integrationStepsTitle')}
          icon="route"
        >
          <dl className="dataList">
            <DataRow
              label={t('lastIndexedAt')}
              value={
                formatDateTime(aiStatus.lastIndexedAt, resolvedLanguage) ??
                t('notAvailable')
              }
            />
            <DataRow
              label={t('lastIndexReport')}
              value={
                aiIndexReport
                  ? `${aiIndexReport.indexedItems + aiIndexReport.updatedItems}`
                  : t('notAvailable')
              }
            />
            <DataRow
              label={t('warningLabel')}
              value={aiStatus.warning ?? t('notAvailable')}
            />
          </dl>
          {aiIntegrationPreview ? (
            <>
              <div className="codeArtifact compact">
                <div className="artifactHeader">
                  <strong>{t('mcpCommand')}</strong>
                  <button
                    className="ghostButton"
                    type="button"
                    onClick={() =>
                      void copyText(aiIntegrationPreview.mcpCommand)
                    }
                  >
                    {t('previewCommand')}
                  </button>
                </div>
                <pre>{aiIntegrationPreview.mcpCommand}</pre>
              </div>
              {aiIntegrationPreview.generatedFiles.length ? (
                <div className="generatedList">
                  {aiIntegrationPreview.generatedFiles.map((file) => (
                    <article className="codeArtifact" key={file.relativePath}>
                      <div className="artifactHeader">
                        <strong>{file.relativePath}</strong>
                        <button
                          className="ghostButton"
                          type="button"
                          onClick={() => void copyText(file.contents)}
                        >
                          {t('previewCommand')}
                        </button>
                      </div>
                      <pre>{file.contents}</pre>
                    </article>
                  ))}
                </div>
              ) : null}
              {aiIntegrationPreview.manualSteps.length ? (
                <ol className="stepList">
                  {aiIntegrationPreview.manualSteps.map((step) => (
                    <li key={step}>{step}</li>
                  ))}
                </ol>
              ) : null}
              {aiIntegrationPreview.warnings.length ? (
                <ul className="plainList">
                  {aiIntegrationPreview.warnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              ) : null}
            </>
          ) : (
            <EmptyState>{t('integrationPreviewEmpty')}</EmptyState>
          )}
        </Surface>
      </>
    )
  } else if (activeView === 'backups') {
    mainContent = (
      <>
        <Surface
          eyebrow={t('backupsTitle')}
          title={t('backupsDescription')}
          icon="fact_check"
          actions={
            <button
              className="primaryButton"
              type="button"
              onClick={handleBackupRun}
            >
              <Glyph icon="play_arrow" />
              {t('runBackupNow')}
            </button>
          }
        >
          {snapshot?.recentRuns.length ? (
            <div className="recordList">
              {snapshot.recentRuns.map((run) => (
                <button
                  key={run.id}
                  className={`recordRow ${selectedRun?.id === run.id ? 'selected' : ''}`}
                  type="button"
                  onClick={() => setSelectedRunId(run.id)}
                >
                  <div className="recordMeta">
                    <span>
                      {t('status')}: {run.status}
                    </span>
                    <span>
                      {t('processedProfiles')}: {run.profilesProcessed}
                    </span>
                    <span>
                      {t('newVisits')}: {run.newVisits}
                    </span>
                  </div>
                  <strong>
                    #{run.id} · {run.manifestHash ?? t('pending')}
                  </strong>
                  <p>{formatDateTime(run.startedAt, resolvedLanguage)}</p>
                </button>
              ))}
            </div>
          ) : (
            <EmptyState>{t('noRuns')}</EmptyState>
          )}
        </Surface>

        <Surface
          eyebrow={t('remoteBackup')}
          title={t('remoteBackupDescription')}
          icon="cloud_upload"
        >
          <div className="infoGrid">
            <InfoStat
              label={t('lastUploadAt')}
              value={remoteBackupLastUploaded}
            />
            <InfoStat
              label={t('objectKey')}
              value={
                remoteBackupConfig.lastUploadedObjectKey ?? t('notAvailable')
              }
            />
            <InfoStat
              label={t('lastError')}
              value={remoteBackupConfig.lastError ?? t('notAvailable')}
            />
          </div>
          <div className="toolbarActions">
            <button
              className="secondaryButton"
              type="button"
              onClick={handlePreviewRemoteBackup}
            >
              {t('previewUpload')}
            </button>
            <button
              className="primaryButton"
              type="button"
              onClick={handleRunRemoteBackup}
            >
              {t('uploadNow')}
            </button>
          </div>
        </Surface>
      </>
    )

    inspectorContent = (
      <>
        <Surface
          eyebrow={t('runDetails')}
          title={selectedRun ? `#${selectedRun.id}` : t('notAvailable')}
          icon="database"
        >
          {selectedRun ? (
            <dl className="dataList">
              <DataRow label={t('status')} value={selectedRun.status} />
              <DataRow
                label={t('startedAt')}
                value={formatDateTime(selectedRun.startedAt, resolvedLanguage)}
              />
              <DataRow
                label={t('finishedAt')}
                value={
                  formatDateTime(
                    selectedRun.finishedAt ?? null,
                    resolvedLanguage,
                  ) ?? t('stillRunning')
                }
              />
              <DataRow
                label={t('manifestHash')}
                value={selectedRun.manifestHash ?? t('pending')}
              />
              <DataRow
                label={t('processedProfiles')}
                value={String(selectedRun.profilesProcessed)}
              />
              <DataRow
                label={t('newVisits')}
                value={String(selectedRun.newVisits)}
              />
              <DataRow
                label={t('newUrls')}
                value={String(selectedRun.newUrls)}
              />
              <DataRow
                label={t('newDownloads')}
                value={String(selectedRun.newDownloads)}
              />
            </dl>
          ) : (
            <EmptyState>{t('noRuns')}</EmptyState>
          )}
        </Surface>

        <Surface
          eyebrow={t('latestAction')}
          title={lastBackupReport?.manifestPath ?? t('notAvailable')}
          icon="construction"
        >
          {lastBackupReport ? (
            <>
              <dl className="dataList">
                <DataRow
                  label={t('manifestPath')}
                  value={lastBackupReport.manifestPath ?? t('notAvailable')}
                />
                <DataRow
                  label={t('remoteBackup')}
                  value={
                    lastBackupReport.remoteBackup?.message ?? t('notAvailable')
                  }
                />
              </dl>
              {lastBackupReport.warnings.length ? (
                <div className="warningPanel">
                  <div className="inlineHeading">
                    <Glyph icon="warning" />
                    <strong>{t('warnings')}</strong>
                  </div>
                  <ul className="plainList">
                    {lastBackupReport.warnings.map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                </div>
              ) : null}
            </>
          ) : (
            <EmptyState>{t('noRuns')}</EmptyState>
          )}
        </Surface>
      </>
    )
  } else if (activeView === 'import') {
    mainContent = (
      <>
        <Surface
          eyebrow={t('importTitle')}
          title={t('importDescription')}
          icon="upload_file"
        >
          <div className="fieldGrid">
            <FieldBlock
              label={t('takeoutPath')}
              control={
                <input
                  placeholder={t('takeoutPlaceholder')}
                  value={takeoutPath}
                  onChange={(event) => setTakeoutPath(event.target.value)}
                />
              }
            />
          </div>
          <p className="fieldHint">{t('importFlowHint')}</p>
          <div className="toolbarActions">
            <button
              className="secondaryButton"
              type="button"
              onClick={() => handleTakeout(true)}
            >
              {t('dryRun')}
            </button>
            <button
              className="primaryButton"
              type="button"
              onClick={() => handleTakeout(false)}
            >
              {t('importSupported')}
            </button>
          </div>
          {importSummary ? (
            <div className="infoGrid four">
              <InfoStat
                label={t('candidateItems')}
                value={String(importSummary.candidateItems)}
              />
              <InfoStat
                label={t('importedItems')}
                value={String(importSummary.importedItems)}
              />
              <InfoStat
                label={t('duplicateItems')}
                value={String(importSummary.duplicateItems)}
              />
              <InfoStat
                label={t('visibleItems')}
                value={String(importSummary.visibleItems)}
              />
            </div>
          ) : null}
        </Surface>

        <Surface
          eyebrow={t('workflowGuide')}
          title={t('importWorkflowTitle')}
          icon="route"
        >
          <OperationWorkflow
            actionLabel={t('automaticPath')}
            labels={{
              why: t('whyThisStepMatters'),
              files: t('dataFilesRead'),
              commands: t('previewCommand'),
              checklist: t('manualSteps'),
              copy: t('previewCommand'),
              current: t('currentStep'),
              complete: t('stepCompleted'),
              pending: t('pending'),
            }}
            language={resolvedLanguage}
            onCopy={copyText}
            steps={importWorkflowSteps}
          />
        </Surface>

        {takeoutInspection ? (
          <Surface
            eyebrow={t('sourcePreview')}
            title={t('previewBeforeImport')}
            icon="database"
          >
            <div className="splitBody importWorkspace">
              <div className="surfaceInset">
                <div className="toolbarLine">
                  <h3>{t('recognizedFiles')}</h3>
                  <span className="surfaceMeta">
                    {takeoutInspection.recognizedFiles.length}
                  </span>
                </div>
                {takeoutInspection.recognizedFiles.length ? (
                  <div className="artifactList">
                    {takeoutInspection.recognizedFiles.map((file) => (
                      <article
                        className="artifactCard compactCard"
                        key={file.path}
                      >
                        <div className="artifactHeader">
                          <div>
                            <strong>{file.path}</strong>
                            <p>{file.kind}</p>
                          </div>
                          <StatusTag tone="info">{file.status}</StatusTag>
                        </div>
                        <small>{file.records}</small>
                      </article>
                    ))}
                  </div>
                ) : (
                  <EmptyState>{t('noRecognizedFiles')}</EmptyState>
                )}

                <div className="subsection">
                  <div className="toolbarLine">
                    <h3>{t('quarantinedFiles')}</h3>
                    <span className="surfaceMeta">
                      {takeoutInspection.quarantinedFiles.length}
                    </span>
                  </div>
                  {takeoutInspection.quarantinedFiles.length ? (
                    <div className="artifactList">
                      {takeoutInspection.quarantinedFiles.map((file) => (
                        <article
                          className="artifactCard compactCard"
                          key={file.path}
                        >
                          <div className="artifactHeader">
                            <div>
                              <strong>{file.path}</strong>
                              <p>{file.kind}</p>
                            </div>
                            <StatusTag tone="danger">{file.status}</StatusTag>
                          </div>
                        </article>
                      ))}
                    </div>
                  ) : (
                    <EmptyState>{t('noQuarantinedFiles')}</EmptyState>
                  )}
                </div>
              </div>

              <div className="surfaceInset">
                <div className="toolbarLine">
                  <h3>{t('previewRows')}</h3>
                  <span className="surfaceMeta">
                    {takeoutInspection.previewEntries.length}
                  </span>
                </div>
                {takeoutInspection.previewEntries.length ? (
                  <PreviewEntryList
                    entries={takeoutInspection.previewEntries}
                    language={resolvedLanguage}
                  />
                ) : (
                  <EmptyState>{t('noPreviewRows')}</EmptyState>
                )}
                {takeoutInspection.notes.length ? (
                  <div className="subsection">
                    <h3>{t('notes')}</h3>
                    <ul className="plainList">
                      {takeoutInspection.notes.map((note) => (
                        <li key={note}>{note}</li>
                      ))}
                    </ul>
                  </div>
                ) : null}
              </div>
            </div>
          </Surface>
        ) : (
          <Surface
            eyebrow={t('sourcePreview')}
            title={t('previewBeforeImport')}
            icon="database"
          >
            <EmptyState>{t('noTakeoutInspection')}</EmptyState>
          </Surface>
        )}

        <Surface
          eyebrow={t('recentImports')}
          title={t('importAuditTrail')}
          icon="history"
        >
          {snapshot?.recentImportBatches.length ? (
            <div className="recordList">
              {snapshot.recentImportBatches.map((batch) => (
                <button
                  key={batch.id}
                  className={`recordRow ${selectedImportBatch?.id === batch.id ? 'selected' : ''}`}
                  type="button"
                  onClick={() => void handlePreviewImportBatch(batch.id)}
                >
                  <div className="recordMeta">
                    <span>
                      {formatDateTime(batch.createdAt, resolvedLanguage)}
                    </span>
                    <span>{batch.sourceKind}</span>
                    <span>
                      {t('visibleItems')}: {batch.visibleItems}
                    </span>
                  </div>
                  <div className="batchHeader">
                    <strong>#{batch.id}</strong>
                    <StatusTag
                      tone={batch.status === 'reverted' ? 'danger' : 'info'}
                    >
                      {formatBatchStatus(batch.status)}
                    </StatusTag>
                  </div>
                  <p>{batch.sourcePath}</p>
                  <div className="batchMetaGrid">
                    <span>
                      {t('candidateItems')}: {batch.candidateItems}
                    </span>
                    <span>
                      {t('importedItems')}: {batch.importedItems}
                    </span>
                    <span>
                      {t('duplicateItems')}: {batch.duplicateItems}
                    </span>
                  </div>
                </button>
              ))}
            </div>
          ) : (
            <EmptyState>{t('noImportBatches')}</EmptyState>
          )}
        </Surface>
      </>
    )

    inspectorContent = (
      <>
        <Surface
          eyebrow={t('selectedBatch')}
          title={
            importBatchDetail
              ? `#${importBatchDetail.batch.id} · ${formatBatchStatus(importBatchDetail.batch.status)}`
              : t('importBatchDetail')
          }
          icon="history"
          actions={
            importBatchDetail &&
            importBatchDetail.batch.status !== 'reverted' ? (
              <button
                className="dangerButton"
                type="button"
                onClick={() =>
                  void handleRevertImportBatch(importBatchDetail.batch.id)
                }
              >
                {t('revertBatch')}
              </button>
            ) : undefined
          }
        >
          {importBatchDetail ? (
            <>
              <dl className="dataList">
                <DataRow
                  label={t('status')}
                  value={formatBatchStatus(importBatchDetail.batch.status)}
                />
                <DataRow
                  label={t('createdAt')}
                  value={formatDateTime(
                    importBatchDetail.batch.createdAt,
                    resolvedLanguage,
                  )}
                />
                <DataRow
                  label={t('importedAt')}
                  value={
                    formatDateTime(
                      importBatchDetail.batch.importedAt ?? null,
                      resolvedLanguage,
                    ) ?? t('notAvailable')
                  }
                />
                <DataRow
                  label={t('revertedAt')}
                  value={
                    formatDateTime(
                      importBatchDetail.batch.revertedAt ?? null,
                      resolvedLanguage,
                    ) ?? t('notAvailable')
                  }
                />
                <DataRow
                  label={t('candidateItems')}
                  value={String(importBatchDetail.batch.candidateItems)}
                />
                <DataRow
                  label={t('importedItems')}
                  value={String(importBatchDetail.batch.importedItems)}
                />
                <DataRow
                  label={t('duplicateItems')}
                  value={String(importBatchDetail.batch.duplicateItems)}
                />
                <DataRow
                  label={t('visibleItems')}
                  value={String(importBatchDetail.batch.visibleItems)}
                />
                <DataRow
                  label={t('takeoutPath')}
                  value={importBatchDetail.batch.sourcePath}
                />
                <DataRow
                  label={t('manifestPath')}
                  value={importBatchDetail.batch.auditPath ?? t('notAvailable')}
                />
              </dl>

              <div className="subsection">
                <div className="inlineHeading">
                  <Glyph icon="history_toggle_off" />
                  <strong>{t('previewRows')}</strong>
                </div>
                {importBatchDetail.previewEntries.length ? (
                  <PreviewEntryList
                    entries={importBatchDetail.previewEntries}
                    language={resolvedLanguage}
                  />
                ) : (
                  <EmptyState>{t('noPreviewRows')}</EmptyState>
                )}
              </div>

              <div className="warningPanel subtleWarning">
                <div className="inlineHeading">
                  <Glyph icon="history_toggle_off" />
                  <strong>{t('revertKeepsAuditTitle')}</strong>
                </div>
                <p>{t('revertKeepsAuditBody')}</p>
              </div>

              {importBatchDetail.notes.length ? (
                <div className="subsection">
                  <h3>{t('notes')}</h3>
                  <ul className="plainList">
                    {importBatchDetail.notes.map((note) => (
                      <li key={note}>{note}</li>
                    ))}
                  </ul>
                </div>
              ) : null}
            </>
          ) : (
            <EmptyState>{t('noImportBatchSelected')}</EmptyState>
          )}
        </Surface>

        <Surface
          eyebrow={t('doctorChecks')}
          title={t('doctorDescription')}
          icon="health_and_safety"
        >
          <div className="toolbarActions">
            <button
              className="secondaryButton"
              type="button"
              onClick={handleDoctor}
            >
              {t('runDoctor')}
            </button>
          </div>
          {doctorReport ? (
            <div className="checkList">
              {doctorReport.checks.map((check) => (
                <article
                  className={`checkRow ${check.status === 'ok' ? 'ok' : 'bad'}`}
                  key={check.name}
                >
                  <strong>{check.name}</strong>
                  <p>{check.message}</p>
                </article>
              ))}
            </div>
          ) : (
            <EmptyState>{t('doctorNotRun')}</EmptyState>
          )}
        </Surface>
      </>
    )
  } else {
    mainContent = (
      <>
        <Surface
          eyebrow={t('languageSection')}
          title={t('languageDescription')}
          icon="language"
        >
          <div className="fieldGrid two">
            <FieldBlock
              label={t('interfaceLanguage')}
              control={
                <select
                  value={currentConfig.preferredLanguage}
                  onChange={(event) =>
                    updateConfig({
                      preferredLanguage: event.target
                        .value as AppConfig['preferredLanguage'],
                    })
                  }
                >
                  <option value="system">{t('followSystem')}</option>
                  <option value="en">{t('english')}</option>
                  <option value="zh-CN">{t('simplifiedChinese')}</option>
                  <option value="zh-TW">{t('traditionalChinese')}</option>
                </select>
              }
            />
            <FieldBlock
              label={t('resolvedLanguage')}
              control={
                <div className="readOnlyField">
                  {languageLabel(resolvedLanguage, resolvedLanguage)}
                </div>
              }
            />
          </div>
        </Surface>

        <Surface
          eyebrow={t('dataSection')}
          title={t('dataDescription')}
          icon="folder_open"
        >
          <p className="surfaceMeta">{t('dataDirectoryHint')}</p>
          <div className="pathList">
            <PathRow
              actions={
                <>
                  <button
                    aria-label={t('openPathLabel', {
                      label: t('storagePath'),
                    })}
                    className="secondaryButton"
                    type="button"
                    onClick={() => void handleOpenPath(directories.appRoot)}
                  >
                    {t('openAction')}
                  </button>
                  <button
                    aria-label={t('copyPathLabel', {
                      label: t('storagePath'),
                    })}
                    className="ghostButton"
                    type="button"
                    onClick={() => void copyText(directories.appRoot)}
                  >
                    {t('copyAction')}
                  </button>
                </>
              }
              label={t('storagePath')}
              value={directories.appRoot || t('notAvailable')}
            />
            <PathRow
              actions={
                <>
                  <button
                    aria-label={t('openPathLabel', {
                      label: t('archiveDatabase'),
                    })}
                    className="secondaryButton"
                    type="button"
                    onClick={() =>
                      void handleOpenPath(directories.archiveDatabasePath)
                    }
                  >
                    {t('openAction')}
                  </button>
                  <button
                    aria-label={t('copyPathLabel', {
                      label: t('archiveDatabase'),
                    })}
                    className="ghostButton"
                    type="button"
                    onClick={() =>
                      void copyText(directories.archiveDatabasePath)
                    }
                  >
                    {t('copyAction')}
                  </button>
                </>
              }
              label={t('archiveDatabase')}
              value={directories.archiveDatabasePath || t('notAvailable')}
            />
            <PathRow
              actions={
                <>
                  <button
                    aria-label={t('openPathLabel', {
                      label: t('auditRepository'),
                    })}
                    className="secondaryButton"
                    type="button"
                    onClick={() =>
                      void handleOpenPath(directories.auditRepoPath)
                    }
                  >
                    {t('openAction')}
                  </button>
                  <button
                    aria-label={t('copyPathLabel', {
                      label: t('auditRepository'),
                    })}
                    className="ghostButton"
                    type="button"
                    onClick={() => void copyText(directories.auditRepoPath)}
                  >
                    {t('copyAction')}
                  </button>
                </>
              }
              label={t('auditRepository')}
              value={directories.auditRepoPath || t('notAvailable')}
            />
          </div>
          <div className="subsection">
            <h3>{t('buildInfoTitle')}</h3>
            <div className="infoGrid">
              <InfoStat label={t('appVersion')} value={buildVersion} />
              <InfoStat label={t('gitCommit')} value={buildCommit} />
              <InfoStat label={t('buildState')} value={buildState} />
            </div>
          </div>
        </Surface>

        <Surface
          eyebrow={t('securitySection')}
          title={t('securityDescription')}
          icon="security"
        >
          {initialized ? (
            <>
              <dl className="dataList">
                <DataRow
                  label={t('archiveMode')}
                  value={
                    archiveStatus.encrypted ? t('encrypted') : t('plaintext')
                  }
                />
                <DataRow
                  label={t('status')}
                  value={archiveStatus.unlocked ? t('unlocked') : t('locked')}
                />
                <DataRow
                  label={t('keyringBackend')}
                  value={keyringStatus.backend || t('notAvailable')}
                />
                <DataRow
                  label={t('rememberedKey')}
                  value={
                    keyringStatus.storedSecret ? t('present') : t('absent')
                  }
                />
              </dl>

              {archiveStatus.encrypted ? (
                archiveStatus.unlocked ? (
                  <>
                    <div className="fieldGrid">
                      <FieldBlock
                        label={t('newMasterPassword')}
                        control={
                          <input
                            placeholder={t('passwordPlaceholder')}
                            type="password"
                            value={rekeyPassword}
                            onChange={(event) =>
                              setRekeyPassword(event.target.value)
                            }
                          />
                        }
                      />
                    </div>
                    <div className="toolbarActions">
                      <button
                        className="secondaryButton"
                        type="button"
                        onClick={handleRememberCurrentKey}
                      >
                        {t('storeRememberedKey')}
                      </button>
                      <button
                        className="secondaryButton"
                        type="button"
                        onClick={handleClearRememberedKey}
                      >
                        {t('clearRememberedKey')}
                      </button>
                    </div>
                    <div className="toolbarActions">
                      <button
                        className="primaryButton"
                        type="button"
                        onClick={handleRotateEncryption}
                      >
                        {t('rotateKey')}
                      </button>
                      <button
                        className="dangerButton"
                        type="button"
                        onClick={handleSwitchToPlaintext}
                      >
                        {t('convertToPlaintext')}
                      </button>
                    </div>
                  </>
                ) : (
                  <div className="fieldGrid">
                    <FieldBlock
                      label={t('masterPassword')}
                      control={
                        <input
                          placeholder={t('passwordPlaceholder')}
                          type="password"
                          value={unlockPassword}
                          onChange={(event) =>
                            setUnlockPassword(event.target.value)
                          }
                        />
                      }
                    />
                    <div className="toolbarActions">
                      <button
                        className="primaryButton"
                        type="button"
                        onClick={handleUnlockWithPassword}
                      >
                        {t('unlockArchive')}
                      </button>
                    </div>
                  </div>
                )
              ) : (
                <div className="warningPanel">
                  <div className="inlineHeading">
                    <Glyph icon="warning" />
                    <strong>{t('encryptionWarningTitle')}</strong>
                  </div>
                  <p>{t('encryptionWarningBody')}</p>
                </div>
              )}
            </>
          ) : (
            <EmptyState>{t('securityBeforeInit')}</EmptyState>
          )}
        </Surface>

        <Surface
          eyebrow={t('remoteSection')}
          title={t('remoteSectionDescription')}
          icon="cloud_upload"
        >
          <div className="toggleList">
            <ToggleRow
              checked={remoteBackupConfig.enabled}
              label={t('enableRemoteBackup')}
              onChange={(checked) => updateRemoteBackup({ enabled: checked })}
            />
            <ToggleRow
              checked={remoteBackupConfig.pathStyle}
              label={t('pathStyle')}
              onChange={(checked) => updateRemoteBackup({ pathStyle: checked })}
            />
            <ToggleRow
              checked={remoteBackupConfig.uploadAfterBackup}
              label={t('uploadAfterBackup')}
              onChange={(checked) =>
                updateRemoteBackup({ uploadAfterBackup: checked })
              }
            />
          </div>

          <div className="fieldGrid two">
            <FieldBlock
              label={t('bucket')}
              control={
                <input
                  value={remoteBackupConfig.bucket}
                  onChange={(event) =>
                    updateRemoteBackup({ bucket: event.target.value })
                  }
                />
              }
            />
            <FieldBlock
              label={t('region')}
              control={
                <input
                  value={remoteBackupConfig.region}
                  onChange={(event) =>
                    updateRemoteBackup({ region: event.target.value })
                  }
                />
              }
            />
            <FieldBlock
              label={t('prefix')}
              control={
                <input
                  value={remoteBackupConfig.prefix}
                  onChange={(event) =>
                    updateRemoteBackup({ prefix: event.target.value })
                  }
                />
              }
            />
            <FieldBlock
              label={t('endpoint')}
              control={
                <input
                  placeholder="https://s3.example.com"
                  value={remoteBackupConfig.endpoint ?? ''}
                  onChange={(event) =>
                    updateRemoteBackup({
                      endpoint: event.target.value ? event.target.value : null,
                    })
                  }
                />
              }
            />
          </div>

          <p className="fieldHint">{t('endpointHint')}</p>

          <div className="fieldGrid two">
            <FieldBlock
              label={t('accessKeyId')}
              control={
                <input
                  autoComplete="off"
                  value={s3AccessKeyId}
                  onChange={(event) => setS3AccessKeyId(event.target.value)}
                />
              }
            />
            <FieldBlock
              label={t('secretAccessKey')}
              control={
                <input
                  autoComplete="off"
                  type="password"
                  value={s3SecretAccessKey}
                  onChange={(event) => setS3SecretAccessKey(event.target.value)}
                />
              }
            />
          </div>

          <div className="toolbarActions">
            <button
              className="secondaryButton"
              type="button"
              onClick={handleStoreS3Credentials}
            >
              {t('saveCredentials')}
            </button>
            <button
              className="ghostButton"
              type="button"
              onClick={handleClearS3Credentials}
            >
              {t('clearCredentials')}
            </button>
            <button
              className="ghostButton"
              type="button"
              onClick={handlePreviewRemoteBackup}
            >
              {t('previewUpload')}
            </button>
            <button
              className="primaryButton"
              type="button"
              onClick={handleRunRemoteBackup}
            >
              {t('uploadNow')}
            </button>
          </div>
        </Surface>
      </>
    )

    inspectorContent = (
      <>
        <Surface eyebrow={t('currentState')} title="" icon="security">
          <dl className="dataList">
            <DataRow
              label={t('credentialsSaved')}
              value={remoteBackupConfig.credentialsSaved ? t('yes') : t('no')}
            />
            <DataRow
              label={t('lastUploadAt')}
              value={remoteBackupInspectorLastUploaded}
            />
            <DataRow
              label={t('objectKey')}
              value={
                remoteBackupConfig.lastUploadedObjectKey ?? t('notAvailable')
              }
            />
            <DataRow
              label={t('lastError')}
              value={remoteBackupConfig.lastError ?? t('notAvailable')}
            />
          </dl>
          <div className="toolbarActions">
            <ToggleRow
              checked={currentConfig.appAutostart}
              label={t('appAutostart')}
              onChange={(checked) => updateConfig({ appAutostart: checked })}
            />
          </div>
          <button
            className="primaryButton fullWidth"
            type="button"
            onClick={handleSaveSettings}
          >
            {t('saveSettings')}
          </button>
        </Surface>

        <Surface eyebrow={t('previewCommand')} title="" icon="cloud_upload">
          {remotePreview ? (
            <>
              <dl className="dataList">
                <DataRow
                  label={t('objectKey')}
                  value={remotePreview.objectKey}
                />
                <DataRow
                  label={t('urlLabel')}
                  value={remotePreview.uploadUrl}
                />
              </dl>
              <div className="codeArtifact compact">
                <div className="artifactHeader">
                  <strong>{t('previewCommand')}</strong>
                  <button
                    className="ghostButton"
                    type="button"
                    onClick={() => void copyText(remotePreview.previewCommand)}
                  >
                    {t('previewCommand')}
                  </button>
                </div>
                <pre>{remotePreview.previewCommand}</pre>
              </div>
              {remotePreview.manualSteps.length ? (
                <ul className="plainList">
                  {remotePreview.manualSteps.map((step) => (
                    <li key={step}>{step}</li>
                  ))}
                </ul>
              ) : null}
            </>
          ) : (
            <EmptyState>{t('previewUpload')}</EmptyState>
          )}
        </Surface>
      </>
    )
  }

  return (
    <div className="appShell">
      <aside className="sideRail">
        <div className="brandMark" aria-hidden="true">
          <Glyph filled icon="history" />
        </div>

        <nav className="navStack">
          {(Object.keys(viewMeta) as ViewId[]).map((viewId) => {
            const item = viewMeta[viewId]
            return (
              <button
                key={viewId}
                className={`navItem ${activeView === viewId ? 'active' : ''}`}
                disabled={item.disabled}
                type="button"
                title={item.label}
                aria-label={item.label}
                onClick={() => setActiveView(viewId)}
              >
                <Glyph icon={item.icon} />
                <span className="navLabel">{item.label}</span>
              </button>
            )
          })}
        </nav>

        <div className="sideRailFooter">
          <Glyph icon="settings" />
        </div>
      </aside>

      <main className="mainStage">
        <header className="topBar">
          <div className="topBarBrand">
            <p className="brandOverline">{t('productName')}</p>
            <h1>{t('productName')}</h1>
            <div className="statusStack">
              {statusItems.map((item) => (
                <span className="statusChip" key={item}>
                  {item}
                </span>
              ))}
            </div>
            {buildStampItems.length ? (
              <div className="buildStamp">
                {buildStampItems.map((item) => (
                  <span className="buildStampItem" key={item}>
                    {item}
                  </span>
                ))}
                {buildInfo?.gitDirty ? (
                  <StatusTag tone="danger">{t('workingTreeDirty')}</StatusTag>
                ) : null}
              </div>
            ) : null}
          </div>
          <div className="topBarActions">
            <button
              className="secondaryButton"
              type="button"
              onClick={handleDoctor}
            >
              <Glyph icon="health_and_safety" />
              {t('runDoctor')}
            </button>
            <button
              className="primaryButton"
              type="button"
              onClick={handleBackupRun}
            >
              <Glyph icon="play_arrow" filled />
              {t('runBackupNow')}
            </button>
          </div>
        </header>

        <section className="pageIntro">
          <p className="sectionEyebrow">{activeMeta.label}</p>
          <h2>{activeMeta.label}</h2>
          <p>{activeMeta.description}</p>
        </section>

        <div className="bannerStack">
          {notice ? <div className="banner success">{notice}</div> : null}
          {error ? <div className="banner danger">{error}</div> : null}
          {busyLabel ? <div className="banner info">{busyLabel}…</div> : null}
        </div>

        <section className="viewCanvas">
          <div className="contentColumn">{mainContent}</div>
          <aside className="inspectorColumn">{inspectorContent}</aside>
        </section>
      </main>
    </div>
  )
}

export default App
