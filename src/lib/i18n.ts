import type { LanguagePreference } from './types'

export type ResolvedLanguage = 'en' | 'zh-CN' | 'zh-TW'

// Stryker disable all: translation tables are static content; mutation focus should stay on the i18n logic below.
const english = {
  productName: 'browser history backup',
  localOnly: 'Local only',
  encrypted: 'Encrypted',
  plaintext: 'Plaintext',
  unlocked: 'Unlocked',
  locked: 'Locked',
  profilesDetected: '{count} profiles',
  dueEveryHours: 'Due every {hours}h',
  autoUnlockedNotice: 'Unlocked with the remembered database key.',
  initializedNotice:
    'Archive initialized. You can now review the schedule and run the first backup.',
  preferencesSavedNotice: 'Changes saved.',
  unlockSuccess: 'Encrypted archive unlocked for this session.',
  rotateSuccess:
    'Archive key rotated and the local Stronghold snapshot was rebuilt with the new password.',
  plaintextSuccess:
    'Archive switched to plaintext mode. The local Stronghold snapshot was cleared.',
  backupComplete: 'Backup completed.',
  backupCompleteWithManifest: 'Backup completed and wrote {path}.',
  schedulePreviewReady: 'Scheduler preview is ready.',
  takeoutDryRunNotice:
    'Takeout dry-run complete. Review the recognized and quarantined files.',
  takeoutImportNotice: 'Takeout import wrote {count} records.',
  revertBatchNotice:
    'The selected import batch was reverted. Live history rows were removed, and the raw audit trail was preserved.',
  doctorUpdated: 'Doctor checks updated.',
  rememberStored:
    'Current database key stored in the system keyring for unattended runs.',
  rememberCleared: 'Remembered database key cleared from the system keyring.',
  s3CredentialsStored: 'S3 credentials stored in the system keyring.',
  s3CredentialsCleared: 'S3 credentials cleared from the system keyring.',
  s3PreviewReady: 'Remote backup preview is ready.',
  copiedNotice: 'Copied to clipboard.',
  enterMasterPassword:
    'Enter the master password you stored in your password manager.',
  unlockBeforeRotate:
    'Unlock the archive first so the app can rotate the encryption key.',
  enterNewMasterPassword:
    'Enter a new master password before rotating the encrypted archive.',
  matchingPasswordsRequired:
    'Encrypted mode requires matching master passwords before initialization.',
  enterTakeoutPath:
    'Enter a Google Takeout zip path or an extracted folder path first.',
  generateSchedulePreviewFirst: 'Generate a schedule preview first.',
  unlockBeforeRemember:
    'Unlock the archive before saving the current key into the system keyring.',
  enterS3Credentials:
    'Enter the S3 access key ID and secret access key before saving credentials.',
  setupNav: 'Setup',
  explorerNav: 'Explorer',
  backupsNav: 'Backups',
  importNav: 'Import',
  settingsNav: 'Settings',
  setupTitle: 'Set up the local archive',
  setupDescription:
    'Work through the source, storage, schedule, and review steps. Every system action still exposes Preview, Manual, and Apply paths.',
  sourcesStep: '1. Sources',
  sourcesDescription:
    'Choose which browser profiles should be included in backups.',
  archiveStep: '2. Archive',
  archiveDescription:
    'Set retention cadence and archive behavior before you initialize or save changes.',
  scheduleStep: '3. Native schedule',
  scheduleDescription:
    'Preview the OS-native scheduler artifact, inspect the commands, then apply it only if you want the app to do it for you.',
  reviewStep: '4. Review',
  reviewDescription:
    'Confirm the local paths and the password-recovery implications before continuing.',
  storagePath: 'App data root',
  archiveDatabase: 'Archive database',
  auditRepository: 'Audit repository',
  archiveMode: 'Archive mode',
  dueAfterHours: 'Back up only when at least this many hours have passed',
  checkIntervalHours: 'Wake-up check interval (hours)',
  captureFavicons: 'Capture favicons alongside history snapshots',
  gitAudit:
    'Commit manifests and audit artifacts into the local git repository',
  rememberKey:
    'Remember the database key in the system keyring for unattended runs',
  masterPassword: 'Master password',
  confirmPassword: 'Confirm password',
  passwordPlaceholder: 'Store this in your password manager',
  createArchive: 'Create archive',
  saveSetup: 'Save setup',
  schedulePlatform: 'Target platform',
  previewSchedule: 'Preview native schedule',
  applyPreview: 'Apply preview',
  manualSteps: 'Manual steps',
  generatedFiles: 'Generated files',
  noSchedulePreview:
    'Generate a schedule preview to inspect the file contents, commands, and rollback path.',
  encryptionWarningTitle: 'Password recovery warning',
  encryptionWarningBody:
    'If you forget the encrypted archive password and do not have another valid unlock path, your encrypted data is effectively gone. Save the password in a password manager before continuing.',
  useSettingsForEncryption:
    'Use Settings to rotate the archive key or switch between encrypted and plaintext modes after initialization.',
  historyDetected: 'History database detected',
  historyMissing: 'History database not found',
  noSignedInUser: 'No signed-in user metadata',
  unknownBrowserVersion: 'Version unavailable',
  selectedProfilesSummary: 'Selected profiles',
  workflowGuide: 'Workflow guide',
  importWorkflowTitle: 'Review every import step before it changes the archive',
  reviewPlan: 'Review plan',
  manualPathTitle: 'Manual path',
  manualPathSummary:
    'Use the same workflow yourself. Every command is visible and copyable.',
  manualPathReason:
    'Manual mode lets you inspect each action before the app performs it automatically.',
  applyChanges: 'Apply changes',
  applyChangesSummary:
    'Run the prepared operation from the app when you are satisfied.',
  applyChangesReason:
    'Automatic mode still follows the same plan. It just executes the reviewed steps for you.',
  verifyOutcome: 'Verify outcome',
  verifyOutcomeSummary:
    'Confirm the result, inspect the audit output, and keep the rollback path nearby.',
  verifyOutcomeReason:
    'Verification reduces surprise and makes it obvious whether the system changed what you expected.',
  finishStep: 'Finish',
  finishSummary:
    'Close the workflow only after the audit path, rollback instructions, and visible results all look right.',
  finishReason:
    'A short finish checkpoint makes later audits and dirty-import recovery much easier.',
  automaticPath: 'Why this step matters',
  currentStep: 'Current',
  markStepComplete: 'Mark complete',
  stepCompleted: 'Completed',
  manualImportSummary:
    'You can inspect the Takeout archive yourself before any rows are imported.',
  manualImportReason:
    'Manual inspection helps you verify privacy impact, recognized files, and archive scope.',
  manualLocateStep: 'Locate the Takeout archive or extracted folder.',
  manualInspectStep:
    'Inspect the files and compare them against the recognized list.',
  manualContinueStep:
    'Return here and continue only when the preview looks correct.',
  applyImportReason:
    'Import only the recognized history records. Unsupported files stay quarantined and reviewable.',
  verifyImportReason:
    'After import, inspect the batch, preview rows, duplicates, and rollback controls.',
  whyThisStepMatters: 'Why this step matters',
  profileSelectionReason:
    'The app reads local browser history databases, stages a safe copy, and then ingests from that copy so your live browser data is never modified.',
  dataFilesRead: 'Data files we read',
  noFilesSelectedYet:
    'Select at least one browser profile to see the paths and manual commands.',
  manualAlternative: 'Manual alternative',
  explorerTitle: 'History explorer',
  explorerDescription:
    'Search the long-term archive, narrow by domain or profile, and export the current view when needed.',
  searchLabel: 'Search',
  searchPlaceholder: 'URL, title, or term',
  domainLabel: 'Domain',
  domainPlaceholder: 'example.com',
  profileLabel: 'Profile',
  allProfiles: 'All selected profiles',
  exportLabel: 'Export',
  noHistoryResults:
    'No matching rows yet. Try a broader search or run a backup.',
  unlockToSearch: 'Unlock or initialize the archive to search the timeline.',
  resultsCount: '{count} matching visits',
  selectedVisit: 'Selected visit',
  visitedAt: 'Visited at',
  titleLabel: 'Title',
  urlLabel: 'URL',
  provenance: 'Provenance',
  domain: 'Domain',
  profile: 'Profile',
  duration: 'Duration',
  transition: 'Transition',
  sourceVisitId: 'Source visit ID',
  appId: 'App ID',
  notAvailable: 'Not available',
  backupsTitle: 'Backups and audit ledger',
  backupsDescription:
    'Review backup runs, manifest hashes, what changed, and the most recent remote backup status.',
  runBackupNow: 'Run backup now',
  recentRuns: 'Recent runs',
  runDetails: 'Run details',
  noRuns:
    'No completed runs yet. Run the first backup to start the audit chain.',
  status: 'Status',
  startedAt: 'Started',
  finishedAt: 'Finished',
  manifestHash: 'Manifest hash',
  processedProfiles: 'Profiles',
  newVisits: 'New visits',
  newUrls: 'New URLs',
  newDownloads: 'New downloads',
  stillRunning: 'Still running',
  latestAction: 'Latest action',
  manifestPath: 'Manifest path',
  warnings: 'Warnings',
  remoteBackup: 'Remote backup',
  remoteBackupDescription:
    'Create a portable bundle and upload it to S3 or an S3-compatible endpoint. Credentials stay in the native keyring.',
  lastUploadAt: 'Last uploaded',
  objectKey: 'Object key',
  lastError: 'Last error',
  noRemoteUploadYet: 'No remote upload has completed yet.',
  importTitle: 'Takeout import and health checks',
  importDescription:
    'Start with a dry-run. Confirm what is recognized, what is quarantined, and then import supported history data.',
  importFlowHint:
    'Preview first. After import, every batch remains reviewable and reversible from the local audit trail.',
  takeoutPath: 'Takeout path',
  takeoutPlaceholder: '/Users/you/Downloads/takeout.zip',
  dryRun: 'Dry-run',
  importSupported: 'Import supported files',
  previewBatch: 'Preview batch',
  previewBatchReady: 'Import batch preview is ready.',
  revertBatch: 'Revert batch',
  revertBatchConfirm:
    'Revert this import batch from the live archive? The raw audit records will stay preserved for review.',
  recognizedFiles: 'Recognized files',
  quarantinedFiles: 'Quarantined files',
  candidateItems: 'Candidate rows',
  importedItems: 'Imported rows',
  duplicateItems: 'Duplicate rows',
  visibleItems: 'Visible rows',
  sourcePreview: 'Source preview',
  previewBeforeImport:
    'Inspect a sample before import, then review or revert imported batches later.',
  previewRows: 'Preview rows',
  notes: 'Notes',
  noRecognizedFiles: 'No directly importable files detected yet.',
  noQuarantinedFiles: 'No quarantined files in this run.',
  noPreviewRows: 'No preview rows are available for this selection yet.',
  noTakeoutInspection:
    'Run a dry-run first to see recognized files, quarantined files, and notes.',
  noImportBatches: 'No import batches have been recorded yet.',
  recentImports: 'Recent import batches',
  importAuditTrail:
    'Every Takeout import becomes a reviewable local batch with its own audit history.',
  selectedBatch: 'Selected batch',
  importBatchDetail: 'Import batch detail',
  noImportBatchSelected:
    'Select an import batch to inspect its preview rows and revert controls.',
  createdAt: 'Created',
  importedAt: 'Imported',
  revertedAt: 'Reverted',
  importedStatus: 'Imported',
  revertedStatus: 'Reverted',
  runningStatus: 'Running',
  revertKeepsAuditTitle: 'Revert keeps the audit trail',
  revertKeepsAuditBody:
    'Reverting removes imported rows from the live archive view, but the raw imported records stay preserved so you can inspect provenance later.',
  doctorChecks: 'Doctor checks',
  doctorDescription:
    'Check archive integrity, unlock state, scheduler files, and source availability.',
  runDoctor: 'Run doctor',
  doctorNotRun: 'Doctor checks have not been run yet in this session.',
  settingsTitle: 'Settings and key management',
  settingsDescription:
    'Control language, encryption, key handling, remote backup, and app behavior in one place.',
  languageSection: 'Language',
  languageDescription:
    'The app follows the system language by default. You can override it at any time.',
  interfaceLanguage: 'Interface language',
  resolvedLanguage: 'Current language',
  followSystem: 'Follow system',
  english: 'English',
  simplifiedChinese: '简体中文',
  traditionalChinese: '繁體中文',
  securitySection: 'Security',
  securityDescription:
    'Manage the encrypted archive state, unlock path, remembered key, and rekey operations.',
  currentState: 'Current state',
  keyringBackend: 'Keyring backend',
  rememberedKey: 'Remembered key',
  present: 'Present',
  absent: 'Absent',
  unlockArchive: 'Unlock archive',
  newMasterPassword: 'New master password',
  rotateKey: 'Rotate archive key',
  convertToPlaintext: 'Convert to plaintext',
  storeRememberedKey: 'Remember current key',
  clearRememberedKey: 'Clear remembered key',
  securityBeforeInit:
    'Initialize the archive first. Encryption changes and key rotation become available after that.',
  remoteSection: 'Remote backup',
  remoteSectionDescription:
    'Store periodic backup bundles in S3 or a compatible object store. Keep advanced endpoint settings secondary.',
  enableRemoteBackup: 'Enable remote backup',
  bucket: 'Bucket',
  region: 'Region',
  endpoint: 'Custom endpoint',
  prefix: 'Object key prefix',
  pathStyle: 'Use path-style requests',
  uploadAfterBackup: 'Upload automatically after each successful backup',
  accessKeyId: 'Access key ID',
  secretAccessKey: 'Secret access key',
  saveCredentials: 'Save credentials',
  clearCredentials: 'Clear credentials',
  credentialsSaved: 'Credentials stored',
  previewUpload: 'Preview upload',
  uploadNow: 'Upload now',
  previewCommand: 'Preview command',
  endpointHint:
    'Leave the endpoint empty for AWS S3. Add a custom endpoint only for S3-compatible storage.',
  appBehavior: 'App behavior',
  appAutostart: 'Launch the app automatically at login',
  saveSettings: 'Save settings',
  pending: 'Pending',
  yes: 'Yes',
  no: 'No',
} as const

