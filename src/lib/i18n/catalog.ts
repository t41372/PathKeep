import type { LanguagePreference } from '../types'

export type ResolvedLanguage = 'en' | 'zh-CN' | 'zh-TW'
export type TranslationNamespace =
  | 'common'
  | 'shell'
  | 'navigation'
  | 'dashboard'
  | 'audit'
  | 'import'
  | 'schedule'
  | 'security'
  | 'settings'
  | 'platform'

interface TranslationDictionary {
  [key: string]: string | TranslationDictionary
}

export const supportedLanguages: ResolvedLanguage[] = ['en', 'zh-CN', 'zh-TW']
export const translationNamespaces: TranslationNamespace[] = [
  'common',
  'shell',
  'navigation',
  'dashboard',
  'audit',
  'import',
  'schedule',
  'security',
  'settings',
  'platform',
]

const catalog: Record<
  ResolvedLanguage,
  Record<TranslationNamespace, TranslationDictionary>
> = {
  en: {
    common: {
      followSystem: 'Follow system',
      english: 'English',
      simplifiedChinese: '简体中文',
      traditionalChinese: '繁體中文',
      openAction: 'Open',
      copyAction: 'Copy',
      openPath: 'Open path',
      copiedNotice: 'Copied to clipboard.',
      loading: 'Loading',
      unavailable: 'Unavailable',
      notAvailable: 'Not available',
      pending: 'Pending',
      current: 'Current',
      complete: 'Complete',
      warning: 'Warning',
      yes: 'Yes',
      no: 'No',
      reviewAction: 'Review',
      rescanAction: 'Rescan',
      runDoctorAction: 'Run doctor',
      repairAction: 'Repair',
      previewTab: 'Preview',
      manualTab: 'Manual',
      executeTab: 'Execute',
      verifyTab: 'Verify',
      whyThisStepMatters: 'Why this step matters',
      filesLabel: 'Artifacts',
      commandsLabel: 'Commands',
      checklistLabel: 'Checklist',
    },
    shell: {
      savingArchiveChoices: 'Saving archive choices',
      preparingArchive: 'Preparing the archive',
      runningManualBackup: 'Running a manual backup',
      loadingLatestArchiveState:
        'PathKeep could not load the latest archive state.',
      savingSettingsFailed:
        'PathKeep could not save the updated archive settings.',
      initializeArchiveFailed: 'PathKeep could not initialize the archive.',
      initializedNotice:
        'Archive initialized. Review the first backup before automation.',
      manualBackupDueWindow: 'The archive is still inside the due window.',
      manualBackupFinished: 'Manual backup finished as run #{runId}.',
      manualBackupFailed: 'PathKeep could not complete the manual backup.',
    },
    navigation: {
      coreSection: 'CORE',
      operationsSection: 'OPERATIONS',
      systemSection: 'SYSTEM',
      dashboardLabel: 'Dashboard',
      dashboardTitle: 'Dashboard',
      dashboardSubtitle: 'Archive overview & system status',
      explorerLabel: 'Explorer',
      explorerTitle: 'History Explorer',
      explorerSubtitle: 'Browse, search & filter your archive',
      insightsLabel: 'Insights',
      insightsTitle: 'Insights',
      insightsSubtitle: 'Topics, threads & browsing patterns',
      assistantLabel: 'AI Assistant',
      assistantTitle: 'AI Assistant',
      assistantSubtitle: 'Ask questions about your browsing history',
      assistantBadge: 'OPT',
      importLabel: 'Import',
      importTitle: 'Import',
      importSubtitle: 'Google Takeout & browser direct import',
      auditLabel: 'Audit Ledger',
      auditTitle: 'Audit Ledger',
      auditSubtitle: 'Manifest chain, run history & integrity',
      scheduleLabel: 'Schedule',
      scheduleTitle: 'Schedule',
      scheduleSubtitle: 'Backup schedule & install artifacts',
      securityLabel: 'Security',
      securityTitle: 'Security',
      securitySubtitle: 'Encryption, keyring & password management',
      settingsLabel: 'Settings',
      settingsTitle: 'Settings',
      settingsSubtitle: 'Profiles, language & platform guidance',
      onboardingLabel: 'Onboarding',
      onboardingTitle: 'Onboarding / Setup',
      onboardingSubtitle:
        'Preview, manual guidance, and first-run archive decisions',
      archiveAttentionNeeded: 'Archive attention needed',
      archiveHealthy: 'Archive healthy',
      archiveNotInitialized: 'Archive not initialized',
      encryptedArchive: 'Encrypted archive',
      plaintextArchive: 'Plaintext archive',
      loadingBuild: 'Loading build',
      expandNavigation: 'Expand navigation',
      collapseNavigation: 'Collapse navigation',
      toggleTheme: 'Toggle theme',
      searchHistory: 'Search history',
      searchHistoryPlaceholder: 'Search history...  ⌘K',
      backupNow: 'Backup now',
      initializeFirst: 'Initialize first',
    },
    dashboard: {
      loadingOverview: 'Loading archive overview',
      archiveReadError: 'Dashboard could not read the archive',
      archiveUnavailable: 'Dashboard data is unavailable',
      archiveUnavailableBody:
        'PathKeep could not load the current archive snapshot.',
      totalRecords: 'TOTAL RECORDS',
      uniqueUrls: '{count} unique URLs',
      lastBackup: 'LAST BACKUP',
      noManifestYet: 'No manifest written to the chain yet',
      profilesInScope: 'PROFILES IN SCOPE',
      profilesReadableAttention:
        '{readable} readable · {attention} need attention',
      archiveMode: 'ARCHIVE MODE',
      archiveUnlocked: 'Archive session is unlocked',
      archiveNeedsUnlock:
        'Archive requires an unlock before Explorer / Audit can read',
      zeroStateTitle: 'The first archive run still needs review',
      zeroStateBody:
        'Dashboard cards stay empty until the archive is initialized and the first manual backup finishes.',
      zeroStateEyebrow: 'DAY-ONE',
      openOnboardingFlow: 'Open onboarding flow',
      recentRuns: 'RECENT RUNS',
      fullLedger: 'Full ledger →',
      run: 'RUN',
      type: 'TYPE',
      source: 'SOURCE',
      records: 'RECORDS',
      status: 'STATUS',
      time: 'TIME',
      backupType: 'BACKUP',
      profilesLabel: '{count} profiles',
      archiveBoundary: 'ARCHIVE BOUNDARY',
      selectedProfiles: '{count} selected profiles',
      historyDetected: 'History database detected',
      historyMissing: 'History database not found',
      trustActions: 'TRUST ACTIONS',
      trustActionsBody:
        'Keep permissions, scheduler state, and rollback surfaces inspectable before the next write.',
      reviewImportBatches: 'Review import batches',
      reviewSecurity: 'Review security',
      reviewSchedule: 'Review schedule',
      storageFootprint: 'STORAGE FOOTPRINT',
      storageTotal: '{size} total',
      archiveDatabase: 'Archive database',
      manifests: 'Manifests',
      snapshots: 'Snapshots',
      exports: 'Exports',
    },
    audit: {
      loadingLedger: 'Loading audit ledger',
      unavailableTitle: 'Audit ledger is unavailable',
      finishOnboarding: 'Finish onboarding',
      emptyLedgerTitle: 'The audit ledger has no archive runs yet',
      emptyLedgerBody:
        'Audit records appear after the first successful backup writes a manifest and artifact trail.',
      noRunsTitle: 'No backup runs recorded yet',
      noRunsBody:
        'The audit ledger will populate as soon as a manual backup finishes and PathKeep writes the manifest chain.',
      runManualBackup: 'Run a manual backup',
      manifestChain: 'MANIFEST CHAIN',
      verifyIntegrity: 'Verify integrity',
      loadingRunDetail: 'Loading run detail',
      runDetailUnavailable: 'Run detail is unavailable',
      manifestDetail: 'RUN #{runId} · MANIFEST DETAIL',
      runId: 'RUN ID',
      runType: 'TYPE',
      runSource: 'SOURCE',
      executedAt: 'EXECUTED AT',
      manifestHash: 'MANIFEST HASH',
      manifestPath: 'MANIFEST PATH',
      manualBackup: 'Manual Backup',
      scheduledBackup: 'Scheduled Backup',
      archiveWide: 'Archive-wide',
      newVisits: 'New visits',
      newUrls: 'New URLs',
      downloads: 'Downloads',
      profiles: 'Profiles',
      artifacts: 'ARTIFACTS · {count} files',
      warningsTitle: 'WARNINGS',
      viewManifest: 'View Manifest',
      copyPath: 'Copy Path',
      copied: 'Copied',
      detailEmptyTitle: 'No audit run selected',
      detailEmptyBody:
        'Click a block in the manifest chain above to inspect run details, artifacts, and the hash trail.',
      repairRoutesTitle: 'TRUST RECOVERY',
      repairRoutesBody:
        'Use the linked pages to inspect imports, scheduler state, or encryption before the next risky operation.',
      repairImports: 'Import review',
      repairSchedule: 'Schedule troubleshooting',
      repairSecurity: 'Security review',
    },
    import: {
      archiveNotInitialized: 'Archive not initialized',
      archiveNotInitializedBody:
        'Initialize the archive first before importing external history data.',
      trustTitle: 'TRUSTED IMPORT FLOW',
      trustBody:
        'Preview first, keep unsupported files quarantined, and verify every rollback path before you write to the archive.',
      takeoutMethodTitle: 'Google Takeout',
      takeoutMethodBody: 'Import from exported archive',
      browserMethodTitle: 'Browser Direct',
      browserMethodBody: 'Import from local browser DB',
      stepUpload: 'Upload',
      stepScan: 'Scan',
      stepPreview: 'Preview',
      stepConfirm: 'Confirm',
      stepImport: 'Import',
      selectTitle: 'Step 1: Select Source',
      takeoutSelectBody:
        'Provide the path to your Google Takeout export (zip or folder).',
      browserSelectBody: 'Provide the path to a browser History database file.',
      sourcePath: 'SOURCE PATH',
      takeoutPathPlaceholder: '/path/to/takeout.zip',
      browserPathPlaceholder: '/path/to/History',
      scanSource: 'Scan source →',
      scanningTitle: 'Step 2: Scanning...',
      scanningBody: 'Inspecting source files for recognized history formats.',
      previewTitle: 'Step 3: Preview Import',
      previewBody: 'Review what will be imported before confirming.',
      recordsFound: 'Records Found',
      duplicates: 'Duplicates',
      newRecords: 'New Records',
      detectedFiles: 'DETECTED FILES',
      quarantinedFiles: 'QUARANTINED FILES',
      backAction: '← Back',
      confirmImport: 'Confirm import →',
      importingTitle: 'Step 4: Importing...',
      importingBody: 'Writing records to the archive. This may take a moment.',
      completeTitle: 'Step 5: Import Complete',
      completeBody: 'Records have been written to the archive.',
      imported: 'Imported',
      duplicatesSkipped: 'Duplicates Skipped',
      importAnother: 'Import another',
      workflowLabel: 'Import PME workflow',
      workflowPreviewTitle: 'Preview boundary',
      workflowPreviewSummary:
        'Inspect recognized files, preview rows, and quarantine notes before any write occurs.',
      workflowPreviewReason:
        'Preview clarifies scope, duplicate risk, and whether unsupported files stay isolated.',
      workflowManualTitle: 'Manual inspection',
      workflowManualSummary:
        'Use the same steps yourself and inspect the source archive outside the app.',
      workflowManualReason:
        'Manual review makes privacy impact and file provenance explicit before execute.',
      workflowExecuteTitle: 'Execute import',
      workflowExecuteSummary:
        'Import only the recognized rows after the preview and manual checks look correct.',
      workflowExecuteReason:
        'Execute should be deliberate because it writes a new import batch into the archive.',
      workflowVerifyTitle: 'Verify result',
      workflowVerifySummary:
        'Inspect the batch detail, preview rows, and rollback controls immediately after import.',
      workflowVerifyReason:
        'Verification keeps rollback and restore close at hand when a batch looks suspicious.',
      workflowFinishTitle: 'Keep rollback nearby',
      workflowFinishSummary:
        'Leave the flow only after the audit path, revert button, and visible row counts look right.',
      workflowFinishReason:
        'Import trust comes from a visible rollback story, not from a success toast alone.',
      manualLocateStep: 'Locate the archive or extracted folder.',
      manualInspectStep:
        'Inspect the files and compare them against the recognized list.',
      manualContinueStep:
        'Return here and continue only when the preview looks correct.',
      recentBatches: 'RECENT IMPORT BATCHES',
      recentBatchesBody:
        'Every import stays reviewable, revertible, and restorable from local audit state.',
      noImportBatches: 'No import batches have been recorded yet.',
      selectedBatch: 'SELECTED BATCH',
      selectedBatchBody:
        'Choose a batch to inspect preview rows, notes, and rollback controls.',
      previewRows: 'Preview rows',
      noPreviewRows: 'No preview rows are available for this selection yet.',
      candidateRows: 'Candidate rows',
      importedRows: 'Imported rows',
      duplicateRows: 'Duplicate rows',
      visibleRows: 'Visible rows',
      auditPath: 'Audit path',
      revertBatch: 'Revert batch',
      restoreBatch: 'Restore batch',
      revertConfirm:
        'Revert this import batch from the live archive? The raw audit trail stays preserved.',
      restoreConfirm:
        'Restore this previously reverted import batch back into the live archive view?',
      healthReport: 'DOCTOR REPORT',
      healthReportBody:
        'Run doctor after import, rollback, or restore to verify derived state and missing artifacts.',
      noHealthChecks: 'Doctor checks have not been run yet in this view.',
      repairSummary:
        'Repair cleared {derivedRows} derived rows, restored {visibilityRows} visibility links, and rebuilt {importAudits} import audit artifacts.',
    },
    schedule: {
      loadingPreview: 'Rendering native schedule preview',
      unavailableTitle: 'Schedule preview unavailable',
      unavailableBody:
        'PathKeep could not render the native schedule artifacts.',
      backupSchedule: 'BACKUP SCHEDULE',
      installState: 'Install state',
      interval: 'Interval',
      verification: 'Verification',
      mechanism: 'Mechanism',
      lastTriggered: 'Last triggered',
      label: 'Label',
      profiles: 'Profiles',
      pmeTitle: 'SCHEDULE PME',
      previewBoundary: 'PREVIEW BOUNDARY',
      previewBody:
        'Review the exact scheduler artifact, install plan, and rollback commands before trusting native automation.',
      noGeneratedFiles:
        'No generated files are available in browser preview mode. Open the desktop build to inspect the full native artifact.',
      openLatestAudit: 'Open latest scheduler audit',
      executeRun: 'RUN',
      executeBody:
        'Execute installs or removes the current native schedule plan only after the preview and warnings look right.',
      applyCommand: 'APPLY COMMAND {index}',
      rollbackCommand: 'ROLLBACK COMMAND {index}',
      applySchedule: 'Apply schedule',
      removeSchedule: 'Remove schedule',
      openSchedulerAudit: 'Open scheduler audit',
      initializeArchiveFirst:
        'Initialize the archive first, then return here to apply the reviewed native schedule.',
      installedDescription:
        'Native schedule files match the current PathKeep plan.',
      mismatchDescription:
        'Installed files exist, but they no longer match the current preview.',
      permissionWarningDescription:
        'PathKeep could not inspect the installed files cleanly on this machine.',
      legacyInstallDescription:
        'A legacy install is still present. Review it before trusting this schedule.',
      manualReviewDescription:
        'This platform stays manual-first in v1. Verify it using the documented steps.',
      notInstalledDescription: 'No installed native schedule was detected yet.',
      installedBadge: 'Installed',
      attentionBadge: 'Attention',
      manualReviewBadge: 'Manual review',
      notInstalledBadge: 'Not installed',
    },
    security: {
      loadingPosture: 'Loading security posture',
      unavailableTitle: 'Security posture is unavailable',
      unavailableBody:
        'PathKeep needs the local app snapshot before it can describe the current encryption and keyring posture.',
      initFirstAction: 'Initialize archive first',
      notInitializedTitle: 'The archive has not been initialized yet',
      notInitializedBody:
        'Security review becomes meaningful after onboarding creates the archive and the first backup writes the baseline manifest chain.',
      encryptionStatus: 'ENCRYPTION STATUS',
      archiveIs: 'Archive is {mode}',
      encryptedDetail: 'SQLCipher at rest · unlock required before read access',
      plaintextDetail:
        'Standard SQLite archive · disk encryption depends on the host system',
      keyring: 'Keyring',
      sessionStatus: 'Session status',
      sessionUnlocked: 'Archive is currently unlocked',
      sessionLocked:
        'Archive is locked — Explorer and Audit remain read-blocked',
      lastBackup: 'Last backup',
      stronghold: 'Stronghold',
      archivePath: 'Archive path',
      passwordLossTitle: 'Password loss = data loss.',
      passwordLossBody:
        'PathKeep does not have a recovery backdoor. Keep the current or future database key in a secure place before re-keying.',
      unlockKeyringTitle: 'UNLOCK + KEYRING',
      sessionActive: 'Session active',
      needsUnlock: 'Needs unlock',
      currentDatabaseKey: 'CURRENT DATABASE KEY',
      currentDatabaseKeyPlaceholder: 'Enter current archive key',
      unlockArchive: 'Unlock archive',
      useKeyring: 'Use keyring',
      lockArchive: 'Lock archive',
      storeInKeyring: 'Store in keyring',
      clearKeyring: 'Clear keyring',
      keyringConvenience:
        'Storing the key in the native keyring is optional convenience unlock. PathKeep still keeps the archive local-first and does not upload secrets anywhere.',
      rekeyTitle: 'RE-KEY PREVIEW',
      previewBeforeExecute: 'Preview before execute',
      targetMode: 'TARGET MODE',
      newDatabaseKey: 'NEW DATABASE KEY',
      newDatabaseKeyPlaceholder: 'Enter the replacement archive key',
      storeNewKey:
        'Store the new database key in the native keyring after execute',
      previewRekey: 'Preview re-key',
      executeRekey: 'Execute re-key',
      mode: 'MODE',
      snapshot: 'SNAPSHOT',
      temporaryDatabase: 'TEMP',
    },
    settings: {
      loadingSettings: 'Loading settings',
      loadingModules: 'Settings modules loading',
      browserProfiles: 'BROWSER PROFILES',
      aiProvider: 'AI PROVIDER',
      optional: 'OPTIONAL',
      general: 'GENERAL',
      baseUrlLabel: 'Base URL',
      embeddingModelLabel: 'Embedding Model',
      llmModelLabel: 'LLM Model',
      apiKeyLabel: 'API Key',
      interfaceLanguage: 'Interface language',
      currentLanguage: 'Current language',
      dataDirectory: 'Data directory',
      openDirectory: 'Open in file manager',
      mcpServer: 'MCP Server',
      version: 'Version',
      enabled: 'Enabled',
      disabled: 'Disabled',
      historyFound: 'History found',
      noHistoryDetected: 'No history file detected',
      platformTroubleshooting: 'PLATFORM TROUBLESHOOTING',
      platformBody:
        'Keep scheduler install paths, Safari access rules, and keyring limitations visible from Settings.',
      reviewSchedule: 'Open schedule',
      reviewSecurity: 'Open security',
      reviewImports: 'Open imports',
    },
    platform: {
      macosLabel: 'macOS LaunchAgent',
      windowsLabel: 'Windows Task Scheduler',
      linuxLabel: 'Linux systemd user timer',
      macosSummary:
        'macOS can install and inspect a LaunchAgent directly from PathKeep when the desktop shell is available.',
      windowsSummary:
        'Windows uses Task Scheduler. Preview the XML, keep the manual import path visible, and verify StartWhenAvailable behavior before apply.',
      linuxSummary:
        'Linux uses a systemd user timer with OnCalendar + Persistent. Manual review stays first-class even when apply becomes available.',
      safariAccessTitle: 'Safari still needs Full Disk Access',
      safariAccessBody:
        'Keep the Safari profile visible and guide the user to grant Full Disk Access instead of silently hiding the profile.',
      keyringTitle: 'Native keyring unavailable',
      keyringBody:
        'Encrypted mode is still allowed, but unattended unlock and remembered-key flows stay unavailable until the machine exposes a supported keyring backend.',
      schedulerMismatchTitle: 'Installed scheduler needs review',
      schedulerMismatchBody:
        'Mismatch, legacy installs, or permission warnings should send the user back through Preview → Manual → Execute instead of pretending the schedule is healthy.',
    },
  },
  'zh-CN': {
    common: {
      followSystem: '跟随系统',
      english: 'English',
      simplifiedChinese: '简体中文',
      traditionalChinese: '繁體中文',
      openAction: '打开',
      copyAction: '复制',
      openPath: '打开路径',
      copiedNotice: '已复制到剪贴板。',
      loading: '加载中',
      unavailable: '不可用',
      notAvailable: '不可用',
      pending: '待处理',
      current: '当前',
      complete: '已完成',
      warning: '警告',
      yes: '是',
      no: '否',
      reviewAction: '查看',
      rescanAction: '重新扫描',
      runDoctorAction: '运行检查',
      repairAction: '修复',
      previewTab: '预览',
      manualTab: '手动',
      executeTab: '执行',
      verifyTab: '验证',
      whyThisStepMatters: '为什么需要这一步',
      filesLabel: '工件',
      commandsLabel: '命令',
      checklistLabel: '检查清单',
    },
    shell: {
      savingArchiveChoices: '正在保存归档选项',
      preparingArchive: '正在准备归档',
      runningManualBackup: '正在运行手动备份',
      loadingLatestArchiveState: 'PathKeep 无法加载最新的归档状态。',
      savingSettingsFailed: 'PathKeep 无法保存更新后的设置。',
      initializeArchiveFailed: 'PathKeep 无法初始化归档。',
      initializedNotice: '归档已初始化。请在开启自动化前先检查第一次备份。',
      manualBackupDueWindow: '归档仍处于未到期窗口内。',
      manualBackupFinished: '手动备份已完成，运行编号 #{runId}。',
      manualBackupFailed: 'PathKeep 无法完成手动备份。',
    },
    navigation: {
      coreSection: '核心',
      operationsSection: '操作',
      systemSection: '系统',
      dashboardLabel: '仪表盘',
      dashboardTitle: '仪表盘',
      dashboardSubtitle: '归档概览与系统状态',
      explorerLabel: '浏览器',
      explorerTitle: '历史浏览器',
      explorerSubtitle: '浏览、搜索并筛选归档',
      insightsLabel: '洞察',
      insightsTitle: '洞察',
      insightsSubtitle: '主题、线程与浏览模式',
      assistantLabel: 'AI 助手',
      assistantTitle: 'AI 助手',
      assistantSubtitle: '就你的浏览历史提问',
      assistantBadge: '可选',
      importLabel: '导入',
      importTitle: '导入',
      importSubtitle: 'Google Takeout 与浏览器直接导入',
      auditLabel: '审计账本',
      auditTitle: '审计账本',
      auditSubtitle: 'Manifest 链、运行历史与完整性',
      scheduleLabel: '计划',
      scheduleTitle: '计划',
      scheduleSubtitle: '备份计划与安装工件',
      securityLabel: '安全',
      securityTitle: '安全',
      securitySubtitle: '加密、钥匙串与密码管理',
      settingsLabel: '设置',
      settingsTitle: '设置',
      settingsSubtitle: '配置文件、语言与平台指引',
      onboardingLabel: '初始化',
      onboardingTitle: '初始化 / 设置',
      onboardingSubtitle: '首轮归档决策的预览、手动指引与执行',
      archiveAttentionNeeded: '归档需要关注',
      archiveHealthy: '归档健康',
      archiveNotInitialized: '归档尚未初始化',
      encryptedArchive: '加密归档',
      plaintextArchive: '明文归档',
      loadingBuild: '正在加载构建信息',
      expandNavigation: '展开导航',
      collapseNavigation: '收起导航',
      toggleTheme: '切换主题',
      searchHistory: '搜索历史',
      searchHistoryPlaceholder: '搜索历史...  ⌘K',
      backupNow: '立即备份',
      initializeFirst: '请先初始化',
    },
    dashboard: {
      loadingOverview: '正在加载归档概览',
      archiveReadError: '仪表盘无法读取归档',
      archiveUnavailable: '仪表盘数据不可用',
      archiveUnavailableBody: 'PathKeep 无法加载当前归档快照。',
      totalRecords: '总记录数',
      uniqueUrls: '{count} 个唯一 URL',
      lastBackup: '上次备份',
      noManifestYet: '链中还没有写入 manifest',
      profilesInScope: '归档范围内的配置文件',
      profilesReadableAttention: '{readable} 个可读 · {attention} 个需要关注',
      archiveMode: '归档模式',
      archiveUnlocked: '当前会话中的归档已解锁',
      archiveNeedsUnlock: '在 Explorer / Audit 读取前仍需要先解锁归档',
      zeroStateTitle: '第一次归档运行仍需要检查',
      zeroStateBody:
        '只有在归档初始化并完成第一次手动备份后，仪表盘卡片才会变成有数据状态。',
      zeroStateEyebrow: '首日',
      openOnboardingFlow: '打开初始化流程',
      recentRuns: '最近运行',
      fullLedger: '完整账本 →',
      run: '运行',
      type: '类型',
      source: '来源',
      records: '记录',
      status: '状态',
      time: '时间',
      backupType: '备份',
      profilesLabel: '{count} 个配置文件',
      archiveBoundary: '归档边界',
      selectedProfiles: '已选择 {count} 个配置文件',
      historyDetected: '已检测到历史数据库',
      historyMissing: '未找到历史数据库',
      trustActions: '信任动作',
      trustActionsBody:
        '在下一次写入前，先把权限、计划状态与回滚入口都检查一遍。',
      reviewImportBatches: '检查导入批次',
      reviewSecurity: '检查安全状态',
      reviewSchedule: '检查计划状态',
      storageFootprint: '存储占用',
      storageTotal: '总计 {size}',
      archiveDatabase: '归档数据库',
      manifests: 'Manifest',
      snapshots: '快照',
      exports: '导出',
    },
    audit: {
      loadingLedger: '正在加载审计账本',
      unavailableTitle: '审计账本不可用',
      finishOnboarding: '完成初始化',
      emptyLedgerTitle: '审计账本里还没有归档运行',
      emptyLedgerBody:
        '第一次成功备份写入 manifest 和工件轨迹后，这里才会出现审计记录。',
      noRunsTitle: '还没有记录任何备份运行',
      noRunsBody:
        '只要完成一次手动备份并写入 manifest 链，审计账本就会开始出现内容。',
      runManualBackup: '运行手动备份',
      manifestChain: 'Manifest 链',
      verifyIntegrity: '验证完整性',
      loadingRunDetail: '正在加载运行详情',
      runDetailUnavailable: '运行详情不可用',
      manifestDetail: '运行 #{runId} · Manifest 详情',
      runId: '运行 ID',
      runType: '类型',
      runSource: '来源',
      executedAt: '执行时间',
      manifestHash: 'Manifest 哈希',
      manifestPath: 'Manifest 路径',
      manualBackup: '手动备份',
      scheduledBackup: '定时备份',
      archiveWide: '整个归档',
      newVisits: '新增访问',
      newUrls: '新增 URL',
      downloads: '下载',
      profiles: '配置文件',
      artifacts: '工件 · {count} 个文件',
      warningsTitle: '警告',
      viewManifest: '查看 Manifest',
      copyPath: '复制路径',
      copied: '已复制',
      detailEmptyTitle: '尚未选择审计运行',
      detailEmptyBody:
        '点击上方 Manifest 链中的任意区块，即可检查运行详情、工件与哈希链。',
      repairRoutesTitle: '信任修复',
      repairRoutesBody:
        '在下一次高风险操作前，先到对应页面检查导入、计划状态或加密设置。',
      repairImports: '导入检查',
      repairSchedule: '计划排障',
      repairSecurity: '安全检查',
    },
    import: {
      archiveNotInitialized: '归档尚未初始化',
      archiveNotInitializedBody: '请先初始化归档，再导入外部历史数据。',
      trustTitle: '可信导入流程',
      trustBody:
        '先预览，把不支持的文件继续隔离，并在写入前看清楚每条回滚路径。',
      takeoutMethodTitle: 'Google Takeout',
      takeoutMethodBody: '从导出的归档导入',
      browserMethodTitle: '浏览器直接导入',
      browserMethodBody: '从本地浏览器数据库导入',
      stepUpload: '上传',
      stepScan: '扫描',
      stepPreview: '预览',
      stepConfirm: '确认',
      stepImport: '导入',
      selectTitle: '步骤 1：选择来源',
      takeoutSelectBody: '提供 Google Takeout 导出文件的路径（zip 或文件夹）。',
      browserSelectBody: '提供浏览器 History 数据库文件的路径。',
      sourcePath: '来源路径',
      takeoutPathPlaceholder: '/path/to/takeout.zip',
      browserPathPlaceholder: '/path/to/History',
      scanSource: '扫描来源 →',
      scanningTitle: '步骤 2：正在扫描...',
      scanningBody: '正在检查来源文件中的已识别历史格式。',
      previewTitle: '步骤 3：预览导入',
      previewBody: '在确认前先检查会写入哪些内容。',
      recordsFound: '发现的记录',
      duplicates: '重复项',
      newRecords: '新记录',
      detectedFiles: '已检测文件',
      quarantinedFiles: '隔离文件',
      backAction: '← 返回',
      confirmImport: '确认导入 →',
      importingTitle: '步骤 4：正在导入...',
      importingBody: '正在把记录写入归档，可能需要一点时间。',
      completeTitle: '步骤 5：导入完成',
      completeBody: '记录已经写入归档。',
      imported: '已导入',
      duplicatesSkipped: '已跳过重复项',
      importAnother: '继续导入其他来源',
      workflowLabel: '导入 PME 流程',
      workflowPreviewTitle: '预览边界',
      workflowPreviewSummary: '在写入发生前检查已识别文件、预览行与隔离备注。',
      workflowPreviewReason:
        '预览能说明范围、重复风险，以及哪些未支持文件会继续隔离。',
      workflowManualTitle: '手动检查',
      workflowManualSummary:
        '你也可以自己走一遍同样的流程，并在应用外检查来源归档。',
      workflowManualReason: '手动检查能在执行前明确隐私影响与文件来源。',
      workflowExecuteTitle: '执行导入',
      workflowExecuteSummary:
        '只有在预览和手动检查都看起来正确后，才导入已识别记录。',
      workflowExecuteReason:
        '执行会把新的导入批次写入归档，所以必须是一个明确决定。',
      workflowVerifyTitle: '验证结果',
      workflowVerifySummary: '导入后立即检查批次详情、预览行和回滚控制。',
      workflowVerifyReason:
        '验证能让回滚和恢复入口保持在手边，尤其是批次看起来可疑的时候。',
      workflowFinishTitle: '把回滚留在手边',
      workflowFinishSummary:
        '只有在审计路径、回滚按钮和可见行计数都正确时才离开这个流程。',
      workflowFinishReason:
        '导入可信来自可见的回滚故事，而不是一个简单的成功提示。',
      manualLocateStep: '定位导出文件或解压后的文件夹。',
      manualInspectStep: '检查文件内容，并与已识别列表逐项对照。',
      manualContinueStep: '只有在预览看起来正确后，再回来继续下一步。',
      recentBatches: '最近导入批次',
      recentBatchesBody: '每次导入都保留可复查、可回滚、可恢复的本地审计状态。',
      noImportBatches: '还没有记录任何导入批次。',
      selectedBatch: '已选批次',
      selectedBatchBody: '选择一个批次来检查预览行、备注与回滚控制。',
      previewRows: '预览行',
      noPreviewRows: '当前选择还没有可显示的预览行。',
      candidateRows: '候选行',
      importedRows: '已导入行',
      duplicateRows: '重复行',
      visibleRows: '当前可见行',
      auditPath: '审计路径',
      revertBatch: '回滚批次',
      restoreBatch: '恢复批次',
      revertConfirm:
        '要把这个导入批次从实时归档中回滚吗？原始审计轨迹仍会保留。',
      restoreConfirm: '要把这个已回滚的导入批次重新恢复到实时归档吗？',
      healthReport: 'Doctor 报告',
      healthReportBody:
        '在导入、回滚或恢复后运行 doctor，确认衍生状态和工件都还一致。',
      noHealthChecks: '这个视图里还没有运行过 doctor。',
      repairSummary:
        '修复清理了 {derivedRows} 条衍生数据，恢复了 {visibilityRows} 条可见性引用，并重建了 {importAudits} 个导入审计工件。',
    },
    schedule: {
      loadingPreview: '正在渲染原生计划预览',
      unavailableTitle: '计划预览不可用',
      unavailableBody: 'PathKeep 无法渲染原生计划工件。',
      backupSchedule: '备份计划',
      installState: '安装状态',
      interval: '间隔',
      verification: '校验频率',
      mechanism: '机制',
      lastTriggered: '上次触发',
      label: '标签',
      profiles: '配置文件',
      pmeTitle: '计划 PME',
      previewBoundary: '预览边界',
      previewBody: '在信任原生自动化前，先检查调度器工件、安装计划和回滚命令。',
      noGeneratedFiles:
        '浏览器预览模式不会生成真实文件。请打开桌面版检查完整的原生工件。',
      openLatestAudit: '打开最近一次计划审计',
      executeRun: '执行',
      executeBody:
        '只有在预览和警告都看起来正确后，才安装或移除当前的原生计划。',
      applyCommand: '应用命令 {index}',
      rollbackCommand: '回滚命令 {index}',
      applySchedule: '应用计划',
      removeSchedule: '移除计划',
      openSchedulerAudit: '打开计划审计',
      initializeArchiveFirst:
        '请先初始化归档，再回来应用已经审查过的原生计划。',
      installedDescription: '原生计划文件与当前的 PathKeep 方案一致。',
      mismatchDescription: '已安装文件存在，但已与当前预览不一致。',
      permissionWarningDescription:
        'PathKeep 无法在这台机器上完整检查已安装文件。',
      legacyInstallDescription:
        '检测到旧版安装残留，在信任这份计划前请先检查。',
      manualReviewDescription: '该平台在 v1 中保持手动优先，请按文档步骤核对。',
      notInstalledDescription: '尚未检测到已安装的原生计划。',
      installedBadge: '已安装',
      attentionBadge: '需要关注',
      manualReviewBadge: '手动检查',
      notInstalledBadge: '未安装',
    },
    security: {
      loadingPosture: '正在加载安全状态',
      unavailableTitle: '安全状态不可用',
      unavailableBody:
        'PathKeep 需要本地应用快照后，才能描述当前的加密和钥匙串状态。',
      initFirstAction: '先初始化归档',
      notInitializedTitle: '归档尚未初始化',
      notInitializedBody:
        '只有在初始化创建归档并写入第一条基线 manifest 后，安全检查才有实际意义。',
      encryptionStatus: '加密状态',
      archiveIs: '归档当前为 {mode}',
      encryptedDetail: '静态数据使用 SQLCipher · 读访问前需要解锁',
      plaintextDetail: '标准 SQLite 归档 · 磁盘加密取决于宿主系统',
      keyring: '钥匙串',
      sessionStatus: '会话状态',
      sessionUnlocked: '归档当前已解锁',
      sessionLocked: '归档已锁定，Explorer 和 Audit 仍然保持只读阻挡',
      lastBackup: '上次备份',
      stronghold: 'Stronghold',
      archivePath: '归档路径',
      passwordLossTitle: '密码丢失 = 数据丢失。',
      passwordLossBody:
        'PathKeep 没有恢复后门。重新加密前，请把当前或新的数据库密钥保存在安全位置。',
      unlockKeyringTitle: '解锁 + 钥匙串',
      sessionActive: '会话激活中',
      needsUnlock: '需要解锁',
      currentDatabaseKey: '当前数据库密钥',
      currentDatabaseKeyPlaceholder: '输入当前归档密钥',
      unlockArchive: '解锁归档',
      useKeyring: '使用钥匙串',
      lockArchive: '锁定归档',
      storeInKeyring: '保存到钥匙串',
      clearKeyring: '清除钥匙串',
      keyringConvenience:
        '把密钥存进原生钥匙串只是一个可选的便利解锁路径。PathKeep 仍然保持 local-first，不会把秘密上传到任何地方。',
      rekeyTitle: '重新加密预览',
      previewBeforeExecute: '执行前先预览',
      targetMode: '目标模式',
      newDatabaseKey: '新的数据库密钥',
      newDatabaseKeyPlaceholder: '输入替换后的归档密钥',
      storeNewKey: '执行后把新的数据库密钥保存到原生钥匙串',
      previewRekey: '预览重新加密',
      executeRekey: '执行重新加密',
      mode: '模式',
      snapshot: '快照',
      temporaryDatabase: '临时数据库',
    },
    settings: {
      loadingSettings: '正在加载设置',
      loadingModules: '正在加载设置模块',
      browserProfiles: '浏览器配置文件',
      aiProvider: 'AI 提供方',
      optional: '可选',
      general: '通用',
      baseUrlLabel: 'Base URL',
      embeddingModelLabel: 'Embedding Model',
      llmModelLabel: 'LLM Model',
      apiKeyLabel: 'API Key',
      interfaceLanguage: '界面语言',
      currentLanguage: '当前语言',
      dataDirectory: '数据目录',
      openDirectory: '在文件管理器中打开',
      mcpServer: 'MCP 服务',
      version: '版本',
      enabled: '已启用',
      disabled: '已禁用',
      historyFound: '已找到历史',
      noHistoryDetected: '未检测到历史文件',
      platformTroubleshooting: '平台排障',
      platformBody:
        '把计划安装路径、Safari 权限规则和钥匙串限制都保留在设置页可见。',
      reviewSchedule: '打开计划',
      reviewSecurity: '打开安全',
      reviewImports: '打开导入',
    },
    platform: {
      macosLabel: 'macOS LaunchAgent',
      windowsLabel: 'Windows 任务计划程序',
      linuxLabel: 'Linux systemd 用户定时器',
      macosSummary:
        '在桌面版可用时，macOS 可以直接从 PathKeep 安装和检查 LaunchAgent。',
      windowsSummary:
        'Windows 使用任务计划程序。应用前先预览 XML、保留手动导入路径，并确认 StartWhenAvailable 行为。',
      linuxSummary:
        'Linux 使用 systemd 用户定时器，要求 OnCalendar + Persistent。即使将来支持 apply，手动检查仍然是一等路径。',
      safariAccessTitle: 'Safari 仍需要 Full Disk Access',
      safariAccessBody:
        '不要把 Safari 配置文件静默隐藏掉，而是保留在 UI 中并明确引导用户授予 Full Disk Access。',
      keyringTitle: '原生钥匙串不可用',
      keyringBody:
        '仍然允许加密模式，但在机器提供受支持的钥匙串后端前，自动解锁和记住密钥流程都不可用。',
      schedulerMismatchTitle: '已安装的计划需要复查',
      schedulerMismatchBody:
        '遇到 mismatch、旧版残留或权限警告时，应把用户送回 Preview → Manual → Execute，而不是假装计划仍然健康。',
    },
  },
  'zh-TW': {
    common: {
      followSystem: '跟隨系統',
      english: 'English',
      simplifiedChinese: '简体中文',
      traditionalChinese: '繁體中文',
      openAction: '開啟',
      copyAction: '複製',
      openPath: '開啟路徑',
      copiedNotice: '已複製到剪貼簿。',
      loading: '載入中',
      unavailable: '不可用',
      notAvailable: '不可用',
      pending: '待處理',
      current: '目前',
      complete: '已完成',
      warning: '警告',
      yes: '是',
      no: '否',
      reviewAction: '檢查',
      rescanAction: '重新掃描',
      runDoctorAction: '執行檢查',
      repairAction: '修復',
      previewTab: '預覽',
      manualTab: '手動',
      executeTab: '執行',
      verifyTab: '驗證',
      whyThisStepMatters: '為什麼需要這一步',
      filesLabel: '工件',
      commandsLabel: '命令',
      checklistLabel: '檢查清單',
    },
    shell: {
      savingArchiveChoices: '正在儲存封存選項',
      preparingArchive: '正在準備封存',
      runningManualBackup: '正在執行手動備份',
      loadingLatestArchiveState: 'PathKeep 無法載入最新的封存狀態。',
      savingSettingsFailed: 'PathKeep 無法儲存更新後的設定。',
      initializeArchiveFailed: 'PathKeep 無法初始化封存。',
      initializedNotice: '封存已初始化。請在開啟自動化前先檢查第一次備份。',
      manualBackupDueWindow: '封存仍處於未到期窗口內。',
      manualBackupFinished: '手動備份已完成，執行編號 #{runId}。',
      manualBackupFailed: 'PathKeep 無法完成手動備份。',
    },
    navigation: {
      coreSection: '核心',
      operationsSection: '操作',
      systemSection: '系統',
      dashboardLabel: '儀表板',
      dashboardTitle: '儀表板',
      dashboardSubtitle: '封存概覽與系統狀態',
      explorerLabel: '瀏覽器',
      explorerTitle: '歷史瀏覽器',
      explorerSubtitle: '瀏覽、搜尋並篩選封存',
      insightsLabel: '洞察',
      insightsTitle: '洞察',
      insightsSubtitle: '主題、線索與瀏覽模式',
      assistantLabel: 'AI 助手',
      assistantTitle: 'AI 助手',
      assistantSubtitle: '就你的瀏覽歷史提問',
      assistantBadge: '可選',
      importLabel: '匯入',
      importTitle: '匯入',
      importSubtitle: 'Google Takeout 與瀏覽器直接匯入',
      auditLabel: '審計帳本',
      auditTitle: '審計帳本',
      auditSubtitle: 'Manifest 鏈、執行歷史與完整性',
      scheduleLabel: '排程',
      scheduleTitle: '排程',
      scheduleSubtitle: '備份排程與安裝工件',
      securityLabel: '安全',
      securityTitle: '安全',
      securitySubtitle: '加密、鑰匙圈與密碼管理',
      settingsLabel: '設定',
      settingsTitle: '設定',
      settingsSubtitle: '設定檔、語言與平台指引',
      onboardingLabel: '初始化',
      onboardingTitle: '初始化 / 設定',
      onboardingSubtitle: '首輪封存決策的預覽、手動指引與執行',
      archiveAttentionNeeded: '封存需要關注',
      archiveHealthy: '封存健康',
      archiveNotInitialized: '封存尚未初始化',
      encryptedArchive: '加密封存',
      plaintextArchive: '明文封存',
      loadingBuild: '正在載入版本資訊',
      expandNavigation: '展開導覽',
      collapseNavigation: '收起導覽',
      toggleTheme: '切換主題',
      searchHistory: '搜尋歷史',
      searchHistoryPlaceholder: '搜尋歷史...  ⌘K',
      backupNow: '立即備份',
      initializeFirst: '請先初始化',
    },
    dashboard: {
      loadingOverview: '正在載入封存概覽',
      archiveReadError: '儀表板無法讀取封存',
      archiveUnavailable: '儀表板資料不可用',
      archiveUnavailableBody: 'PathKeep 無法載入目前的封存快照。',
      totalRecords: '總記錄數',
      uniqueUrls: '{count} 個唯一 URL',
      lastBackup: '上次備份',
      noManifestYet: '鏈中還沒有寫入 manifest',
      profilesInScope: '封存範圍內的設定檔',
      profilesReadableAttention: '{readable} 個可讀 · {attention} 個需要關注',
      archiveMode: '封存模式',
      archiveUnlocked: '目前工作階段中的封存已解鎖',
      archiveNeedsUnlock: '在 Explorer / Audit 讀取前仍需要先解鎖封存',
      zeroStateTitle: '第一次封存執行仍需要檢查',
      zeroStateBody:
        '只有在封存初始化並完成第一次手動備份後，儀表板卡片才會顯示實際資料。',
      zeroStateEyebrow: '首日',
      openOnboardingFlow: '開啟初始化流程',
      recentRuns: '最近執行',
      fullLedger: '完整帳本 →',
      run: '執行',
      type: '類型',
      source: '來源',
      records: '記錄',
      status: '狀態',
      time: '時間',
      backupType: '備份',
      profilesLabel: '{count} 個設定檔',
      archiveBoundary: '封存邊界',
      selectedProfiles: '已選擇 {count} 個設定檔',
      historyDetected: '已偵測到歷史資料庫',
      historyMissing: '未找到歷史資料庫',
      trustActions: '信任動作',
      trustActionsBody:
        '在下一次寫入前，先把權限、排程狀態與回滾入口都檢查一遍。',
      reviewImportBatches: '檢查匯入批次',
      reviewSecurity: '檢查安全狀態',
      reviewSchedule: '檢查排程狀態',
      storageFootprint: '儲存占用',
      storageTotal: '總計 {size}',
      archiveDatabase: '封存資料庫',
      manifests: 'Manifest',
      snapshots: '快照',
      exports: '匯出',
    },
    audit: {
      loadingLedger: '正在載入審計帳本',
      unavailableTitle: '審計帳本不可用',
      finishOnboarding: '完成初始化',
      emptyLedgerTitle: '審計帳本裡還沒有封存執行',
      emptyLedgerBody:
        '第一次成功備份寫入 manifest 和工件軌跡後，這裡才會出現審計記錄。',
      noRunsTitle: '還沒有記錄任何備份執行',
      noRunsBody:
        '只要完成一次手動備份並寫入 manifest 鏈，審計帳本就會開始顯示內容。',
      runManualBackup: '執行手動備份',
      manifestChain: 'Manifest 鏈',
      verifyIntegrity: '驗證完整性',
      loadingRunDetail: '正在載入執行詳情',
      runDetailUnavailable: '執行詳情不可用',
      manifestDetail: '執行 #{runId} · Manifest 詳情',
      runId: '執行 ID',
      runType: '類型',
      runSource: '來源',
      executedAt: '執行時間',
      manifestHash: 'Manifest 雜湊',
      manifestPath: 'Manifest 路徑',
      manualBackup: '手動備份',
      scheduledBackup: '排程備份',
      archiveWide: '整個封存',
      newVisits: '新增造訪',
      newUrls: '新增 URL',
      downloads: '下載',
      profiles: '設定檔',
      artifacts: '工件 · {count} 個檔案',
      warningsTitle: '警告',
      viewManifest: '查看 Manifest',
      copyPath: '複製路徑',
      copied: '已複製',
      detailEmptyTitle: '尚未選擇審計執行',
      detailEmptyBody:
        '點擊上方 Manifest 鏈中的任一區塊，即可檢查執行詳情、工件與雜湊鏈。',
      repairRoutesTitle: '信任修復',
      repairRoutesBody:
        '在下一次高風險操作前，先到對應頁面檢查匯入、排程狀態或加密設定。',
      repairImports: '匯入檢查',
      repairSchedule: '排程排障',
      repairSecurity: '安全檢查',
    },
    import: {
      archiveNotInitialized: '封存尚未初始化',
      archiveNotInitializedBody: '請先初始化封存，再匯入外部歷史資料。',
      trustTitle: '可信匯入流程',
      trustBody:
        '先預覽，把不支援的檔案繼續隔離，並在寫入前看清楚每條回滾路徑。',
      takeoutMethodTitle: 'Google Takeout',
      takeoutMethodBody: '從匯出的封存檔匯入',
      browserMethodTitle: '瀏覽器直接匯入',
      browserMethodBody: '從本機瀏覽器資料庫匯入',
      stepUpload: '上傳',
      stepScan: '掃描',
      stepPreview: '預覽',
      stepConfirm: '確認',
      stepImport: '匯入',
      selectTitle: '步驟 1：選擇來源',
      takeoutSelectBody: '提供 Google Takeout 匯出檔的路徑（zip 或資料夾）。',
      browserSelectBody: '提供瀏覽器 History 資料庫檔案的路徑。',
      sourcePath: '來源路徑',
      takeoutPathPlaceholder: '/path/to/takeout.zip',
      browserPathPlaceholder: '/path/to/History',
      scanSource: '掃描來源 →',
      scanningTitle: '步驟 2：正在掃描...',
      scanningBody: '正在檢查來源檔案中的已識別歷史格式。',
      previewTitle: '步驟 3：預覽匯入',
      previewBody: '在確認前先檢查會寫入哪些內容。',
      recordsFound: '發現的記錄',
      duplicates: '重複項',
      newRecords: '新記錄',
      detectedFiles: '已檢測檔案',
      quarantinedFiles: '隔離檔案',
      backAction: '← 返回',
      confirmImport: '確認匯入 →',
      importingTitle: '步驟 4：正在匯入...',
      importingBody: '正在把記錄寫入封存，可能需要一點時間。',
      completeTitle: '步驟 5：匯入完成',
      completeBody: '記錄已經寫入封存。',
      imported: '已匯入',
      duplicatesSkipped: '已跳過重複項',
      importAnother: '繼續匯入其他來源',
      workflowLabel: '匯入 PME 流程',
      workflowPreviewTitle: '預覽邊界',
      workflowPreviewSummary: '在寫入發生前檢查已識別檔案、預覽列與隔離備註。',
      workflowPreviewReason:
        '預覽能說明範圍、重複風險，以及哪些未支援檔案會繼續隔離。',
      workflowManualTitle: '手動檢查',
      workflowManualSummary:
        '你也可以自己走一遍同樣的流程，並在應用外檢查來源封存。',
      workflowManualReason: '手動檢查能在執行前明確隱私影響與檔案來源。',
      workflowExecuteTitle: '執行匯入',
      workflowExecuteSummary:
        '只有在預覽和手動檢查都看起來正確後，才匯入已識別記錄。',
      workflowExecuteReason:
        '執行會把新的匯入批次寫入封存，所以必須是一個明確決定。',
      workflowVerifyTitle: '驗證結果',
      workflowVerifySummary: '匯入後立即檢查批次詳情、預覽列和回滾控制。',
      workflowVerifyReason:
        '驗證能讓回滾和恢復入口保持在手邊，尤其是批次看起來可疑的時候。',
      workflowFinishTitle: '把回滾留在手邊',
      workflowFinishSummary:
        '只有在審計路徑、回滾按鈕和可見列計數都正確時才離開這個流程。',
      workflowFinishReason:
        '匯入可信來自可見的回滾故事，而不是一個簡單的成功提示。',
      manualLocateStep: '定位匯出檔或解壓後的資料夾。',
      manualInspectStep: '檢查檔案內容，並與已識別清單逐項對照。',
      manualContinueStep: '只有在預覽看起來正確後，再回來繼續下一步。',
      recentBatches: '最近匯入批次',
      recentBatchesBody: '每次匯入都保留可複查、可回滾、可恢復的本地審計狀態。',
      noImportBatches: '還沒有記錄任何匯入批次。',
      selectedBatch: '已選批次',
      selectedBatchBody: '選擇一個批次來檢查預覽列、備註與回滾控制。',
      previewRows: '預覽列',
      noPreviewRows: '目前選擇還沒有可顯示的預覽列。',
      candidateRows: '候選列',
      importedRows: '已匯入列',
      duplicateRows: '重複列',
      visibleRows: '目前可見列',
      auditPath: '審計路徑',
      revertBatch: '回滾批次',
      restoreBatch: '恢復批次',
      revertConfirm:
        '要把這個匯入批次從即時封存中回滾嗎？原始審計軌跡仍會保留。',
      restoreConfirm: '要把這個已回滾的匯入批次重新恢復到即時封存嗎？',
      healthReport: 'Doctor 報告',
      healthReportBody:
        '在匯入、回滾或恢復後執行 doctor，確認衍生狀態和工件都還一致。',
      noHealthChecks: '這個檢視裡還沒有執行過 doctor。',
      repairSummary:
        '修復清理了 {derivedRows} 筆衍生資料，恢復了 {visibilityRows} 筆可見性引用，並重建了 {importAudits} 個匯入審計工件。',
    },
    schedule: {
      loadingPreview: '正在渲染原生排程預覽',
      unavailableTitle: '排程預覽不可用',
      unavailableBody: 'PathKeep 無法渲染原生排程工件。',
      backupSchedule: '備份排程',
      installState: '安裝狀態',
      interval: '間隔',
      verification: '檢查頻率',
      mechanism: '機制',
      lastTriggered: '上次觸發',
      label: '標籤',
      profiles: '設定檔',
      pmeTitle: '排程 PME',
      previewBoundary: '預覽邊界',
      previewBody: '在信任原生自動化前，先檢查排程器工件、安裝計畫和回滾命令。',
      noGeneratedFiles:
        '瀏覽器預覽模式不會生成真實檔案。請開啟桌面版檢查完整的原生工件。',
      openLatestAudit: '開啟最近一次排程審計',
      executeRun: '執行',
      executeBody:
        '只有在預覽和警告都看起來正確後，才安裝或移除目前的原生排程。',
      applyCommand: '套用命令 {index}',
      rollbackCommand: '回滾命令 {index}',
      applySchedule: '套用排程',
      removeSchedule: '移除排程',
      openSchedulerAudit: '開啟排程審計',
      initializeArchiveFirst: '請先初始化封存，再回來套用已審查過的原生排程。',
      installedDescription: '原生排程檔案與目前的 PathKeep 方案一致。',
      mismatchDescription: '已安裝檔案存在，但已與目前的預覽不一致。',
      permissionWarningDescription:
        'PathKeep 無法在這台機器上完整檢查已安裝檔案。',
      legacyInstallDescription:
        '偵測到舊版安裝殘留，在信任這份排程前請先檢查。',
      manualReviewDescription: '該平台在 v1 中保持手動優先，請依文檔步驟核對。',
      notInstalledDescription: '尚未偵測到已安裝的原生排程。',
      installedBadge: '已安裝',
      attentionBadge: '需要關注',
      manualReviewBadge: '手動檢查',
      notInstalledBadge: '未安裝',
    },
    security: {
      loadingPosture: '正在載入安全狀態',
      unavailableTitle: '安全狀態不可用',
      unavailableBody:
        'PathKeep 需要本地應用快照後，才能描述目前的加密和鑰匙圈狀態。',
      initFirstAction: '先初始化封存',
      notInitializedTitle: '封存尚未初始化',
      notInitializedBody:
        '只有在初始化建立封存並寫入第一條基線 manifest 後，安全檢查才有實際意義。',
      encryptionStatus: '加密狀態',
      archiveIs: '封存目前為 {mode}',
      encryptedDetail: '靜態資料使用 SQLCipher · 讀取前需要解鎖',
      plaintextDetail: '標準 SQLite 封存 · 磁碟加密取決於宿主系統',
      keyring: '鑰匙圈',
      sessionStatus: '工作階段狀態',
      sessionUnlocked: '封存目前已解鎖',
      sessionLocked: '封存已鎖定，Explorer 和 Audit 仍然保持只讀阻擋',
      lastBackup: '上次備份',
      stronghold: 'Stronghold',
      archivePath: '封存路徑',
      passwordLossTitle: '密碼遺失 = 資料遺失。',
      passwordLossBody:
        'PathKeep 沒有恢復後門。重新加密前，請把目前或新的資料庫金鑰保存在安全位置。',
      unlockKeyringTitle: '解鎖 + 鑰匙圈',
      sessionActive: '工作階段啟用中',
      needsUnlock: '需要解鎖',
      currentDatabaseKey: '目前資料庫金鑰',
      currentDatabaseKeyPlaceholder: '輸入目前的封存金鑰',
      unlockArchive: '解鎖封存',
      useKeyring: '使用鑰匙圈',
      lockArchive: '鎖定封存',
      storeInKeyring: '儲存到鑰匙圈',
      clearKeyring: '清除鑰匙圈',
      keyringConvenience:
        '把金鑰存進原生鑰匙圈只是可選的便利解鎖路徑。PathKeep 仍然保持 local-first，不會把秘密上傳到任何地方。',
      rekeyTitle: '重新加密預覽',
      previewBeforeExecute: '執行前先預覽',
      targetMode: '目標模式',
      newDatabaseKey: '新的資料庫金鑰',
      newDatabaseKeyPlaceholder: '輸入替換後的封存金鑰',
      storeNewKey: '執行後把新的資料庫金鑰儲存到原生鑰匙圈',
      previewRekey: '預覽重新加密',
      executeRekey: '執行重新加密',
      mode: '模式',
      snapshot: '快照',
      temporaryDatabase: '暫存資料庫',
    },
    settings: {
      loadingSettings: '正在載入設定',
      loadingModules: '正在載入設定模組',
      browserProfiles: '瀏覽器設定檔',
      aiProvider: 'AI 提供者',
      optional: '可選',
      general: '通用',
      baseUrlLabel: 'Base URL',
      embeddingModelLabel: 'Embedding Model',
      llmModelLabel: 'LLM Model',
      apiKeyLabel: 'API Key',
      interfaceLanguage: '介面語言',
      currentLanguage: '目前語言',
      dataDirectory: '資料目錄',
      openDirectory: '在檔案管理器中開啟',
      mcpServer: 'MCP 服務',
      version: '版本',
      enabled: '已啟用',
      disabled: '已停用',
      historyFound: '已找到歷史',
      noHistoryDetected: '未偵測到歷史檔案',
      platformTroubleshooting: '平台排障',
      platformBody:
        '把排程安裝路徑、Safari 權限規則和鑰匙圈限制都保留在設定頁可見。',
      reviewSchedule: '開啟排程',
      reviewSecurity: '開啟安全',
      reviewImports: '開啟匯入',
    },
    platform: {
      macosLabel: 'macOS LaunchAgent',
      windowsLabel: 'Windows 工作排程器',
      linuxLabel: 'Linux systemd 使用者計時器',
      macosSummary:
        '在桌面版可用時，macOS 可以直接從 PathKeep 安裝和檢查 LaunchAgent。',
      windowsSummary:
        'Windows 使用工作排程器。套用前先預覽 XML、保留手動匯入路徑，並確認 StartWhenAvailable 行為。',
      linuxSummary:
        'Linux 使用 systemd 使用者計時器，要求 OnCalendar + Persistent。即使未來支援 apply，手動檢查仍然是一等路徑。',
      safariAccessTitle: 'Safari 仍需要 Full Disk Access',
      safariAccessBody:
        '不要把 Safari 設定檔靜默隱藏，而是保留在 UI 中並明確引導使用者授予 Full Disk Access。',
      keyringTitle: '原生鑰匙圈不可用',
      keyringBody:
        '仍然允許加密模式，但在機器提供受支援的鑰匙圈後端前，自動解鎖和記住金鑰流程都不可用。',
      schedulerMismatchTitle: '已安裝的排程需要複查',
      schedulerMismatchBody:
        '遇到 mismatch、舊版殘留或權限警告時，應把使用者送回 Preview → Manual → Execute，而不是假裝排程仍然健康。',
    },
  },
}

