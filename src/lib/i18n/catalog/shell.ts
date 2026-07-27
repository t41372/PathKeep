/**
 * @file shell.ts
 * @description Owns app-shell status, navigation chrome, and global feedback copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `shell` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so shell copy changes do not pull in runtime translation logic.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `shell` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const shellNamespaceCatalog = {
  en: {
    savingArchiveChoices: 'Saving archive choices',
    savingArchiveChoicesDetail:
      'Writing the updated archive configuration and refreshing the app state.',
    preparingArchive: 'Preparing the archive',
    preparingArchiveDetail:
      'Creating the archive database, applying migrations, and locking in the current setup choices.',
    runningManualBackup: 'Running a manual backup',
    runningManualBackupDetail:
      'Inspecting the selected browser profiles before PathKeep writes archive records.',
    backupWritingArchive: 'Writing archive records',
    backupWritingArchiveDetail:
      'Normalizing visits, URLs, and audit artifacts. Large real-world profiles can take a while here.',
    backupProfileProgress: '{profileId} ({current}/{total})',
    backupProgressPending: 'Waiting for backup progress',
    backupRecordProgressPending: 'Waiting for record batches',
    backupRecordProgress: '{count} records processed',
    backupRecordStats: '{imported} new · {duplicates} duplicates',
    backupSkippedRecords: '{count} skipped',
    backupFinalizeProgress:
      'Processed {current} of {total} selected profiles. Preparing the manifest and cached totals.',
    refreshingArchiveViews: 'Refreshing archive views',
    refreshingArchiveViewsDetail:
      'Reloading dashboard totals, recent runs, and other shell surfaces with the latest archive state.',
    backupStepPrepare: 'Inspect selected browser profiles',
    backupStepArchive: 'Write archive records',
    backupStepRefresh: 'Refresh dashboard and app state',
    loadingLatestArchiveState:
      'PathKeep could not load the latest archive state.',
    runtimeCrashNotice:
      'PathKeep found a recent crash report. Review the logs and crash diagnostics in Settings.',
    scheduleStaleAfterUpgradeTitle: 'Backup schedule needs re-applying',
    scheduleStaleAfterUpgradeBody:
      'Your existing macOS LaunchAgent was installed by an earlier PathKeep build that the current macOS no longer permits to launch directly. Open Settings → Backup → Schedule and re-apply the schedule once to update the launcher.',
    scheduleHealthProbeFailedTitle:
      'PathKeep could not inspect scheduled backup',
    scheduleHealthProbeFailedBody:
      'PathKeep cannot confirm that scheduled backup is healthy. Open Scheduled Backup Settings to review and retry. Diagnostic:',
    savingSettingsFailed:
      'PathKeep could not save the updated archive settings.',
    initializeArchiveFailed: 'PathKeep could not initialize the archive.',
    initializedNotice:
      'Archive initialized. Review the first backup before automation.',
    manualBackupDueWindow:
      'Your archive is already up to date, so PathKeep skipped this backup. The next one runs when the schedule comes due.',
    manualBackupWriteLockDeferred:
      'Another archive task is still writing, so PathKeep did not start this backup. Wait for it to finish in Activity, then run the backup again.',
    manualBackupFinished: 'Manual backup finished as run #{runId}.',
    manualBackupFailed: 'PathKeep could not complete the manual backup.',
    safariFullDiskAccessBackupWarning:
      'Backup run #{runId} finished for readable profiles, but Safari was skipped because it still needs Full Disk Access.',
    fullDiskAccessBackupError:
      'PathKeep needs Full Disk Access to read your browser history. Open System Settings → Privacy & Security → Full Disk Access and enable PathKeep, then run the backup again.',
    fullDiskAccessOpenSettings: 'Open Full Disk Access settings',
    backupFailedAlertLabel: 'Backup failed — action needed',
    backupFailedHeading: 'Backup didn’t finish',
    configSaveFailedHeading: 'That setting wasn’t saved',
    configSaveFailedReassurance:
      'The control may still show what you picked, but PathKeep kept the previously saved value. Change it again to retry, or copy the details below.',
    backupFailedReassurance:
      'Your existing archive is safe — nothing was lost.',
    backupFailedRetry: 'Try again',
    backupFailedCopyDiagnostics: 'Copy diagnostics',
    backupFailedCopied: 'Copied',
    backupFailedShowDetails: 'Technical details',
    backupFailedDismiss: 'Dismiss this alert',
    backupRunFailed: 'Failed',
    backupRunErrorReason: 'Error',
    revealLogs: 'Reveal logs',
    revealLogsAriaLabel: 'Reveal the PathKeep logs folder in Finder',
    settingAppLockPasscode: 'Saving app lock passcode',
    settingAppLockPasscodeDetail:
      'Storing the session passcode and refreshing the app lock state.',
    setAppLockPasscodeFailed: 'PathKeep could not save the app lock passcode.',
    clearingAppLockPasscode: 'Clearing app lock passcode',
    clearingAppLockPasscodeDetail:
      'Removing the current passcode and disabling app lock for this device.',
    clearAppLockPasscodeFailed:
      'PathKeep could not clear the app lock passcode.',
    lockingApp: 'Locking PathKeep',
    lockingAppDetail:
      'Hiding archive data and returning the desktop app to the lock screen.',
    lockAppFailed: 'PathKeep could not lock the current app session.',
    unlockingApp: 'Unlocking PathKeep',
    unlockingAppDetail:
      'Verifying the app lock passcode and restoring the latest app state.',
    unlockAppFailed: 'PathKeep could not unlock the current app session.',
    unlockErrorBiometricNotEnrolled:
      'No biometric credentials are enrolled on this device. Use the app lock passcode instead.',
    unlockErrorBiometricLockout:
      'Biometric unlock is locked out on this device right now. Unlock it in the system settings or use the app lock passcode.',
    unlockErrorBiometricUnavailable:
      'Biometric unlock is not available on this device right now. Use the app lock passcode instead.',
    unlockErrorBiometricCanceled:
      'Biometric unlock was canceled. Try again or use the app lock passcode.',
    unlockErrorBiometricTurnedOff:
      'Biometric unlock is currently turned off in Settings. Use the app lock passcode instead.',
    unlockErrorBiometricFailed:
      'Biometric unlock could not verify your identity. Try again or use the app lock passcode.',
    lockEyebrow: 'APP LOCK',
    lockTitle: 'Unlock PathKeep',
    lockDescription:
      'This lock only protects the PathKeep desktop session. Archive encryption remains a separate at-rest layer.',
    lockReason: 'Lock reason',
    lockReasonStartup: 'Startup check',
    lockReasonManual: 'Manual lock',
    lockReasonIdleTimeout: 'Idle timeout',
    lockConfigPath: 'Lock config',
    lastUnlockedAt: 'Last unlocked',
    lockPasscodeLabel: 'Passcode',
    lockPasscodePlaceholder: 'Enter your app lock passcode',
    unlockApp: 'Unlock',
    unlockWithBiometric: 'Use biometric',
    unlockWithTouchId: 'Use Touch ID',
    unlockBiometricUnavailable:
      'Biometric unlock is not available in this desktop build yet, so PathKeep is using the passcode fallback.',
    unlockTouchIdUnavailable:
      'Touch ID is unavailable on this Mac right now, so PathKeep is using the passcode fallback.',
    lockRecoveryTitle: 'Forgot passcode?',
    lockRecoveryBody:
      'The passcode cannot be reset from this screen, and your history is not lost — the app lock only guards this window. Open the config path and follow the support guidance to clear the lock.',
    lockRecoveryHintBody:
      'Recovery hint: {hint}. PathKeep still requires the passcode to unlock this session.',
    lockRecoveryAction: 'Open config path',
    onboardingVersion: 'Setup',
    onboardingLeaveHint:
      'You can leave setup at any time. Your choices are saved automatically, and you can come back from Dashboard or Settings.',
    exitSetup: 'Exit setup',
    // ── Paper-redesign shell chrome ──
    findAPage: 'Find a page…',
    archiveKept: 'Archive kept',
    archiving: 'Archiving…',
    archiveNotInitialized: 'Archive not initialized',
    backgroundTasksRunningOne: '1 task running',
    backgroundTasksRunningMany: '{count} tasks running',
    backgroundTaskViewActivity: 'View background activity',
    backgroundTaskStarted: 'Background work started',
    backgroundTaskEnded: 'Background work finished',
    pages: 'pages',
    since: 'Since {month} {year}',
    lastArchivedAt: 'Last archived {time}',
    sourcesTitle: 'Sources',
    sourcesConnected: '{count} connected',
    sourcesAll: 'All sources',
    sourcesCountSingular: '{count} source',
    sourcesCountPlural: '{count} sources',
    manageSources: 'Manage sources…',
    paletteTitle: 'Find a page',
    paletteDescription: 'Search across every visit in your archive',
    paletteEmptyHint: 'Start typing to search your archive…',
    paletteLoading: 'Searching…',
    paletteNoResults: 'Nothing here yet. Memory is patient.',
    paletteHintOpen: 'open',
    paletteHintFullSearch: 'full search',
    paletteHintNavigate: 'navigate',
    paletteHintClose: 'close',
    epigraph1: 'Memory is patient.',
    epigraph2: 'Nothing is lost.',
    epigraph3: 'Every page, kept.',
    epigraph4: "You've been somewhere.",
    epigraph5: 'The archive remembers.',
    epigraph6: 'A small library, growing.',
  },
  'zh-CN': {
    savingArchiveChoices: '正在保存存档选项',
    savingArchiveChoicesDetail: '正在写入更新后的存档配置，并刷新 shell 状态。',
    preparingArchive: '正在准备存档',
    preparingArchiveDetail:
      '正在创建存档数据库、应用迁移，并锁定当前的初始化选择。',
    runningManualBackup: '正在运行手动备份',
    runningManualBackupDetail: '正在检查所选的浏览器配置，然后写入存档记录。',
    backupWritingArchive: '正在写入存档记录',
    backupWritingArchiveDetail:
      '正在整理访问记录、网址和审计文件。浏览记录多的浏览器在这一步可能会花一些时间。',
    backupProfileProgress: '{profileId}（{current}/{total}）',
    backupProgressPending: '等待备份进度',
    backupRecordProgressPending: '等待记录批次',
    backupRecordProgress: '已处理 {count} 条记录',
    backupRecordStats: '新增 {imported} · 重复 {duplicates}',
    backupSkippedRecords: '已跳过 {count} 条',
    backupFinalizeProgress:
      '已处理 {current}/{total} 个选定的浏览器配置，正在准备清单与缓存总计。',
    refreshingArchiveViews: '正在刷新存档视图',
    refreshingArchiveViewsDetail:
      '正在重新加载仪表盘统计、最近运行和其他 shell 视图。',
    backupStepPrepare: '检查所选的浏览器配置',
    backupStepArchive: '写入存档记录',
    backupStepRefresh: '刷新仪表盘与 shell 状态',
    loadingLatestArchiveState: 'PathKeep 无法加载最新的存档状态。',
    runtimeCrashNotice:
      'PathKeep 发现了最近一次崩溃报告。请到设置里查看日志和崩溃诊断。',
    scheduleStaleAfterUpgradeTitle: '备份计划需要重新套用',
    scheduleStaleAfterUpgradeBody:
      '当前安装的 macOS LaunchAgent 由旧版 PathKeep 写入，新版 macOS 已不再允许它直接启动。请到「设置 → 备份 → 计划」重新套用一次，新的启动方式才会生效。',
    scheduleHealthProbeFailedTitle: 'PathKeep 无法检查定时备份',
    scheduleHealthProbeFailedBody:
      'PathKeep 无法确认定时备份是否正常。请打开定时备份设置查看并重试。诊断：',
    savingSettingsFailed: 'PathKeep 无法保存更新后的设置。',
    initializeArchiveFailed: 'PathKeep 无法初始化存档。',
    initializedNotice: '存档已初始化。请在开启自动化前先检查第一次备份。',
    manualBackupDueWindow:
      '你的存档已经是最新的，PathKeep 跳过了这次备份。下次备份会在计划到期时运行。',
    manualBackupWriteLockDeferred:
      '另一项存档任务还在写入，PathKeep 没有开始这次备份。等它在活动页里跑完，再重新备份一次。',
    manualBackupFinished: '手动备份已完成，运行编号 #{runId}。',
    manualBackupFailed: 'PathKeep 无法完成手动备份。',
    safariFullDiskAccessBackupWarning:
      '备份运行 #{runId} 已完成可读取的浏览器配置，但 Safari 仍缺少“完全磁盘访问权限”，本次已跳过。',
    fullDiskAccessBackupError:
      'PathKeep 需要“完全磁盘访问权限”才能读取浏览器历史。请到「系统设置 → 隐私与安全性 → 完全磁盘访问权限」，为 PathKeep 开启权限，然后再次运行备份。',
    fullDiskAccessOpenSettings: '打开“完全磁盘访问权限”设置',
    backupFailedAlertLabel: '备份失败，需要处理',
    backupFailedHeading: '备份未能完成',
    configSaveFailedHeading: '这项设置没有保存',
    configSaveFailedReassurance:
      '开关可能仍显示你刚才的选择，但 PathKeep 保留的是之前已保存的值。重新改一次即可重试，或复制下方的详细信息。',
    backupFailedReassurance: '你现有的存档安全无损，没有丢失任何内容。',
    backupFailedRetry: '重试',
    backupFailedCopyDiagnostics: '复制诊断信息',
    backupFailedCopied: '已复制',
    backupFailedShowDetails: '技术细节',
    backupFailedDismiss: '关闭此提示',
    backupRunFailed: '失败',
    backupRunErrorReason: '错误',
    revealLogs: '显示日志',
    revealLogsAriaLabel: '在访达中显示 PathKeep 日志文件夹',
    settingAppLockPasscode: '正在保存应用锁密码',
    settingAppLockPasscodeDetail: '写入会话密码并刷新应用锁状态。',
    setAppLockPasscodeFailed: 'PathKeep 无法保存应用锁密码。',
    clearingAppLockPasscode: '正在清除应用锁密码',
    clearingAppLockPasscodeDetail: '移除当前密码，并在这台设备上关闭应用锁。',
    clearAppLockPasscodeFailed: 'PathKeep 无法清除应用锁密码。',
    lockingApp: '正在锁定 PathKeep',
    lockingAppDetail: '隐藏存档数据，并返回锁定页面。',
    lockAppFailed: 'PathKeep 无法锁定当前应用会话。',
    unlockingApp: '正在解锁 PathKeep',
    unlockingAppDetail: '正在验证应用锁密码并恢复最新界面状态。',
    unlockAppFailed: 'PathKeep 无法解锁当前应用会话。',
    unlockErrorBiometricNotEnrolled:
      '这台设备还没有录入任何生物识别凭据。请改用应用锁密码。',
    unlockErrorBiometricLockout:
      '这台设备的生物识别解锁目前已被系统锁定。请先在系统设置中解锁，或改用应用锁密码。',
    unlockErrorBiometricUnavailable:
      '这台设备目前无法使用生物识别解锁。请改用应用锁密码。',
    unlockErrorBiometricCanceled:
      '生物识别解锁已取消。请重试，或改用应用锁密码。',
    unlockErrorBiometricTurnedOff:
      '生物识别解锁目前已在设置中关闭。请改用应用锁密码。',
    unlockErrorBiometricFailed:
      '生物识别解锁无法确认你的身份。请重试，或改用应用锁密码。',
    lockEyebrow: '应用锁',
    lockTitle: '解锁 PathKeep',
    lockDescription:
      '这个锁只保护 PathKeep 桌面会话。存档加密仍然是独立的静态数据保护层。',
    lockReason: '锁定原因',
    lockReasonStartup: '启动检查',
    lockReasonManual: '手动锁定',
    lockReasonIdleTimeout: '闲置超时',
    lockConfigPath: '锁定配置',
    lastUnlockedAt: '上次解锁',
    lockPasscodeLabel: '密码',
    lockPasscodePlaceholder: '输入应用锁密码',
    unlockApp: '解锁',
    unlockWithBiometric: '使用生物识别',
    unlockWithTouchId: '使用 Touch ID',
    unlockBiometricUnavailable:
      '当前桌面构建暂不支持生物识别，所以 PathKeep 仍使用密码作为回退方式。',
    unlockTouchIdUnavailable:
      '这台 Mac 当前无法使用 Touch ID，所以 PathKeep 仍使用密码作为回退方式。',
    lockRecoveryTitle: '忘记密码？',
    lockRecoveryBody:
      '这里无法重置密码，但你的历史记录没有丢失——应用锁只锁住这个窗口。打开配置路径，按支持文档的说明清除这把锁。',
    lockRecoveryHintBody: '恢复提示：{hint}。仍然需要正确密码才能解锁。',
    lockRecoveryAction: '打开配置路径',
    onboardingVersion: '初始设置',
    onboardingLeaveHint:
      '你可以随时离开设置。选项会自动保存，之后可以从总览或设置页继续。',
    exitSetup: '退出设置',
    // ── 纸面重设计 shell ──
    findAPage: '查找页面…',
    archiveKept: '存档已保留',
    archiving: '正在备份…',
    archiveNotInitialized: '尚未初始化存档',
    backgroundTasksRunningOne: '{count} 个后台任务进行中',
    backgroundTasksRunningMany: '{count} 个后台任务进行中',
    backgroundTaskViewActivity: '查看后台活动',
    backgroundTaskStarted: '后台工作已开始',
    backgroundTaskEnded: '后台工作已结束',
    pages: '页',
    since: '自 {year} 年 {month}',
    lastArchivedAt: '最近备份于 {time}',
    sourcesTitle: '来源',
    sourcesConnected: '已连接 {count} 个',
    sourcesAll: '所有来源',
    sourcesCountSingular: '{count} 个来源',
    sourcesCountPlural: '{count} 个来源',
    manageSources: '管理来源…',
    paletteTitle: '查找页面',
    paletteDescription: '在你的整个存档中搜索每一次访问',
    paletteEmptyHint: '开始输入即可搜索存档…',
    paletteLoading: '正在搜索…',
    paletteNoResults: '这里还什么都没有。记忆从容耐心。',
    paletteHintOpen: '打开',
    paletteHintFullSearch: '完整搜索',
    paletteHintNavigate: '导航',
    paletteHintClose: '关闭',
    epigraph1: '记忆从容耐心。',
    epigraph2: '从未失去。',
    epigraph3: '每一页都被保留。',
    epigraph4: '你曾经到过。',
    epigraph5: '存档记得。',
    epigraph6: '一座小小的图书馆，悄悄生长。',
  },
  'zh-TW': {
    savingArchiveChoices: '正在儲存封存選項',
    savingArchiveChoicesDetail:
      '正在寫入更新後的封存設定，並重新整理 shell 狀態。',
    preparingArchive: '正在準備封存',
    preparingArchiveDetail:
      '正在建立封存資料庫、套用資料庫更新，並鎖定目前的初始化選擇。',
    runningManualBackup: '正在執行手動備份',
    runningManualBackupDetail: '正在檢查所選的瀏覽器設定檔，接著寫入封存紀錄。',
    backupWritingArchive: '正在寫入封存紀錄',
    backupWritingArchiveDetail:
      '正在整理瀏覽紀錄、網址與稽核檔案。瀏覽紀錄多的瀏覽器在這一步可能會花一些時間。',
    backupProfileProgress: '{profileId}（{current}/{total}）',
    backupProgressPending: '等待備份進度',
    backupRecordProgressPending: '等待紀錄批次',
    backupRecordProgress: '已處理 {count} 筆紀錄',
    backupRecordStats: '新增 {imported} · 重複 {duplicates}',
    backupSkippedRecords: '已略過 {count} 筆',
    backupFinalizeProgress:
      '已處理 {current}/{total} 個選定設定檔，正在準備清單與快取總計。',
    refreshingArchiveViews: '正在重新整理封存檢視',
    refreshingArchiveViewsDetail:
      '正在重新載入儀表板統計、最近執行與其他 shell 畫面。',
    backupStepPrepare: '檢查所選瀏覽器設定檔',
    backupStepArchive: '寫入封存紀錄',
    backupStepRefresh: '重新整理儀表板與 shell 狀態',
    loadingLatestArchiveState: 'PathKeep 無法載入最新的封存狀態。',
    runtimeCrashNotice:
      'PathKeep 發現了最近一次崩潰報告。請到設定裡查看日誌與崩潰診斷。',
    scheduleStaleAfterUpgradeTitle: '備份排程需要重新套用',
    scheduleStaleAfterUpgradeBody:
      '目前安裝的 macOS LaunchAgent 是由舊版 PathKeep 寫入，新版 macOS 已不再允許它直接啟動。請到「設定 → 備份 → 排程」重新套用一次，新的啟動方式才會生效。',
    scheduleHealthProbeFailedTitle: 'PathKeep 無法檢查定時備份',
    scheduleHealthProbeFailedBody:
      'PathKeep 無法確認定時備份是否正常。請開啟定時備份設定查看並重試。診斷：',
    savingSettingsFailed: 'PathKeep 無法儲存更新後的設定。',
    initializeArchiveFailed: 'PathKeep 無法初始化封存。',
    initializedNotice: '封存已初始化。請在開啟自動化前先檢查第一次備份。',
    manualBackupDueWindow:
      '你的封存已經是最新的，PathKeep 跳過了這次備份。下次備份會在排程到期時執行。',
    manualBackupWriteLockDeferred:
      '另一項封存工作還在寫入，PathKeep 沒有開始這次備份。等它在活動頁裡跑完，再重新備份一次。',
    manualBackupFinished: '手動備份已完成，執行編號 #{runId}。',
    manualBackupFailed: 'PathKeep 無法完成手動備份。',
    safariFullDiskAccessBackupWarning:
      '備份執行 #{runId} 已完成可讀取的設定檔，但 Safari 仍缺少「完整磁碟取用權限」，本次已略過。',
    fullDiskAccessBackupError:
      'PathKeep 需要「完整磁碟取用權限」才能讀取瀏覽器歷史。請到「系統設定 → 隱私權與安全性 → 完整磁碟取用權限」，為 PathKeep 開啟權限，然後再次執行備份。',
    fullDiskAccessOpenSettings: '開啟完整磁碟取用權限設定',
    backupFailedAlertLabel: '備份失敗，需要處理',
    backupFailedHeading: '備份未能完成',
    configSaveFailedHeading: '這項設定沒有儲存',
    configSaveFailedReassurance:
      '控制項可能仍顯示你剛才的選擇，但 PathKeep 保留的是先前已儲存的值。重新改一次即可重試，或複製下方的詳細資訊。',
    backupFailedReassurance: '你現有的備份存檔安全無損，沒有遺失任何內容。',
    backupFailedRetry: '重試',
    backupFailedCopyDiagnostics: '複製診斷資訊',
    backupFailedCopied: '已複製',
    backupFailedShowDetails: '技術細節',
    backupFailedDismiss: '關閉此提示',
    backupRunFailed: '失敗',
    backupRunErrorReason: '錯誤',
    revealLogs: '顯示日誌',
    revealLogsAriaLabel: '在 Finder 中顯示 PathKeep 日誌資料夾',
    settingAppLockPasscode: '正在儲存應用鎖密碼',
    settingAppLockPasscodeDetail: '寫入工作階段密碼並重新整理應用鎖狀態。',
    setAppLockPasscodeFailed: 'PathKeep 無法儲存應用鎖密碼。',
    clearingAppLockPasscode: '正在清除應用鎖密碼',
    clearingAppLockPasscodeDetail: '移除目前密碼，並在這台裝置上關閉應用鎖。',
    clearAppLockPasscodeFailed: 'PathKeep 無法清除應用鎖密碼。',
    lockingApp: '正在鎖定 PathKeep',
    lockingAppDetail: '隱藏封存資料，並返回鎖定畫面。',
    lockAppFailed: 'PathKeep 無法鎖定目前的應用工作階段。',
    unlockingApp: '正在解鎖 PathKeep',
    unlockingAppDetail: '正在驗證應用鎖密碼並恢復最新畫面狀態。',
    unlockAppFailed: 'PathKeep 無法解鎖目前的應用工作階段。',
    unlockErrorBiometricNotEnrolled:
      '這台裝置尚未註冊任何生物辨識憑證。請改用應用鎖密碼。',
    unlockErrorBiometricLockout:
      '這台裝置的生物辨識解鎖目前已被系統鎖定。請先在系統設定中解鎖，或改用應用鎖密碼。',
    unlockErrorBiometricUnavailable:
      '這台裝置目前無法使用生物辨識解鎖。請改用應用鎖密碼。',
    unlockErrorBiometricCanceled:
      '生物辨識解鎖已取消。請再試一次，或改用應用鎖密碼。',
    unlockErrorBiometricTurnedOff:
      '生物辨識解鎖目前已在設定中關閉。請改用應用鎖密碼。',
    unlockErrorBiometricFailed:
      '生物辨識解鎖無法確認你的身分。請再試一次，或改用應用鎖密碼。',
    lockEyebrow: '應用鎖',
    lockTitle: '解鎖 PathKeep',
    lockDescription:
      '這個鎖只保護 PathKeep 桌面工作階段。封存加密仍然是獨立的靜態資料保護層。',
    lockReason: '鎖定原因',
    lockReasonStartup: '啟動檢查',
    lockReasonManual: '手動鎖定',
    lockReasonIdleTimeout: '閒置逾時',
    lockConfigPath: '鎖定設定',
    lastUnlockedAt: '上次解鎖',
    lockPasscodeLabel: '密碼',
    lockPasscodePlaceholder: '輸入應用鎖密碼',
    unlockApp: '解鎖',
    unlockWithBiometric: '使用生物辨識',
    unlockWithTouchId: '使用 Touch ID',
    unlockBiometricUnavailable:
      '目前桌面建置暫不支援生物辨識，所以 PathKeep 仍使用密碼作為回退方式。',
    unlockTouchIdUnavailable:
      '這台 Mac 目前無法使用 Touch ID，所以 PathKeep 仍使用密碼作為回退方式。',
    lockRecoveryTitle: '忘記密碼？',
    lockRecoveryBody:
      '這裡無法重設密碼，但你的歷史紀錄沒有遺失——應用程式鎖只鎖住這個視窗。開啟設定路徑，依照支援文件的說明清除這把鎖。',
    lockRecoveryHintBody: '恢復提示：{hint}。仍然需要正確密碼才能解鎖。',
    lockRecoveryAction: '開啟設定路徑',
    onboardingVersion: '初始設定',
    onboardingLeaveHint:
      '你可以隨時離開設定。選項會自動儲存，之後可以從總覽或設定頁繼續。',
    exitSetup: '離開設定',
    // ── 紙面重設計 shell ──
    findAPage: '尋找頁面…',
    archiveKept: '封存已保留',
    archiving: '正在封存…',
    archiveNotInitialized: '尚未初始化封存',
    backgroundTasksRunningOne: '{count} 個背景工作進行中',
    backgroundTasksRunningMany: '{count} 個背景工作進行中',
    backgroundTaskViewActivity: '查看背景活動',
    backgroundTaskStarted: '背景工作已開始',
    backgroundTaskEnded: '背景工作已結束',
    pages: '頁',
    since: '自 {year} 年 {month}',
    lastArchivedAt: '最近備份於 {time}',
    sourcesTitle: '來源',
    sourcesConnected: '已連線 {count} 個',
    sourcesAll: '所有來源',
    sourcesCountSingular: '{count} 個來源',
    sourcesCountPlural: '{count} 個來源',
    manageSources: '管理來源…',
    paletteTitle: '尋找頁面',
    paletteDescription: '在你的整個封存中搜尋每一次造訪',
    paletteEmptyHint: '開始輸入即可搜尋封存…',
    paletteLoading: '搜尋中…',
    paletteNoResults: '這裡還空空如也。記憶從容耐心。',
    paletteHintOpen: '開啟',
    paletteHintFullSearch: '完整搜尋',
    paletteHintNavigate: '瀏覽',
    paletteHintClose: '關閉',
    epigraph1: '記憶從容耐心。',
    epigraph2: '不曾失去。',
    epigraph3: '每一頁，都被保留。',
    epigraph4: '你曾經到過。',
    epigraph5: '封存記得。',
    epigraph6: '一座小小的圖書館，悄悄生長。',
  },
} as const
