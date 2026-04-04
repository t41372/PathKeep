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
import {
  createTranslator,
  languageLabel,
  localeTag,
  resolveLanguage,
  type ResolvedLanguage,
} from './lib/i18n'
import {
  readDatabaseKeyStronghold,
  storeDatabaseKeyStronghold,
} from './lib/stronghold'
import type {
  ApplyResult,
  AppConfig,
  AppSnapshot,
  ArchiveMode,
  BackupReport,
  BrowserProfile,
  ExportFormat,
  HealthReport,
  HistoryQueryResponse,
  ImportBatchDetail,
  RemoteBackupConfig,
  RemoteBackupPreview,
  SchedulePlan,
  TakeoutInspection,
} from './lib/types'

type ViewId = 'setup' | 'explorer' | 'backups' | 'import' | 'settings'

type PlatformId = 'macos' | 'windows' | 'linux'

function formatDateTime(
  value: string | null | undefined,
  language: ResolvedLanguage,
) {
  if (!value) {
    return null
  }

  return new Intl.DateTimeFormat(localeTag(language), {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value))
}

function formatDuration(durationMs: number | null | undefined) {
  if (!durationMs || durationMs <= 0) {
    return '0s'
  }

  const totalSeconds = Math.round(durationMs / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes === 0) {
    return `${seconds}s`
  }
  return `${minutes}m ${seconds}s`
}