function isRecord(
  value: string | TranslationDictionary | undefined,
): value is TranslationDictionary {
  return typeof value === 'object' && value !== null
}

function getValue(
  dictionary: TranslationDictionary,
  path: string[],
): string | null {
  let current: string | TranslationDictionary | undefined = dictionary

  for (const segment of path) {
    if (!isRecord(current)) {
      return null
    }
    current = current[segment]
  }

  return typeof current === 'string' ? current : null
}

function flattenDictionary(
  dictionary: TranslationDictionary,
  prefix = '',
  target: Record<string, string> = {},
) {
  for (const [key, value] of Object.entries(dictionary)) {
    const nextKey = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') {
      target[nextKey] = value
      target[key] ??= value
      continue
    }

    flattenDictionary(value, nextKey, target)
  }

  return target
}

const flattenedCatalog = Object.fromEntries(
  supportedLanguages.map((language) => [
    language,
    flattenDictionary(catalog[language]),
  ]),
) as Record<ResolvedLanguage, Record<string, string>>

function interpolate(
  value: string,
  vars?: Record<string, string | number | null | undefined>,
) {
  if (!vars) {
    return value
  }

  return value.replaceAll(/\{([^}]+)\}/g, (match, key: string) => {
    const next = vars[key]
    return next === null || next === undefined ? match : String(next)
  })
}

