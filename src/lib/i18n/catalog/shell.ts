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
      'Writing the updated archive configuration and refreshing the shell state.',
    preparingArchive: 'Preparing the archive',
    preparingArchiveDetail:
      'Creating the archive database, applying migrations, and locking in the current setup choices.',
    runningManualBackup: 'Running a manual backup',
    runningManualBackupDetail:
      'Inspecting the selected browser profiles before PathKeep writes the canonical archive run.',
    backupWritingArchive: 'Writing archive facts',
    backupWritingArchiveDetail:
      'Normalizing visits, URLs, and audit artifacts. Large real-world profiles can take a while here.',
    backupProfileProgress: '{profileId} ({current}/{total})',
    backupFinalizeProgress:
      'Processed {current} of {total} selected profiles. Preparing the manifest and cached totals.',
    refreshingArchiveViews: 'Refreshing archive views',
    refreshingArchiveViewsDetail:
      'Reloading dashboard totals, recent runs, and other shell surfaces with the latest archive state.',
    backupStepPrepare: 'Inspect selected browser profiles',
    backupStepArchive: 'Write the canonical archive run',
    backupStepRefresh: 'Refresh dashboard and shell state',
    loadingLatestArchiveState:
      'PathKeep could not load the latest archive state.',
    runtimeCrashNotice:
      'PathKeep found a recent crash report. Review the logs and crash diagnostics in Settings.',
    savingSettingsFailed:
      'PathKeep could not save the updated archive settings.',
    initializeArchiveFailed: 'PathKeep could not initialize the archive.',
    initializedNotice:
      'Archive initialized. Review the first backup before automation.',
    manualBackupDueWindow: 'The archive is still inside the due window.',
    manualBackupFinished: 'Manual backup finished as run #{runId}.',
    manualBackupFailed: 'PathKeep could not complete the manual backup.',
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
      'Verifying the app lock passcode and restoring the latest shell state.',
    unlockAppFailed: 'PathKeep could not unlock the current app session.',
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
      'PathKeep does not offer a fake recovery flow here. Open the config path and follow the support guidance if you need to reset the UI session lock.',
    lockRecoveryHintBody:
      'Recovery hint: {hint}. PathKeep still requires the passcode to unlock this session.',
    lockRecoveryAction: 'Open config path',
    onboardingVersion: 'Setup',
    onboardingLeaveHint:
      'You can leave setup at any time. Your choices are saved automatically, and you can come back from Dashboard or Settings.',
    exitSetup: 'Exit setup',
  },
  'zh-CN': {
    savingArchiveChoices: '正在保存归档选项',
    savingArchiveChoicesDetail: '正在写入更新后的归档配置，并刷新 shell 状态。',
    preparingArchive: '正在准备归档',
    preparingArchiveDetail:
      '正在创建归档数据库、应用迁移，并锁定当前的初始化选择。',
    runningManualBackup: '正在运行手动备份',
    runningManualBackupDetail:
      '正在检查所选浏览器配置文件，然后 PathKeep 才会写入 canonical archive run。',
    backupWritingArchive: '正在写入归档事实',
    backupWritingArchiveDetail:
      '正在规范化 visits、URL 和审计工件。真实的大型配置文件在这一步可能会花一些时间。',
    backupProfileProgress: '{profileId}（{current}/{total}）',
    backupFinalizeProgress:
      '已处理 {current}/{total} 个选定配置文件，正在准备 manifest 与缓存总计。',
    refreshingArchiveViews: '正在刷新归档视图',
    refreshingArchiveViewsDetail:
      '正在重新加载仪表盘统计、最近运行和其他 shell 视图。',
    backupStepPrepare: '检查所选浏览器配置文件',
    backupStepArchive: '写入 canonical archive run',
    backupStepRefresh: '刷新仪表盘与 shell 状态',
    loadingLatestArchiveState: 'PathKeep 无法加载最新的归档状态。',
    runtimeCrashNotice:
      'PathKeep 发现了最近一次崩溃报告。请到设置里查看日志和崩溃诊断。',
    savingSettingsFailed: 'PathKeep 无法保存更新后的设置。',
    initializeArchiveFailed: 'PathKeep 无法初始化归档。',
    initializedNotice: '归档已初始化。请在开启自动化前先检查第一次备份。',
    manualBackupDueWindow: '归档仍处于未到期窗口内。',
    manualBackupFinished: '手动备份已完成，运行编号 #{runId}。',
    manualBackupFailed: 'PathKeep 无法完成手动备份。',
    settingAppLockPasscode: '正在保存应用锁密码',
    settingAppLockPasscodeDetail: '写入会话密码并刷新应用锁状态。',
    setAppLockPasscodeFailed: 'PathKeep 无法保存应用锁密码。',
    clearingAppLockPasscode: '正在清除应用锁密码',
    clearingAppLockPasscodeDetail: '移除当前密码，并在这台设备上关闭应用锁。',
    clearAppLockPasscodeFailed: 'PathKeep 无法清除应用锁密码。',
    lockingApp: '正在锁定 PathKeep',
    lockingAppDetail: '隐藏归档数据，并返回锁定页面。',
    lockAppFailed: 'PathKeep 无法锁定当前应用会话。',
    unlockingApp: '正在解锁 PathKeep',
    unlockingAppDetail: '正在验证应用锁密码并恢复最新界面状态。',
    unlockAppFailed: 'PathKeep 无法解锁当前应用会话。',
    lockEyebrow: '应用锁',
    lockTitle: '解锁 PathKeep',
    lockDescription:
      '这个锁只保护 PathKeep 桌面会话。归档加密仍然是独立的静态数据保护层。',
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
      'PathKeep 不会在这里提供假的恢复流程。打开配置路径，并按照支持文档重置 UI 会话锁。',
    lockRecoveryHintBody: '恢复提示：{hint}。仍然需要正确密码才能解锁。',
    lockRecoveryAction: '打开配置路径',
    onboardingVersion: '初始设置',
    onboardingLeaveHint:
      '你可以随时离开设置。选项会自动保存，之后可以从总览或设置页继续。',
    exitSetup: '退出设置',
  },
  'zh-TW': {
    savingArchiveChoices: '正在儲存歸檔選項',
    savingArchiveChoicesDetail: '正在寫入更新後的歸檔設定，並刷新 shell 狀態。',
    preparingArchive: '正在準備歸檔',
    preparingArchiveDetail:
      '正在建立歸檔資料庫、套用 migration，並鎖定目前的初始化選擇。',
    runningManualBackup: '正在執行手動備份',
    runningManualBackupDetail:
      '正在檢查所選的瀏覽器設定檔，接著 PathKeep 才會寫入 canonical archive run。',
    backupWritingArchive: '正在寫入歸檔 facts',
    backupWritingArchiveDetail:
      '正在正規化 visits、URL 與審計工件。真實的大型設定檔在這一步可能會花一些時間。',
    backupProfileProgress: '{profileId}（{current}/{total}）',
    backupFinalizeProgress:
      '已處理 {current}/{total} 個選定設定檔，正在準備 manifest 與快取總計。',
    refreshingArchiveViews: '正在刷新歸檔視圖',
    refreshingArchiveViewsDetail:
      '正在重新載入儀表板統計、最近執行與其他 shell 畫面。',
    backupStepPrepare: '檢查所選瀏覽器設定檔',
    backupStepArchive: '寫入 canonical archive run',
    backupStepRefresh: '刷新儀表板與 shell 狀態',
    loadingLatestArchiveState: 'PathKeep 無法載入最新的歸檔狀態。',
    runtimeCrashNotice:
      'PathKeep 發現了最近一次崩潰報告。請到設定裡查看日誌與崩潰診斷。',
    savingSettingsFailed: 'PathKeep 無法儲存更新後的設定。',
    initializeArchiveFailed: 'PathKeep 無法初始化歸檔。',
    initializedNotice: '歸檔已初始化。請在開啟自動化前先檢查第一次備份。',
    manualBackupDueWindow: '歸檔仍處於未到期窗口內。',
    manualBackupFinished: '手動備份已完成，執行編號 #{runId}。',
    manualBackupFailed: 'PathKeep 無法完成手動備份。',
    settingAppLockPasscode: '正在儲存應用鎖密碼',
    settingAppLockPasscodeDetail: '寫入會話密碼並刷新應用鎖狀態。',
    setAppLockPasscodeFailed: 'PathKeep 無法儲存應用鎖密碼。',
    clearingAppLockPasscode: '正在清除應用鎖密碼',
    clearingAppLockPasscodeDetail: '移除目前密碼，並在這台裝置上關閉應用鎖。',
    clearAppLockPasscodeFailed: 'PathKeep 無法清除應用鎖密碼。',
    lockingApp: '正在鎖定 PathKeep',
    lockingAppDetail: '隱藏封存資料，並返回鎖定畫面。',
    lockAppFailed: 'PathKeep 無法鎖定目前的應用會話。',
    unlockingApp: '正在解鎖 PathKeep',
    unlockingAppDetail: '正在驗證應用鎖密碼並恢復最新畫面狀態。',
    unlockAppFailed: 'PathKeep 無法解鎖目前的應用會話。',
    lockEyebrow: '應用鎖',
    lockTitle: '解鎖 PathKeep',
    lockDescription:
      '這個鎖只保護 PathKeep 桌面會話。封存加密仍然是獨立的靜態資料保護層。',
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
      'PathKeep 不會在這裡提供假的恢復流程。開啟設定路徑，並依照支援文件重設 UI 會話鎖。',
    lockRecoveryHintBody: '恢復提示：{hint}。仍然需要正確密碼才能解鎖。',
    lockRecoveryAction: '開啟設定路徑',
    onboardingVersion: '初始設定',
    onboardingLeaveHint:
      '你可以隨時離開設定。選項會自動儲存，之後可以從總覽或設定頁繼續。',
    exitSetup: '離開設定',
  },
} as const