function App() {
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

  useEffect(() => {
    void (async () => {
      const next = await backend.getAppSnapshot()
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
        if (!cancelled) {
          setImportBatchDetail(detail)
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
  }, [activeView, selectedImportBatchId])

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
    setDraftConfig((current) => (current ? { ...current, ...patch } : current))
  }

  function updateRemoteBackup(patch: Partial<RemoteBackupConfig>) {
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

  async function copyText(value: string) {
    await navigator.clipboard.writeText(value)
    setLocalizedNotice(t('copiedNotice'))
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

  async function persistConfig(nextConfig: AppConfig) {
    await syncAppAutostart(nextConfig)
    const nextSnapshot = await backend.saveConfig(nextConfig)
    setSnapshot(nextSnapshot)
    setDraftConfig(nextSnapshot.config)
    setRememberKey(nextSnapshot.config.rememberDatabaseKeyInKeyring)
    setLocalizedNotice(t('preferencesSavedNotice'))
  }

  function strongholdPath() {
    return snapshot?.directories.strongholdPath ?? ''
  }

  async function handleInitialize() {
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
      setSnapshot((current) =>
        current ? { ...current, keyringStatus: report } : current,
      )
      setLocalizedNotice(t('rememberStored'))
    })
  }

  async function handleClearRememberedKey() {
    await runTask(t('clearRememberedKey'), async () => {
      const report = await backend.keyringClearDatabaseKey()
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

  function toggleProfile(profileId: string) {
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

  function browserGlyph(profile: BrowserProfile) {
    switch (profile.browserFamily) {
      case 'firefox':
        return 'local_fire_department'
      case 'safari':
        return 'travel_explore'
      default:
        return 'language'
    }
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
    const stagingDir = snapshot?.directories.stagingDir ?? '/tmp'
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
        snapshot.archiveStatus.encrypted ? t('encrypted') : t('plaintext'),
        snapshot.archiveStatus.unlocked ? t('unlocked') : t('locked'),
        t('profilesDetected', { count: snapshot.browserProfiles.length }),
        t('dueEveryHours', { hours: draftConfig?.dueAfterHours ?? 72 }),
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
          (file) => file.absolutePath ?? file.relativePath,
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
                <span className="profileBrowserMark" aria-hidden="true">
                  <Glyph icon={browserGlyph(profile)} />
                </span>
                <div className="profileIdentity">
                  <div className="profileNameStack">
                    <span className="profileName">{profile.profileName}</span>
                    <span className="browserPill">{profile.browserName}</span>
                  </div>
                  <span className="profileId">{profile.profileId}</span>
                </div>
                <div className="profileMeta">
                  <span>{profile.userName ?? t('noSignedInUser')}</span>
                  <span>
                    {profile.historyExists
                      ? t('historyDetected')
                      : t('historyMissing')}
                  </span>
                  <span>
                    {profile.browserVersion ?? t('unknownBrowserVersion')}
                  </span>
                </div>
                <p className="profilePathText">
                  {profile.historyPath ?? profile.profilePath}
                </p>
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
            <DataRow
              label={t('storagePath')}
              value={snapshot?.directories.appRoot}
            />
            <DataRow
              label={t('archiveDatabase')}
              value={snapshot?.directories.archiveDatabasePath}
            />
            <DataRow
              label={t('auditRepository')}
              value={snapshot?.directories.auditRepoPath}
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
        title={
          selectedHistory?.title ?? selectedHistory?.url ?? t('notAvailable')
        }
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
            <DataRow
              label={t('titleLabel')}
              value={selectedHistory.title ?? t('notAvailable')}
            />
            <DataRow label={t('urlLabel')} value={selectedHistory.url} />
            <DataRow label={t('domain')} value={selectedHistory.domain} />
            <DataRow label={t('profile')} value={selectedHistory.profileId} />
            <DataRow
              label={t('duration')}
              value={formatDuration(selectedHistory.durationMs)}
            />
            <DataRow
              label={t('transition')}
              value={
                selectedHistory.transition != null
                  ? String(selectedHistory.transition)
                  : t('notAvailable')
              }
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
              value={
                draftConfig?.remoteBackup.lastUploadedAt
                  ? (formatDateTime(
                      draftConfig.remoteBackup.lastUploadedAt,
                      resolvedLanguage,
                    ) ?? t('pending'))
                  : t('noRemoteUploadYet')
              }
            />
            <InfoStat
              label={t('objectKey')}
              value={
                draftConfig?.remoteBackup.lastUploadedObjectKey ??
                t('notAvailable')
              }
            />
            <InfoStat
              label={t('lastError')}
              value={draftConfig?.remoteBackup.lastError ?? t('notAvailable')}
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
                  className={`checkRow ${check.ok ? 'ok' : 'bad'}`}
                  key={check.name}
                >
                  <strong>{check.name}</strong>
                  <p>{check.detail}</p>
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
                  value={draftConfig?.preferredLanguage ?? 'system'}
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
                    snapshot?.archiveStatus.encrypted
                      ? t('encrypted')
                      : t('plaintext')
                  }
                />
                <DataRow
                  label={t('status')}
                  value={
                    snapshot?.archiveStatus.unlocked
                      ? t('unlocked')
                      : t('locked')
                  }
                />
                <DataRow
                  label={t('keyringBackend')}
                  value={snapshot?.keyringStatus.backend ?? t('notAvailable')}
                />
                <DataRow
                  label={t('rememberedKey')}
                  value={
                    snapshot?.keyringStatus.storedSecret
                      ? t('present')
                      : t('absent')
                  }
                />
              </dl>

              {snapshot?.archiveStatus.encrypted ? (
                snapshot.archiveStatus.unlocked ? (
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
              checked={draftConfig?.remoteBackup.enabled ?? false}
              label={t('enableRemoteBackup')}
              onChange={(checked) => updateRemoteBackup({ enabled: checked })}
            />
            <ToggleRow
              checked={draftConfig?.remoteBackup.pathStyle ?? true}
              label={t('pathStyle')}
              onChange={(checked) => updateRemoteBackup({ pathStyle: checked })}
            />
            <ToggleRow
              checked={draftConfig?.remoteBackup.uploadAfterBackup ?? false}
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
                  value={draftConfig?.remoteBackup.bucket ?? ''}
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
                  value={draftConfig?.remoteBackup.region ?? ''}
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
                  value={draftConfig?.remoteBackup.prefix ?? ''}
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
                  value={draftConfig?.remoteBackup.endpoint ?? ''}
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
              value={
                draftConfig?.remoteBackup.credentialsSaved ? t('yes') : t('no')
              }
            />
            <DataRow
              label={t('lastUploadAt')}
              value={
                formatDateTime(
                  draftConfig?.remoteBackup.lastUploadedAt ?? null,
                  resolvedLanguage,
                ) ?? t('notAvailable')
              }
            />
            <DataRow
              label={t('objectKey')}
              value={
                draftConfig?.remoteBackup.lastUploadedObjectKey ??
                t('notAvailable')
              }
            />
            <DataRow
              label={t('lastError')}
              value={draftConfig?.remoteBackup.lastError ?? t('notAvailable')}
            />
          </dl>
          <div className="toolbarActions">
            <ToggleRow
              checked={draftConfig?.appAutostart ?? false}
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
                <span className="srOnly">{item.label}</span>
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

function Surface({
  eyebrow,
  title,
  icon,
  actions,
  children,
}: {
  eyebrow: string
  title: string
  icon: string
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="surface">
      <header className="surfaceHeader">
        <div className="surfaceTitle">
          <span className="surfaceIcon">
            <Glyph icon={icon} />
          </span>
          <div>
            <p className="sectionEyebrow">{eyebrow}</p>
            {title ? <h3>{title}</h3> : null}
          </div>
        </div>
        {actions ? <div className="surfaceActions">{actions}</div> : null}
      </header>
      <div className="surfaceBody">{children}</div>
    </section>
  )
}

function FieldBlock({ label, control }: { label: string; control: ReactNode }) {
  return (
    <label className="fieldBlock">
      <span className="fieldLabel">{label}</span>
      {control}
    </label>
  )
}

function ToggleRow({
  checked,
  label,
  onChange,
}: {
  checked: boolean
  label: string
  onChange: (checked: boolean) => void
}) {
  return (
    <label className="toggleRow">
      <span>{label}</span>
      <input
        checked={checked}
        type="checkbox"
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  )
}

function DataRow({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="dataRow">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  )
}

function EmptyState({ children }: { children: ReactNode }) {
  return <div className="emptyState">{children}</div>
}

function InfoStat({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="infoStat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

type WorkflowStep = {
  id: string
  title: string
  status: 'pending' | 'complete'
  summary: string
  reason: string
  files?: string[]
  commands?: string[]
  checklist?: string[]
  actions?: ReactNode
}

function OperationWorkflow({
  actionLabel,
  labels,
  language,
  onCopy,
  steps,
}: {
  actionLabel: string
  labels: {
    why: string
    files: string
    commands: string
    checklist: string
    copy: string
    current: string
    complete: string
    pending: string
  }
  language: ResolvedLanguage
  onCopy: (value: string) => Promise<void>
  steps: WorkflowStep[]
}) {
  const currentIndex = steps.findIndex((step) => step.status !== 'complete')

  return (
    <ol className="workflowList" aria-label={actionLabel}>
      {steps.map((step, index) => {
        const displayStatus =
          step.status === 'complete'
            ? 'complete'
            : currentIndex === index
              ? 'current'
              : 'pending'

        return (
          <li className={`workflowStep ${displayStatus}`} key={step.id}>
            <div className="workflowMarker">
              <span>{index + 1}</span>
            </div>
            <div className="workflowCard">
              <div className="workflowHeader">
                <div>
                  <p className="sectionEyebrow">
                    {displayStatus === 'complete'
                      ? labels.complete
                      : displayStatus === 'current'
                        ? labels.current
                        : labels.pending}
                  </p>
                  <h3>{step.title}</h3>
                </div>
                <StatusTag
                  tone={
                    displayStatus === 'complete'
                      ? 'success'
                      : displayStatus === 'current'
                        ? 'info'
                        : 'neutral'
                  }
                >
                  {displayStatus === 'complete'
                    ? labels.complete
                    : displayStatus === 'current'
                      ? labels.current
                      : labels.pending}
                </StatusTag>
              </div>
              <p className="workflowSummary">{step.summary}</p>
              <div className="workflowSection">
                <strong>{labels.why}</strong>
                <p>{step.reason}</p>
              </div>
              {step.files?.length ? (
                <div className="workflowSection">
                  <strong>{labels.files}</strong>
                  <div className="artifactList">
                    {step.files.map((file) => (
                      <article className="artifactCard compactCard" key={file}>
                        <strong>{file}</strong>
                      </article>
                    ))}
                  </div>
                </div>
              ) : null}
              {step.commands?.length ? (
                <div className="workflowSection">
                  <strong>{labels.commands}</strong>
                  <div className="generatedList">
                    {step.commands.map((command) => (
                      <article className="codeArtifact" key={command}>
                        <div className="artifactHeader">
                          <strong>
                            {formatDateTime(new Date().toISOString(), language)}
                          </strong>
                          <button
                            className="ghostButton"
                            type="button"
                            onClick={() => void onCopy(command)}
                          >
                            {labels.copy}
                          </button>
                        </div>
                        <pre>{command}</pre>
                      </article>
                    ))}
                  </div>
                </div>
              ) : null}
              {step.checklist?.length ? (
                <div className="workflowSection">
                  <strong>{labels.checklist}</strong>
                  <ol className="stepList">
                    {step.checklist.map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ol>
                </div>
              ) : null}
              {step.actions ? (
                <div className="workflowActions">{step.actions}</div>
              ) : null}
            </div>
          </li>
        )
      })}
    </ol>
  )
}

function PreviewEntryList({
  entries,
  language,
}: {
  entries: TakeoutInspection['previewEntries']
  language: ResolvedLanguage
}) {
  return (
    <div className="previewList">
      {entries.map((entry) => (
        <article
          className="previewEntry"
          key={`${entry.sourcePath}:${entry.sourceVisitId}`}
        >
          <div className="previewMeta">
            <span>{formatDateTime(entry.visitedAt, language)}</span>
            <StatusTag tone={entry.status === 'imported' ? 'success' : 'info'}>
              {entry.status}
            </StatusTag>
          </div>
          <strong>{entry.title || entry.url}</strong>
          <p>{entry.url}</p>
          <small>
            {entry.sourcePath} · #{entry.sourceVisitId}
          </small>
        </article>
      ))}
    </div>
  )
}

function StatusTag({
  tone,
  children,
}: {
  tone: 'info' | 'success' | 'danger' | 'neutral'
  children: ReactNode
}) {
  return <span className={`statusTag ${tone}`}>{children}</span>
}

function Glyph({ icon, filled = false }: { icon: string; filled?: boolean }) {
  return (
    <span
      className={`material-symbols-outlined glyph ${filled ? 'filled' : ''}`}
    >
      {icon}
    </span>
  )
}

export default App