export function translationCatalog() {
  return structuredClone(catalog)
}

export function listTranslationKeys(language: ResolvedLanguage = 'en') {
  return Object.keys(flattenedCatalog[language]).sort()
}

export function pseudoLocalize(value: string) {
  const accentMap: Record<string, string> = {
    a: 'á',
    e: 'ë',
    i: 'ï',
    o: 'ô',
    u: 'ü',
    A: 'Á',
    E: 'Ë',
    I: 'Ï',
    O: 'Ô',
    U: 'Ü',
  }

  const placeholders: string[] = []
  const withPlaceholdersPreserved = value.replaceAll(/\{[^}]+\}/g, (match) => {
    const token = `@@__${placeholders.length}__@@`
    placeholders.push(match)
    return token
  })

  const expanded = withPlaceholdersPreserved
    .split('')
    .map((char) => accentMap[char] ?? char)
    .join('')

  const restored = expanded.replaceAll(/@@__(\d+)__@@/g, (_, index: string) => {
    return placeholders[Number(index)] ?? ''
  })

  return `［${restored}］`
}

export function detectSystemLanguage(
  languages?: readonly string[],
): ResolvedLanguage {
  const preferred =
    languages ??
    (typeof navigator !== 'undefined'
      ? navigator.languages?.length
        ? navigator.languages
        : [navigator.language]
      : [])

  for (const locale of preferred) {
    const normalized = locale.toLowerCase()
    if (normalized.startsWith('zh')) {
      if (
        normalized.includes('tw') ||
        normalized.includes('hk') ||
        normalized.includes('mo') ||
        normalized.includes('hant')
      ) {
        return 'zh-TW'
      }
      return 'zh-CN'
    }

    if (normalized.startsWith('en')) {
      return 'en'
    }
  }

  return 'en'
}