export type TranslationKey = keyof typeof english

type TranslationDictionary = Record<TranslationKey, string>

const dictionaries: Record<ResolvedLanguage, TranslationDictionary> = {
  en: english,
  'zh-CN': {
    productName: 'browser history backup',
    localOnly: '仅本地',
    encrypted: '已加密',
    plaintext: '明文',
    unlocked: '已解锁',
    locked: '已锁定',
    profilesDetected: '{count} 个配置文件',
    dueEveryHours: '每 {hours} 小时到期',
    autoUnlockedNotice: '已使用记住的数据库密钥完成解锁。',
    initializedNotice: '归档已初始化。现在可以检查定时任务并运行第一次备份。',
    preferencesSavedNotice: '更改已保存。',
    unlockSuccess: '本次会话中的加密归档已解锁。',
    rotateSuccess: '归档密钥已轮换，本地 Stronghold 快照也已用新密码重建。',
    plaintextSuccess: '归档已切换为明文模式，本地 Stronghold 快照已清除。',
    backupComplete: '备份已完成。',
    backupCompleteWithManifest: '备份已完成，并写入了 {path}。',
    schedulePreviewReady: '定时任务预览已生成。',
    takeoutDryRunNotice: 'Takeout 预演已完成。请检查已识别文件与隔离文件。',
    takeoutImportNotice: 'Takeout 导入已写入 {count} 条记录。',
    revertBatchNotice:
      '已回滚所选导入批次。实时归档中的导入行已移除，但原始审计轨迹仍被保留。',
    doctorUpdated: '健康检查已更新。',
    rememberStored: '当前数据库密钥已保存到系统钥匙串，可用于无人值守运行。',
    rememberCleared: '系统钥匙串中的数据库密钥已清除。',
    s3CredentialsStored: 'S3 凭据已保存到系统钥匙串。',
    s3CredentialsCleared: 'S3 凭据已从系统钥匙串中清除。',
    s3PreviewReady: '远程备份预览已生成。',
    copiedNotice: '已复制到剪贴板。',
    enterMasterPassword: '请输入你保存在密码管理器中的主密码。',
    unlockBeforeRotate: '请先解锁归档，然后再轮换加密密钥。',
    enterNewMasterPassword: '请输入新的主密码，然后再轮换加密归档。',
    matchingPasswordsRequired: '加密模式初始化前需要输入一致的主密码。',
    enterTakeoutPath: '请先输入 Google Takeout 的 zip 路径或解压后的目录路径。',
    generateSchedulePreviewFirst: '请先生成定时任务预览。',
    unlockBeforeRemember: '请先解锁归档，再将当前密钥保存到系统钥匙串。',
    enterS3Credentials: '请先输入 S3 Access Key ID 和 Secret Access Key。',
    setupNav: '设置',
    explorerNav: '浏览',
    backupsNav: '备份',
    importNav: '导入',
    settingsNav: '设置项',
    setupTitle: '设置本地归档',
    setupDescription:
      '依次完成来源、存储、定时和复核步骤。每个系统动作都保留 Preview、Manual 和 Apply 路径。',
    sourcesStep: '1. 来源',
    sourcesDescription: '选择哪些浏览器配置文件应包含在备份中。',
    archiveStep: '2. 归档',
    archiveDescription: '在初始化或保存前，设置保留节奏与归档行为。',
    scheduleStep: '3. 原生定时任务',
    scheduleDescription:
      '预览操作系统原生调度器产物，检查命令，然后只在你愿意时由应用代为执行。',
    reviewStep: '4. 复核',
    reviewDescription: '继续前请确认本地路径和密码恢复风险。',
    storagePath: '应用数据根目录',
    archiveDatabase: '归档数据库',
    auditRepository: '审计仓库',
    archiveMode: '归档模式',
    dueAfterHours: '至少经过这么多小时才执行一次备份',
    checkIntervalHours: '唤醒检查间隔（小时）',
    captureFavicons: '同时保存 favicon 快照',
    gitAudit: '将 manifest 和审计工件提交到本地 git 仓库',
    rememberKey: '将数据库密钥保存在系统钥匙串中，供无人值守运行使用',
    masterPassword: '主密码',
    confirmPassword: '确认密码',
    passwordPlaceholder: '请保存到密码管理器',
    createArchive: '创建归档',
    saveSetup: '保存设置',
    schedulePlatform: '目标平台',
    previewSchedule: '预览原生定时任务',
    applyPreview: '应用当前预览',
    manualSteps: '手动步骤',
    generatedFiles: '生成的文件',
    noSchedulePreview: '先生成定时任务预览，以检查文件内容、命令和回滚路径。',
    encryptionWarningTitle: '密码恢复警告',
    encryptionWarningBody:
      '如果你忘记了加密归档的密码，而且没有其他有效的解锁路径，那么加密数据就等同于永久丢失。继续前请把密码存进密码管理器。',
    useSettingsForEncryption:
      '初始化后如需轮换密钥或在加密/明文间切换，请到设置页面操作。',
    historyDetected: '已检测到历史数据库',
    historyMissing: '未找到历史数据库',
    noSignedInUser: '没有登录用户元数据',
    unknownBrowserVersion: '版本不可用',
    selectedProfilesSummary: '已选配置文件',
    workflowGuide: '流程指引',
    importWorkflowTitle: '在变更归档前逐步检查每个导入步骤',
    reviewPlan: '审查计划',
    manualPathTitle: '手动路径',
    manualPathSummary: '你可以自己完成同样的流程。每条命令都能看到并复制。',
    manualPathReason: '手动模式让你在应用自动执行前先检查每一步。',
    applyChanges: '应用变更',
    applyChangesSummary: '当你确认无误后，再由应用执行已准备好的操作。',
    applyChangesReason:
      '自动模式仍然遵循同一份计划，只是把已审查的步骤替你执行。',
    verifyOutcome: '验证结果',
    verifyOutcomeSummary: '确认结果、检查审计输出，并把回滚路径放在手边。',
    verifyOutcomeReason:
      '验证步骤可以减少意外，并清楚说明系统是否按预期发生了变化。',
    finishStep: '完成',
    finishSummary:
      '只有在审计路径、回滚说明和可见结果都正确时，才结束这个流程。',
    finishReason: '最后的收尾检查能让后续审计与脏数据回滚更容易。',
    automaticPath: '为什么需要这一步',
    currentStep: '当前',
    markStepComplete: '标记为已完成',
    stepCompleted: '已完成',
    manualImportSummary: '在导入任何记录前，你可以先自己检查 Takeout 档案。',
    manualImportReason: '手动检查有助于确认隐私影响、已识别文件和归档范围。',
    manualLocateStep: '定位 Takeout 压缩包或解压后的文件夹。',
    manualInspectStep: '检查文件内容，并与已识别列表逐项对照。',
    manualContinueStep: '只有在预览看起来正确后，再回来继续下一步。',
    applyImportReason:
      '只导入已识别的历史记录，未支持的文件会保持隔离并可继续检查。',
    verifyImportReason: '导入后检查批次、预览行、重复项以及回滚控制。',
    whyThisStepMatters: '为什么需要这一步',
    profileSelectionReason:
      '应用只会读取本地浏览器历史数据库，先生成安全的暂存副本，再从副本导入，因此不会修改正在使用的浏览器数据。',
    dataFilesRead: '将读取的数据文件',
    noFilesSelectedYet:
      '请至少选择一个浏览器配置文件，才能查看路径和手动命令。',
    manualAlternative: '手动替代方案',
    explorerTitle: '历史记录浏览器',
    explorerDescription:
      '搜索长期归档，按域名或配置文件筛选，并在需要时导出当前视图。',
    searchLabel: '搜索',
    searchPlaceholder: 'URL、标题或关键词',
    domainLabel: '域名',
    domainPlaceholder: 'example.com',
    profileLabel: '配置文件',
    allProfiles: '所有已选配置文件',
    exportLabel: '导出',
    noHistoryResults: '还没有匹配结果。可以尝试放宽搜索条件或先运行一次备份。',
    unlockToSearch: '请先初始化或解锁归档，再搜索时间线。',
    resultsCount: '{count} 条匹配访问',
    selectedVisit: '选中的访问记录',
    visitedAt: '访问时间',
    titleLabel: '标题',
    urlLabel: 'URL',
    provenance: '来源追踪',
    domain: '域名',
    profile: '配置文件',
    duration: '停留时长',
    transition: '跳转类型',
    sourceVisitId: '源 visit ID',
    appId: '应用 ID',
    notAvailable: '不可用',
    backupsTitle: '备份与审计账本',
    backupsDescription:
      '查看备份运行、manifest 哈希、变化摘要，以及最近一次远程备份状态。',
    runBackupNow: '立即备份',
    recentRuns: '最近运行',
    runDetails: '运行详情',
    noRuns: '还没有完成的运行。执行第一次备份后会开始形成审计链。',
    status: '状态',
    startedAt: '开始时间',
    finishedAt: '结束时间',
    manifestHash: 'Manifest 哈希',
    processedProfiles: '配置文件数',
    newVisits: '新增访问',
    newUrls: '新增 URL',
    newDownloads: '新增下载',
    stillRunning: '仍在运行',
    latestAction: '最近一次操作',
    manifestPath: 'Manifest 路径',
    warnings: '警告',
    remoteBackup: '远程备份',
    remoteBackupDescription:
      '创建便携备份包并上传到 S3 或兼容端点。凭据只保存在原生钥匙串中。',
    lastUploadAt: '最近上传',
    objectKey: '对象键',
    lastError: '最近错误',
    noRemoteUploadYet: '还没有成功完成远程上传。',
    importTitle: 'Takeout 导入与健康检查',
    importDescription:
      '先做预演，再确认哪些文件被识别、哪些被隔离，然后导入受支持的历史数据。',
    importFlowHint: '先预览。导入后，每个批次都可以继续检查，也可以单独回滚。',
    takeoutPath: 'Takeout 路径',
    takeoutPlaceholder: '/Users/you/Downloads/takeout.zip',
    dryRun: '预演',
    importSupported: '导入支持的文件',
    previewBatch: '预览批次',
    previewBatchReady: '导入批次预览已生成。',
    revertBatch: '回滚批次',
    revertBatchConfirm:
      '要把这个导入批次从实时归档中回滚吗？原始审计记录仍会保留供后续检查。',
    recognizedFiles: '已识别文件',
    quarantinedFiles: '隔离文件',
    candidateItems: '候选行',
    importedItems: '已导入行',
    duplicateItems: '重复行',
    visibleItems: '当前可见行',
    sourcePreview: '来源预览',
    previewBeforeImport:
      '先检查样本，再导入；导入后的批次也可以随时复查或回滚。',
    previewRows: '预览行',
    notes: '备注',
    noRecognizedFiles: '暂时没有检测到可直接导入的文件。',
    noQuarantinedFiles: '本次运行没有隔离文件。',
    noPreviewRows: '当前选择还没有可显示的预览行。',
    noTakeoutInspection: '请先运行预演，查看已识别文件、隔离文件和备注。',
    noImportBatches: '还没有记录任何导入批次。',
    recentImports: '最近导入批次',
    importAuditTrail:
      '每次 Takeout 导入都会成为一个可复查、可回滚的本地批次，并保留自己的审计历史。',
    selectedBatch: '选中批次',
    importBatchDetail: '导入批次详情',
    noImportBatchSelected: '请选择一个导入批次，以查看预览行与回滚控制。',
    createdAt: '创建时间',
    importedAt: '导入时间',
    revertedAt: '回滚时间',
    importedStatus: '已导入',
    revertedStatus: '已回滚',
    runningStatus: '进行中',
    revertKeepsAuditTitle: '回滚不会抹掉审计轨迹',
    revertKeepsAuditBody:
      '回滚会把导入行从实时归档视图中移除，但原始导入记录仍会保留下来，方便之后检查来源与差异。',
    doctorChecks: '健康检查',
    doctorDescription: '检查归档完整性、解锁状态、调度文件与来源可用性。',
    runDoctor: '运行检查',
    doctorNotRun: '本次会话中还没有运行健康检查。',
    settingsTitle: '设置与密钥管理',
    settingsDescription:
      '在同一处管理语言、加密、密钥处理、远程备份和应用行为。',
    languageSection: '语言',
    languageDescription: '默认跟随系统语言，也可以随时手动覆盖。',
    interfaceLanguage: '界面语言',
    resolvedLanguage: '当前语言',
    followSystem: '跟随系统',
    english: 'English',
    simplifiedChinese: '简体中文',
    traditionalChinese: '繁體中文',
    securitySection: '安全',
    securityDescription: '管理加密状态、解锁路径、记住的密钥以及重新加密操作。',
    currentState: '当前状态',
    keyringBackend: '钥匙串后端',
    rememberedKey: '记住的密钥',
    present: '存在',
    absent: '不存在',
    unlockArchive: '解锁归档',
    newMasterPassword: '新的主密码',
    rotateKey: '轮换归档密钥',
    convertToPlaintext: '转换为明文',
    storeRememberedKey: '记住当前密钥',
    clearRememberedKey: '清除记住的密钥',
    securityBeforeInit: '请先初始化归档。初始化后才能进行加密变更和密钥轮换。',
    remoteSection: '远程备份',
    remoteSectionDescription:
      '把周期性备份包保存到 S3 或兼容对象存储。高级端点设置保持次要层级。',
    enableRemoteBackup: '启用远程备份',
    bucket: 'Bucket',
    region: 'Region',
    endpoint: '自定义端点',
    prefix: '对象键前缀',
    pathStyle: '使用 path-style 请求',
    uploadAfterBackup: '每次成功备份后自动上传',
    accessKeyId: 'Access Key ID',
    secretAccessKey: 'Secret Access Key',
    saveCredentials: '保存凭据',
    clearCredentials: '清除凭据',
    credentialsSaved: '凭据已保存',
    previewUpload: '预览上传',
    uploadNow: '立即上传',
    previewCommand: '预览命令',
    endpointHint:
      '若使用 AWS S3，请留空 endpoint。仅在使用 S3 兼容存储时填写自定义 endpoint。',
    appBehavior: '应用行为',
    appAutostart: '登录时自动启动应用',
    saveSettings: '保存设置',
    pending: '待处理',
    yes: '是',
    no: '否',
  },
  'zh-TW': {
    productName: 'browser history backup',
    localOnly: '僅本地',
    encrypted: '已加密',
    plaintext: '明文',
    unlocked: '已解鎖',
    locked: '已鎖定',
    profilesDetected: '{count} 個設定檔',
    dueEveryHours: '每 {hours} 小時到期',
    autoUnlockedNotice: '已使用記住的資料庫金鑰完成解鎖。',
    initializedNotice: '封存已初始化。現在可以檢查排程並執行第一次備份。',
    preferencesSavedNotice: '變更已儲存。',
    unlockSuccess: '本次工作階段中的加密封存已解鎖。',
    rotateSuccess: '封存金鑰已輪換，本地 Stronghold 快照也已用新密碼重建。',
    plaintextSuccess: '封存已切換為明文模式，本地 Stronghold 快照已清除。',
    backupComplete: '備份已完成。',
    backupCompleteWithManifest: '備份已完成，並寫入了 {path}。',
    schedulePreviewReady: '排程預覽已生成。',
    takeoutDryRunNotice: 'Takeout 預演已完成。請檢查已識別檔案與隔離檔案。',
    takeoutImportNotice: 'Takeout 匯入已寫入 {count} 筆記錄。',
    revertBatchNotice:
      '已回滾所選匯入批次。即時封存中的匯入列已移除，但原始審計軌跡仍被保留。',
    doctorUpdated: '健康檢查已更新。',
    rememberStored:
      '目前的資料庫金鑰已儲存到系統鑰匙圈，可供無人值守執行使用。',
    rememberCleared: '系統鑰匙圈中的資料庫金鑰已清除。',
    s3CredentialsStored: 'S3 憑證已儲存到系統鑰匙圈。',
    s3CredentialsCleared: 'S3 憑證已從系統鑰匙圈中清除。',
    s3PreviewReady: '遠端備份預覽已生成。',
    copiedNotice: '已複製到剪貼簿。',
    enterMasterPassword: '請輸入你保存在密碼管理器中的主密碼。',
    unlockBeforeRotate: '請先解鎖封存，再輪換加密金鑰。',
    enterNewMasterPassword: '請先輸入新的主密碼，再輪換加密封存。',
    matchingPasswordsRequired: '加密模式初始化前需要輸入一致的主密碼。',
    enterTakeoutPath:
      '請先輸入 Google Takeout 的 zip 路徑或解壓後的資料夾路徑。',
    generateSchedulePreviewFirst: '請先生成排程預覽。',
    unlockBeforeRemember: '請先解鎖封存，再將目前的金鑰儲存到系統鑰匙圈。',
    enterS3Credentials: '請先輸入 S3 Access Key ID 和 Secret Access Key。',
    setupNav: '設定',
    explorerNav: '瀏覽',
    backupsNav: '備份',
    importNav: '匯入',
    settingsNav: '設定項',
    setupTitle: '設定本地封存',
    setupDescription:
      '依序完成來源、儲存、排程與複核步驟。每個系統動作都保留 Preview、Manual 和 Apply 路徑。',
    sourcesStep: '1. 來源',
    sourcesDescription: '選擇哪些瀏覽器設定檔要納入備份。',
    archiveStep: '2. 封存',
    archiveDescription: '在初始化或儲存前，設定保留節奏與封存行為。',
    scheduleStep: '3. 原生排程',
    scheduleDescription:
      '預覽作業系統原生排程器工件，檢查命令，然後只在你願意時由應用代為執行。',
    reviewStep: '4. 複核',
    reviewDescription: '繼續前請確認本地路徑與密碼恢復風險。',
    storagePath: '應用資料根目錄',
    archiveDatabase: '封存資料庫',
    auditRepository: '審計倉庫',
    archiveMode: '封存模式',
    dueAfterHours: '至少經過這麼多小時才執行一次備份',
    checkIntervalHours: '喚醒檢查間隔（小時）',
    captureFavicons: '同時保存 favicon 快照',
    gitAudit: '將 manifest 和審計工件提交到本地 git 倉庫',
    rememberKey: '將資料庫金鑰保存在系統鑰匙圈中，供無人值守執行使用',
    masterPassword: '主密碼',
    confirmPassword: '確認密碼',
    passwordPlaceholder: '請保存到密碼管理器',
    createArchive: '建立封存',
    saveSetup: '儲存設定',
    schedulePlatform: '目標平台',
    previewSchedule: '預覽原生排程',
    applyPreview: '套用目前預覽',
    manualSteps: '手動步驟',
    generatedFiles: '生成的檔案',
    noSchedulePreview: '先生成排程預覽，以檢查檔案內容、命令與回滾路徑。',
    encryptionWarningTitle: '密碼恢復警告',
    encryptionWarningBody:
      '如果你忘記了加密封存的密碼，而且沒有其他有效的解鎖路徑，那麼加密資料就等同於永久遺失。繼續前請把密碼存進密碼管理器。',
    useSettingsForEncryption:
      '初始化後如需輪換金鑰或在加密/明文間切換，請到設定頁面操作。',
    historyDetected: '已檢測到歷史資料庫',
    historyMissing: '未找到歷史資料庫',
    noSignedInUser: '沒有登入使用者中繼資料',
    unknownBrowserVersion: '版本不可用',
    selectedProfilesSummary: '已選設定檔',
    workflowGuide: '流程指引',
    importWorkflowTitle: '在變更封存前逐步檢查每個匯入步驟',
    reviewPlan: '審查計畫',
    manualPathTitle: '手動路徑',
    manualPathSummary: '你可以自己完成同樣的流程。每條命令都能看到並複製。',
    manualPathReason: '手動模式讓你在應用自動執行前先檢查每一步。',
    applyChanges: '套用變更',
    applyChangesSummary: '當你確認無誤後，再由應用執行已準備好的操作。',
    applyChangesReason:
      '自動模式仍然遵循同一份計畫，只是把已審查的步驟替你執行。',
    verifyOutcome: '驗證結果',
    verifyOutcomeSummary: '確認結果、檢查審計輸出，並把回滾路徑放在手邊。',
    verifyOutcomeReason:
      '驗證步驟可以減少意外，並清楚說明系統是否按預期發生了變化。',
    finishStep: '完成',
    finishSummary:
      '只有在審計路徑、回滾說明與可見結果都正確時，才結束這個流程。',
    finishReason: '最後的收尾檢查能讓後續審計與髒資料回滾更容易。',
    automaticPath: '為什麼需要這一步',
    currentStep: '目前',
    markStepComplete: '標記為已完成',
    stepCompleted: '已完成',
    manualImportSummary: '在匯入任何記錄前，你可以先自己檢查 Takeout 封存。',
    manualImportReason: '手動檢查有助於確認隱私影響、已識別檔案和封存範圍。',
    manualLocateStep: '定位 Takeout 壓縮檔或解壓後的資料夾。',
    manualInspectStep: '檢查檔案內容，並與已識別清單逐項對照。',
    manualContinueStep: '只有在預覽看起來正確後，再回來繼續下一步。',
    applyImportReason:
      '只匯入已識別的歷史記錄，未支援的檔案會保持隔離並可繼續檢查。',
    verifyImportReason: '匯入後檢查批次、預覽列、重複項以及回滾控制。',
    whyThisStepMatters: '為什麼需要這一步',
    profileSelectionReason:
      '應用只會讀取本地瀏覽器歷史資料庫，先生成安全的暫存副本，再從副本匯入，因此不會修改正在使用的瀏覽器資料。',
    dataFilesRead: '將讀取的資料檔案',
    noFilesSelectedYet: '請至少選擇一個瀏覽器設定檔，才能查看路徑與手動命令。',
    manualAlternative: '手動替代方案',
    explorerTitle: '歷史紀錄瀏覽器',
    explorerDescription:
      '搜尋長期封存，按網域或設定檔篩選，並在需要時匯出目前視圖。',
    searchLabel: '搜尋',
    searchPlaceholder: 'URL、標題或關鍵字',
    domainLabel: '網域',
    domainPlaceholder: 'example.com',
    profileLabel: '設定檔',
    allProfiles: '所有已選設定檔',
    exportLabel: '匯出',
    noHistoryResults: '還沒有匹配結果。可以嘗試放寬搜尋條件或先執行一次備份。',
    unlockToSearch: '請先初始化或解鎖封存，再搜尋時間軸。',
    resultsCount: '{count} 筆匹配訪問',
    selectedVisit: '選中的訪問記錄',
    visitedAt: '訪問時間',
    titleLabel: '標題',
    urlLabel: 'URL',
    provenance: '來源追蹤',
    domain: '網域',
    profile: '設定檔',
    duration: '停留時長',
    transition: '跳轉類型',
    sourceVisitId: '來源 visit ID',
    appId: '應用 ID',
    notAvailable: '不可用',
    backupsTitle: '備份與審計帳本',
    backupsDescription:
      '查看備份執行、manifest 雜湊、變更摘要，以及最近一次遠端備份狀態。',
    runBackupNow: '立即備份',
    recentRuns: '最近執行',
    runDetails: '執行詳情',
    noRuns: '還沒有完成的執行。執行第一次備份後會開始形成審計鏈。',
    status: '狀態',
    startedAt: '開始時間',
    finishedAt: '結束時間',
    manifestHash: 'Manifest 雜湊',
    processedProfiles: '設定檔數',
    newVisits: '新增訪問',
    newUrls: '新增 URL',
    newDownloads: '新增下載',
    stillRunning: '仍在執行',
    latestAction: '最近一次操作',
    manifestPath: 'Manifest 路徑',
    warnings: '警告',
    remoteBackup: '遠端備份',
    remoteBackupDescription:
      '建立可攜備份包並上傳到 S3 或相容端點。憑證只保存在原生鑰匙圈中。',
    lastUploadAt: '最近上傳',
    objectKey: '物件鍵',
    lastError: '最近錯誤',
    noRemoteUploadYet: '還沒有成功完成遠端上傳。',
    importTitle: 'Takeout 匯入與健康檢查',
    importDescription:
      '先做預演，再確認哪些檔案被識別、哪些被隔離，然後匯入受支援的歷史資料。',
    importFlowHint: '先預覽。匯入後，每個批次都可以繼續檢查，也可以單獨回滾。',
    takeoutPath: 'Takeout 路徑',
    takeoutPlaceholder: '/Users/you/Downloads/takeout.zip',
    dryRun: '預演',
    importSupported: '匯入支援的檔案',
    previewBatch: '預覽批次',
    previewBatchReady: '匯入批次預覽已生成。',
    revertBatch: '回滾批次',
    revertBatchConfirm:
      '要把這個匯入批次從即時封存中回滾嗎？原始審計記錄仍會保留供後續檢查。',
    recognizedFiles: '已識別檔案',
    quarantinedFiles: '隔離檔案',
    candidateItems: '候選列',
    importedItems: '已匯入列',
    duplicateItems: '重複列',
    visibleItems: '目前可見列',
    sourcePreview: '來源預覽',
    previewBeforeImport:
      '先檢查樣本，再匯入；匯入後的批次也可以隨時複查或回滾。',
    previewRows: '預覽列',
    notes: '備註',
    noRecognizedFiles: '暫時沒有檢測到可直接匯入的檔案。',
    noQuarantinedFiles: '本次執行沒有隔離檔案。',
    noPreviewRows: '目前選擇還沒有可顯示的預覽列。',
    noTakeoutInspection: '請先執行預演，查看已識別檔案、隔離檔案與備註。',
    noImportBatches: '還沒有記錄任何匯入批次。',
    recentImports: '最近匯入批次',
    importAuditTrail:
      '每次 Takeout 匯入都會成為一個可複查、可回滾的本地批次，並保留自己的審計歷史。',
    selectedBatch: '選中批次',
    importBatchDetail: '匯入批次詳情',
    noImportBatchSelected: '請選擇一個匯入批次，以查看預覽列與回滾控制。',
    createdAt: '建立時間',
    importedAt: '匯入時間',
    revertedAt: '回滾時間',
    importedStatus: '已匯入',
    revertedStatus: '已回滾',
    runningStatus: '進行中',
    revertKeepsAuditTitle: '回滾不會抹掉審計軌跡',
    revertKeepsAuditBody:
      '回滾會把匯入列從即時封存視圖中移除，但原始匯入記錄仍會保留下來，方便之後檢查來源與差異。',
    doctorChecks: '健康檢查',
    doctorDescription: '檢查封存完整性、解鎖狀態、排程檔案與來源可用性。',
    runDoctor: '執行檢查',
    doctorNotRun: '本次工作階段中還沒有執行健康檢查。',
    settingsTitle: '設定與金鑰管理',
    settingsDescription:
      '在同一處管理語言、加密、金鑰處理、遠端備份與應用行為。',
    languageSection: '語言',
    languageDescription: '預設跟隨系統語言，也可以隨時手動覆蓋。',
    interfaceLanguage: '介面語言',
    resolvedLanguage: '目前語言',
    followSystem: '跟隨系統',
    english: 'English',
    simplifiedChinese: '简体中文',
    traditionalChinese: '繁體中文',
    securitySection: '安全',
    securityDescription: '管理加密狀態、解鎖路徑、記住的金鑰以及重新加密操作。',
    currentState: '目前狀態',
    keyringBackend: '鑰匙圈後端',
    rememberedKey: '記住的金鑰',
    present: '存在',
    absent: '不存在',
    unlockArchive: '解鎖封存',
    newMasterPassword: '新的主密碼',
    rotateKey: '輪換封存金鑰',
    convertToPlaintext: '轉換為明文',
    storeRememberedKey: '記住目前金鑰',
    clearRememberedKey: '清除記住的金鑰',
    securityBeforeInit: '請先初始化封存。初始化後才能進行加密變更與金鑰輪換。',
    remoteSection: '遠端備份',
    remoteSectionDescription:
      '把週期性備份包保存到 S3 或相容物件儲存。進階端點設定保持次要層級。',
    enableRemoteBackup: '啟用遠端備份',
    bucket: 'Bucket',
    region: 'Region',
    endpoint: '自訂端點',
    prefix: '物件鍵前綴',
    pathStyle: '使用 path-style 請求',
    uploadAfterBackup: '每次成功備份後自動上傳',
    accessKeyId: 'Access Key ID',
    secretAccessKey: 'Secret Access Key',
    saveCredentials: '儲存憑證',
    clearCredentials: '清除憑證',
    credentialsSaved: '憑證已儲存',
    previewUpload: '預覽上傳',
    uploadNow: '立即上傳',
    previewCommand: '預覽命令',
    endpointHint:
      '若使用 AWS S3，請留空 endpoint。僅在使用 S3 相容儲存時填寫自訂 endpoint。',
    appBehavior: '應用行為',
    appAutostart: '登入時自動啟動應用',
    saveSettings: '儲存設定',
    pending: '待處理',
    yes: '是',
    no: '否',
  },
}
// Stryker restore all

