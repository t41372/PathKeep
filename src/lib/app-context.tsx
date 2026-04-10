import {
  createContext,
  startTransition,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
import { backend } from './backend'
import {
  createTranslator,
  resolveLanguage,
  type ResolvedLanguage,
} from './i18n'
import {
  readDatabaseKeyStronghold,
  storeDatabaseKeyStronghold,
} from './stronghold'
import type {
  AiProviderConfig,
  AiProviderPurpose,
  AppBuildInfo,
  AppConfig,
  AppSnapshot,
  InsightStatus,
  RemoteBackupConfig,
} from './types'

// ---------------------------------------------------------------------------
// Empty defaults
// ---------------------------------------------------------------------------

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
  enrichmentEnabled: true,
  enrichmentPlugins: [
    { pluginId: 'title-normalization', enabled: true },
    { pluginId: 'readable-content-refetch', enabled: true },
  ],
  llmProviderId: null,
  embeddingProviderId: null,
  retrievalTopK: 8,
  assistantSystemPrompt: '',
  llmProviders: [],
  embeddingProviders: [],
}

export const EMPTY_CONFIG: AppConfig = {
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

export const EMPTY_DIRECTORIES: AppSnapshot['directories'] = {
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

export const EMPTY_ARCHIVE_STATUS: AppSnapshot['archiveStatus'] = {
  initialized: false,
  encrypted: true,
  unlocked: false,
  databasePath: '',
  lastSuccessfulBackupAt: null,
  warning: null,
}

export const EMPTY_KEYRING_STATUS: AppSnapshot['keyringStatus'] = {
  available: false,
  backend: '',
  storedSecret: false,
  message: null,
}

export const EMPTY_AI_STATUS: AppSnapshot['aiStatus'] = {
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

export const EMPTY_INSIGHT_STATUS: InsightStatus = {
  ready: false,
  lastRunAt: null,
  runs: 0,
  cards: 0,
  topics: 0,
  threads: 0,
  contentCoverage: 0,
  warning: null,
}

// ---------------------------------------------------------------------------
// View types
// ---------------------------------------------------------------------------

export type PageId =
  | 'dashboard'
  | 'explorer'
  | 'insights'
  | 'activity'
  | 'import'
  | 'settings'
  | 'onboarding'

export type SettingsTab =
  | 'general'
  | 'sources'
  | 'schedule'
  | 'security'
  | 'remote'
  | 'ai-providers'

// ---------------------------------------------------------------------------
// Context value shape
// ---------------------------------------------------------------------------

export interface AppContextValue {
  // Global data
  buildInfo: AppBuildInfo | null
  snapshot: AppSnapshot | null
  draftConfig: AppConfig
  resolvedLanguage: ResolvedLanguage
  t: ReturnType<typeof createTranslator>

  // Derived convenience fields
  initialized: boolean
  unlocked: boolean
  directories: AppSnapshot['directories']
  archiveStatus: AppSnapshot['archiveStatus']
  keyringStatus: AppSnapshot['keyringStatus']
  aiStatus: AppSnapshot['aiStatus']
  insightStatus: InsightStatus

  // Session
  sessionDatabaseKey: string | null

  // Navigation
  activePage: PageId
  setActivePage: (page: PageId) => void
  activeSettingsTab: SettingsTab
  setActiveSettingsTab: (tab: SettingsTab) => void

  // UI feedback
  busyLabel: string | null
  notice: string | null
  error: string | null
  setNotice: (message: string | null) => void
  setError: (message: string | null) => void

  // Config mutation
  updateConfig: (patch: Partial<AppConfig>) => void
  updateRemoteBackup: (patch: Partial<RemoteBackupConfig>) => void
  updateAiSettings: (patch: Partial<AppConfig['ai']>) => void
  addAiProvider: (purpose: AiProviderPurpose) => void
  updateAiProvider: (
    purpose: AiProviderPurpose,
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) => void
  removeAiProvider: (purpose: AiProviderPurpose, providerId: string) => void

  // Side-effect actions
  reloadSnapshot: () => Promise<void>
  persistConfig: (nextConfig: AppConfig) => Promise<void>
  runTask: (label: string, action: () => Promise<void>) => Promise<void>
  copyText: (value: string) => Promise<void>
  handleOpenPath: (path: string | null | undefined) => Promise<void>

  // Initialization actions
  handleInitialize: (
    masterPassword: string,
    confirmPassword: string,
    rememberKey: boolean,
  ) => Promise<void>
  handleUnlockWithPassword: (password: string) => Promise<void>
  handleRotateEncryption: (
    newPassword: string,
    newMode: AppConfig['archiveMode'],
  ) => Promise<void>

  // Provider secrets (ephemerally in-memory)
  providerSecrets: Record<string, string>
  setProviderSecrets: React.Dispatch<
    React.SetStateAction<Record<string, string>>
  >
}

// ---------------------------------------------------------------------------
// Context creation
// ---------------------------------------------------------------------------

const AppContext = createContext<AppContextValue | null>(null)

export function useApp(): AppContextValue {
  const context = useContext(AppContext)
  if (!context) {
    /* v8 ignore next -- always used within AppProvider in prod and tests */
    throw new Error('useApp must be used inside AppProvider')
  }
  return context
}

// ---------------------------------------------------------------------------
// Provider component
// ---------------------------------------------------------------------------

export function AppProvider({ children }: { children: ReactNode }) {
  // ------ core state ------
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null)
  const [draftConfig, setDraftConfig] = useState<AppConfig>(EMPTY_CONFIG)
  const [sessionDatabaseKey, setSessionDatabaseKey] = useState<string | null>(
    null,
  )

  // ------ navigation ------
  const [activePage, setActivePage] = useState<PageId>('dashboard')
  const [activeSettingsTab, setActiveSettingsTab] =
    useState<SettingsTab>('general')

  // ------ UI feedback ------
  const [busyLabel, setBusyLabel] = useState<string | null>(null)
  const [notice, setNoticeState] = useState<string | null>(null)
  const [error, setErrorState] = useState<string | null>(null)

  // ------ provider secrets (ephemeral) ------
  const [providerSecrets, setProviderSecrets] = useState<
    Record<string, string>
  >({})

  // ------ derived values ------
  const resolvedLanguage = resolveLanguage(draftConfig.preferredLanguage)
  const t = useMemo(
    () => createTranslator(resolvedLanguage),
    [resolvedLanguage],
  )
  const initialized = snapshot?.config.initialized ?? false
  const unlocked = snapshot?.archiveStatus.unlocked ?? false
  const directories = snapshot?.directories ?? EMPTY_DIRECTORIES
  const archiveStatus = snapshot?.archiveStatus ?? EMPTY_ARCHIVE_STATUS
  const keyringStatus = snapshot?.keyringStatus ?? EMPTY_KEYRING_STATUS
  const aiStatus = snapshot?.aiStatus ?? EMPTY_AI_STATUS
  const insightStatus = snapshot?.insightStatus ?? EMPTY_INSIGHT_STATUS

  // ------ bootstrap effect ------
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
          setActivePage(
            unlockedSnapshot.config.initialized ? 'dashboard' : 'onboarding',
          )
          startTransition(() => setNoticeState(translate('autoUnlockedNotice')))
          return
        }
      }

      setSnapshot(next)
      setDraftConfig(next.config)
      setActivePage(next.config.initialized ? 'dashboard' : 'onboarding')
    })()
  }, [])

  // ------ helpers ------
  const setNotice = useCallback(
    (message: string | null) => startTransition(() => setNoticeState(message)),
    [],
  )
  const setError = useCallback(
    (message: string | null) => setErrorState(message),
    [],
  )

  const runTask = useCallback(
    async (label: string, action: () => Promise<void>) => {
      setBusyLabel(label)
      setErrorState(null)
      try {
        await action()
      } catch (taskError) {
        setErrorState(
          taskError instanceof Error ? taskError.message : String(taskError),
        )
      } finally {
        setBusyLabel(null)
      }
    },
    [],
  )

  const reloadSnapshot = useCallback(async () => {
    const next = await backend.getAppSnapshot()
    setSnapshot(next)
    setDraftConfig(next.config)
  }, [])

  const copyText = useCallback(
    async (value: string) => {
      await navigator.clipboard.writeText(value)
      setNotice(t('copiedNotice'))
    },
    [setNotice, t],
  )

  const handleOpenPath = useCallback(
    async (path: string | null | undefined) => {
      if (!path) return
      await runTask(t('openAction'), async () => {
        const openedPath = await backend.openPathInFileManager(path)
        setNotice(t('openedDirectoryNotice', { path: openedPath }))
      })
    },
    [runTask, setNotice, t],
  )

  // ------ config mutation ------
  const updateConfig = useCallback((patch: Partial<AppConfig>) => {
    setDraftConfig((c) => ({ ...c, ...patch }))
  }, [])

  const updateRemoteBackup = useCallback(
    (patch: Partial<RemoteBackupConfig>) => {
      setDraftConfig((c) => ({
        ...c,
        remoteBackup: { ...c.remoteBackup, ...patch },
      }))
    },
    [],
  )

  const updateAiSettings = useCallback((patch: Partial<AppConfig['ai']>) => {
    setDraftConfig((c) => ({ ...c, ai: { ...c.ai, ...patch } }))
  }, [])

  const updateAiProviderCollection = useCallback(
    (
      purpose: AiProviderPurpose,
      updater: (providers: AiProviderConfig[]) => AiProviderConfig[],
    ) => {
      setDraftConfig((c) => {
        const key =
          purpose === 'llm'
            ? ('llmProviders' as const)
            : ('embeddingProviders' as const)
        return { ...c, ai: { ...c.ai, [key]: updater(c.ai[key]) } }
      })
    },
    [],
  )

  const addAiProvider = useCallback(
    (purpose: AiProviderPurpose) => {
      const id = `${purpose}-${crypto.randomUUID().slice(0, 8)}`
      updateAiProviderCollection(purpose, (providers) => [
        ...providers,
        {
          id,
          name:
            purpose === 'llm' ? 'New LLM provider' : 'New embedding provider',
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
    },
    [updateAiProviderCollection],
  )

  const updateAiProvider = useCallback(
    (
      purpose: AiProviderPurpose,
      providerId: string,
      patch: Partial<AiProviderConfig>,
    ) => {
      updateAiProviderCollection(purpose, (providers) =>
        providers.map((p) => (p.id === providerId ? { ...p, ...patch } : p)),
      )
    },
    [updateAiProviderCollection],
  )

  const removeAiProvider = useCallback(
    (purpose: AiProviderPurpose, providerId: string) => {
      updateAiProviderCollection(purpose, (providers) =>
        providers.filter((p) => p.id !== providerId),
      )
      setDraftConfig((c) => {
        const updates: Partial<AppConfig['ai']> = {}
        if (purpose === 'llm' && c.ai.llmProviderId === providerId) {
          updates.llmProviderId = null
        }
        if (
          purpose === 'embedding' &&
          c.ai.embeddingProviderId === providerId
        ) {
          updates.embeddingProviderId = null
        }
        if (Object.keys(updates).length === 0) return c
        return { ...c, ai: { ...c.ai, ...updates } }
      })
      setProviderSecrets((current) => {
        const next = { ...current }
        delete next[providerId]
        return next
      })
    },
    [updateAiProviderCollection],
  )

  // ------ sync autostart ------
  const syncAppAutostart = useCallback(async (nextConfig: AppConfig) => {
    try {
      const enabledNow = await isEnabled()
      if (nextConfig.appAutostart && !enabledNow) await enable()
      if (!nextConfig.appAutostart && enabledNow) await disable()
    } catch {
      // Unsupported during web preview.
    }
  }, [])

  const persistConfig = useCallback(
    async (nextConfig: AppConfig) => {
      await syncAppAutostart(nextConfig)
      const nextSnapshot = await backend.saveConfig(nextConfig)
      setSnapshot(nextSnapshot)
      setDraftConfig(nextSnapshot.config)
      setNotice(t('preferencesSavedNotice'))
    },
    [setNotice, syncAppAutostart, t],
  )

  // ------ initialize ------
  const handleInitialize = useCallback(
    async (
      masterPassword: string,
      confirmPassword: string,
      rememberKey: boolean,
    ) => {
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
          const bytes = crypto.getRandomValues(new Uint8Array(32))
          databaseKey = Array.from(bytes, (b) =>
            b.toString(16).padStart(2, '0'),
          ).join('')
          await backend.resetLocalSecretVault()
          await storeDatabaseKeyStronghold(
            masterPassword,
            databaseKey,
            directories.strongholdPath,
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
        setActivePage('dashboard')
        setNotice(t('initializedNotice'))
      })
    },
    [
      draftConfig,
      directories.strongholdPath,
      runTask,
      setNotice,
      syncAppAutostart,
      t,
    ],
  )

  // ------ unlock ------
  const handleUnlockWithPassword = useCallback(
    async (password: string) => {
      if (!password) {
        setError(t('enterMasterPassword'))
        return
      }

      await runTask(t('unlockArchive'), async () => {
        const databaseKey = await readDatabaseKeyStronghold(
          password,
          directories.strongholdPath,
        )
        if (!databaseKey) {
          throw new Error(
            'No database key was found in the Stronghold snapshot for that password.',
          )
        }
        await backend.setSessionDatabaseKey(databaseKey)
        setSessionDatabaseKey(databaseKey)
        await reloadSnapshot()
        setNotice(t('unlockSuccess'))
      })
    },
    [
      directories.strongholdPath,
      reloadSnapshot,
      runTask,
      setError,
      setNotice,
      t,
    ],
  )

  // ------ rotate encryption ------
  const handleRotateEncryption = useCallback(
    async (newPassword: string, newMode: AppConfig['archiveMode']) => {
      if (!sessionDatabaseKey) {
        setError(t('unlockBeforeRotate'))
        return
      }
      if (!newPassword && newMode === 'Encrypted') {
        setError(t('enterNewMasterPassword'))
        return
      }

      await runTask(t('rotateKey'), async () => {
        let newKey: string | null = null
        if (newMode === 'Encrypted') {
          const bytes = crypto.getRandomValues(new Uint8Array(32))
          newKey = Array.from(bytes, (b) =>
            b.toString(16).padStart(2, '0'),
          ).join('')
          await storeDatabaseKeyStronghold(
            newPassword,
            newKey,
            directories.strongholdPath,
          )
        }

        const nextSnapshot = await backend.rekeyArchive({
          newMode,
          newKey,
        })
        if (newKey) {
          await backend.setSessionDatabaseKey(newKey)
        }
        setSessionDatabaseKey(newKey)
        setSnapshot(nextSnapshot)
        setDraftConfig(nextSnapshot.config)
        setNotice(t('rotateSuccess'))
      })
    },
    [
      directories.strongholdPath,
      runTask,
      sessionDatabaseKey,
      setError,
      setNotice,
      t,
    ],
  )

  // ------ context value ------
  const value = useMemo<AppContextValue>(
    () => ({
      buildInfo,
      snapshot,
      draftConfig,
      resolvedLanguage,
      t,
      initialized,
      unlocked,
      directories,
      archiveStatus,
      keyringStatus,
      aiStatus,
      insightStatus,
      sessionDatabaseKey,
      activePage,
      setActivePage,
      activeSettingsTab,
      setActiveSettingsTab,
      busyLabel,
      notice,
      error,
      setNotice,
      setError,
      updateConfig,
      updateRemoteBackup,
      updateAiSettings,
      addAiProvider,
      updateAiProvider,
      removeAiProvider,
      reloadSnapshot,
      persistConfig,
      runTask,
      copyText,
      handleOpenPath,
      handleInitialize,
      handleUnlockWithPassword,
      handleRotateEncryption,
      providerSecrets,
      setProviderSecrets,
    }),
    [
      buildInfo,
      snapshot,
      draftConfig,
      resolvedLanguage,
      t,
      initialized,
      unlocked,
      directories,
      archiveStatus,
      keyringStatus,
      aiStatus,
      insightStatus,
      sessionDatabaseKey,
      activePage,
      activeSettingsTab,
      busyLabel,
      notice,
      error,
      setNotice,
      setError,
      updateConfig,
      updateRemoteBackup,
      updateAiSettings,
      addAiProvider,
      updateAiProvider,
      removeAiProvider,
      reloadSnapshot,
      persistConfig,
      runTask,
      copyText,
      handleOpenPath,
      handleInitialize,
      handleUnlockWithPassword,
      handleRotateEncryption,
      providerSecrets,
    ],
  )

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>
}