export function resolveLanguage(
  preference?: LanguagePreference,
  languages?: readonly string[],
): ResolvedLanguage {
  if (!preference || preference === 'system') {
    return detectSystemLanguage(languages)
  }

  return supportedLanguages.includes(preference) ? preference : 'en'
}

export type TranslationKey = string

export function createTranslator(language: ResolvedLanguage, pseudo = false) {
  const dictionary = flattenedCatalog[language] ?? flattenedCatalog.en
  const fallback = flattenedCatalog.en

  return (key: TranslationKey, vars?: Record<string, string | number>) => {
    const value = dictionary[key] ?? fallback[key] ?? key
    const translated = interpolate(value, vars)
    return pseudo ? pseudoLocalize(translated) : translated
  }
}

export function createNamespaceTranslator(
  language: ResolvedLanguage,
  namespace: TranslationNamespace,
  pseudo = false,
) {
  const translate = createTranslator(language, pseudo)
  return (key: string, vars?: Record<string, string | number>) =>
    translate(`${namespace}.${key}`, vars)
}

export function languageLabel(
  preference: LanguagePreference,
  uiLanguage: ResolvedLanguage,
) {
  const translate = createTranslator(uiLanguage)
  if (preference === 'system') {
    return translate('common.followSystem')
  }
  if (preference === 'zh-CN') {
    return translate('common.simplifiedChinese')
  }
  if (preference === 'zh-TW') {
    return translate('common.traditionalChinese')
  }
  return translate('common.english')
}

export function localeTag(language: ResolvedLanguage) {
  if (language === 'zh-CN') return 'zh-CN'
  if (language === 'zh-TW') return 'zh-TW'
  return 'en-US'
}

export function resolveTranslation(
  language: ResolvedLanguage,
  key: string,
  vars?: Record<string, string | number>,
) {
  const segments = key.split('.')
  const namespaced =
    segments.length > 1
      ? (getValue(catalog[language], segments) ??
        getValue(catalog.en, segments))
      : null

  const value =
    namespaced ??
    flattenedCatalog[language][key] ??
    flattenedCatalog.en[key] ??
    key
  return interpolate(value, vars)
}