function interpolate(template: string, vars?: Record<string, string | number>) {
  if (!vars) {
    return template
  }

  return template.replace(/\{(\w+)\}/g, (_, key: string) =>
    key in vars ? String(vars[key]) : `{${key}}`,
  )
}

export function detectSystemLanguage(
  languages?: readonly string[],
): ResolvedLanguage {
  const source =
    languages ??
    (typeof navigator !== 'undefined'
      ? navigator.languages.length
        ? navigator.languages
        : [navigator.language]
      : ['en'])

  for (const value of source) {
    const normalized = value.toLowerCase()
    if (
      normalized.includes('zh-tw') ||
      normalized.includes('zh-hk') ||
      normalized.includes('zh-mo') ||
      normalized.includes('zh-hant')
    ) {
      return 'zh-TW'
    }
    if (normalized.startsWith('zh')) {
      return 'zh-CN'
    }
  }

  return 'en'
}

export function resolveLanguage(
  preference: LanguagePreference | null | undefined,
  languages?: readonly string[],
): ResolvedLanguage {
  if (!preference || preference === 'system') {
    return detectSystemLanguage(languages)
  }

  return preference
}

export function createTranslator(language: ResolvedLanguage) {
  const dictionary = dictionaries[language] ?? dictionaries.en
  return (key: TranslationKey, vars?: Record<string, string | number>) =>
    interpolate(dictionary[key] ?? dictionaries.en[key] ?? key, vars)
}

export function languageLabel(
  value: LanguagePreference | ResolvedLanguage,
  uiLanguage: ResolvedLanguage,
) {
  const translate = createTranslator(uiLanguage)
  switch (value) {
    case 'system':
      return translate('followSystem')
    case 'zh-CN':
      return translate('simplifiedChinese')
    case 'zh-TW':
      return translate('traditionalChinese')
    default:
      return translate('english')
  }
}

export function localeTag(language: ResolvedLanguage) {
  return language
}
