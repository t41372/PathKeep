/**
 * @file onboarding.ts
 * @description Owns onboarding setup flow copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `onboarding` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so onboarding wording stays separate from setup side effects.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `onboarding` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const onboardingNamespaceCatalog = {
  en: {
    stepWelcome: 'Welcome',
    stepBrowsers: 'Browsers',
    stepStorage: 'Storage',
    stepSecurity: 'Security',
    stepSchedule: 'Schedule',
    stepReady: 'Ready',
    loadingDecisions: 'Loading setup…',
    errorTitle: 'Setup data unavailable',
    emptyEyebrow: 'SETUP',
    emptyTitle: 'Getting things ready…',
    emptyDescription:
      'PathKeep is preparing your setup options. This should only take a moment.',
    schedulePreviewFallbackError:
      "Couldn't preview the schedule. You can set this up later in Settings.",
    errorSelectProfile: 'Pick at least one browser profile to back up.',
    errorNeedPassword: 'Enter a master password to use encrypted mode.',
    errorPasswordMismatch: "Passwords don't match. Try again.",
    errorFinishFailed: 'Something went wrong during setup. You can try again.',
    welcomeTagline1: 'Your browsing history is yours.',
    welcomeTagline2: 'Back it up. Search it. Learn from it.',
    featureBackupTitle: 'AUTOMATIC BACKUP',
    featureBackupDesc:
      'Start with Google Chrome today, plus Safari on macOS after Full Disk Access is granted. Other browser adapters may appear in setup before they become public support commitments.',
    featureSearchTitle: 'POWERFUL SEARCH',
    featureSearchDesc:
      'Search across your entire history, even years back. Find any page you ever visited by keyword or natural language.',
    featureInsightsTitle: 'INTELLIGENCE LAYERS',
    featureInsightsDesc:
      'Start with local Core Intelligence for patterns, trails, and refinds. Add AI later if you want semantic search and assistant workflows.',
    trustLocalFirst: 'Local-first — your data never leaves your machine',
    trustOpenSource: 'Open-source — GPL v3, inspect the code yourself',
    trustBuiltWith: 'Built with Tauri + Rust + SQLite',
    versionLine: 'v{version} · Tauri desktop app',
    beginSetup: 'Get Started →',
    browserDetectionTitle: 'Choose Your Browsers',
    browserDetectionDesc:
      'We found browser profiles on this system. Choose which ones PathKeep should inspect before your first backup review.',
    scanStatus: '{count} profiles found · {selected} selected',
    detectedProfiles: 'YOUR BROWSERS',
    found: '{count} found',
    historyFound: 'READY',
    actionRequired: 'NEEDS ATTENTION',
    versionUnknown: 'Version unknown',
    browserEngineLabel: '{version} · {engine} engine',
    safariAccessHint:
      'Safari needs Full Disk Access permission. Open System Settings → Privacy & Security → Full Disk Access.',
    cannotReadHint: "Can't read {fileName} yet. Check file permissions.",
    firefoxSafariInfo:
      'Google Chrome is part of the validated setup path today. Safari joins that path on macOS after Full Disk Access is granted. Firefox-family and other adapters may appear here before they become public support commitments.',
    backButton: '← Back',
    continueButton: 'Continue →',
    storageTitle: 'Where Your Data Lives',
    storageDesc:
      "Everything is stored locally on your computer. Here's where the archive will be saved.",
    archiveRoot: 'ARCHIVE LOCATION',
    localFirst: 'Local only',
    sizeEstimates: 'ESTIMATED SIZE',
    projected: 'Estimated',
    estimateArchiveDb: 'Archive database',
    estimateManifest: 'Audit logs',
    estimateSnapshots: 'Raw snapshots',
    estimateTotal: 'Total',
    estimateExplanation:
      'Based on {count} readable profiles ({source} detected browser data), plus archive and audit overhead.',
    securityTitle: 'Encryption',
    securityDesc:
      'Would you like to encrypt your archive? You can change this later.',
    encryptionModeLabel: 'Encryption mode',
    encryptedOption: 'Encrypted',
    encryptedSelectLabel: 'Use encryption',
    recommended: 'RECOMMENDED',
    encryptedDesc:
      "Your archive is encrypted with AES-256. You'll need a master password to access it.",
    masterPasswordLabel: 'MASTER PASSWORD',
    masterPasswordPlaceholder: 'Enter a password',
    confirmPasswordLabel: 'CONFIRM PASSWORD',
    confirmPasswordPlaceholder: 'Enter the same password again',
    storeInKeyring: 'Remember password in system keychain',
    plaintextOption: 'No encryption',
    plaintextSelectLabel: 'Skip encryption',
    plaintextDesc:
      'No encryption. Choose this only if your disk is already encrypted (e.g., FileVault, BitLocker).',
    tradeoffNoPassword: 'No password needed',
    tradeoffEasyInspect: 'Database can be opened with any SQLite tool',
    tradeoffVisible: 'Anyone with access to your files can read your history',
    tradeoffNoUpgrade: 'Switching to encrypted later requires a re-key',
    passwordWarningTitle: 'There is no password recovery.',
    passwordWarningBody:
      'If you forget your master password, your archive cannot be recovered. PathKeep has no backdoor and does not store your password. Write it down somewhere safe.',
    scheduleTitle: 'Backup Schedule',
    scheduleDesc:
      "How often should PathKeep check for new history? It will only back up when there's new data.",
    backupInterval: 'CHECK EVERY',
    selectHours: 'Hours between checks',
    intervalChipLabel: '{hours}h',
    previewingSchedule: 'Generating schedule preview…',
    schedulePreview: 'Preview',
    scheduleManualStepLaunchAgentSave:
      'Save the plist to ~/Library/LaunchAgents/{label}.plist.',
    scheduleManualStepLaunchAgentBootstrap:
      'Run `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/{label}.plist` to load the new schedule.',
    scheduleManualStepLaunchAgentReviewInstalled:
      'Open the desktop app to review the LaunchAgent file and install status.',
    scheduleManualStepLaunchAgentRemove:
      'Remove the LaunchAgent if you no longer want automatic backups.',
    scheduleManualStepWindowsSaveXml:
      'Save the XML file and import it in Task Scheduler.',
    scheduleManualStepWindowsCreateTask:
      'Alternatively run `schtasks /Create /TN {label} /XML {label}.task.xml`.',
    scheduleManualStepLinuxCopy: 'Copy the files to ~/.config/systemd/user/.',
    scheduleManualStepLinuxReload: 'Run `systemctl --user daemon-reload`.',
    scheduleManualStepLinuxEnable:
      'Run `systemctl --user enable --now {label}.timer`.',
    scheduleManualStepLinuxVerify:
      'Run `systemctl --user list-timers {label}.timer` to verify the next scheduled run.',
    readyTitle: 'All Set',
    readyDesc: 'Review your choices below, then start the first backup.',
    configSummary: 'YOUR CHOICES',
    reviewBeforeInit: 'Review',
    configProfiles: 'Browsers',
    configProfilesValue: '{count} profiles selected',
    configStorage: 'Storage',
    configEncryption: 'Encryption',
    configSchedule: 'Schedule',
    configScheduleValue: 'Every {hours} hours',
    initSteps: 'WHAT HAPPENS NEXT',
    whatHappensNext: 'When you click the button below',
    initStep1Action: 'Create the archive',
    initStep1DetailEncrypted: 'Encrypted with SQLCipher',
    initStep1DetailPlaintext: 'Unencrypted SQLite',
    initStep2Action: 'Save your settings',
    initStep2Detail: 'Starts the audit trail',
    initStep3Action: 'Run the first backup',
    initStep3Detail: 'Back up history from {count} profile{plural}',
    initButton: 'Create Archive & Back Up →',
  },
  'zh-CN': {
    stepWelcome: '欢迎',
    stepBrowsers: '浏览器',
    stepStorage: '存储',
    stepSecurity: '加密',
    stepSchedule: '定时',
    stepReady: '完成',
    loadingDecisions: '加载中…',
    errorTitle: '设置数据不可用',
    emptyEyebrow: '设置',
    emptyTitle: '正在准备…',
    emptyDescription: 'PathKeep 正在准备设置选项，稍等一下就好。',
    schedulePreviewFallbackError:
      '暂时无法预览定时备份，你可以稍后在设置中配置。',
    errorSelectProfile: '请至少选择一个浏览器来备份。',
    errorNeedPassword: '选择加密模式需要设置密码。',
    errorPasswordMismatch: '两次输入的密码不一致，请重试。',
    errorFinishFailed: '设置过程出错了，可以再试一次。',
    welcomeTagline1: '你的浏览历史属于你。',
    welcomeTagline2: '备份它，搜索它，从中发现规律。',
    featureBackupTitle: '自动备份',
    featureBackupDesc:
      '当前先以 Google Chrome 作为已验证路径；在 macOS 上授予完全磁盘访问权限后，也可验证 Safari 的基础备份。其他浏览器适配器可能会先出现在设置里，但还不算公开支持承诺。',
    featureSearchTitle: '强大的搜索',
    featureSearchDesc:
      '搜索你所有的浏览历史，哪怕是几年前访问的页面，也能通过关键词或自然语言找到。',
    featureInsightsTitle: '智能分析分层',
    featureInsightsDesc:
      '先用本地 Core Intelligence 看规律、搜索旅程和重找页面；之后如果需要，再开启 AI 做语义搜索和助手工作流。',
    trustLocalFirst: '本地优先 — 数据只在你的电脑上',
    trustOpenSource: '开源 — GPL v3 协议，代码完全公开',
    trustBuiltWith: '基于 Tauri + Rust + SQLite',
    versionLine: 'v{version} · Tauri 桌面应用',
    beginSetup: '开始设置 →',
    browserDetectionTitle: '选择浏览器',
    browserDetectionDesc:
      '我们在这台设备上找到了浏览器 profile，选择要纳入首次备份审查的来源。',
    scanStatus: '找到 {count} 个 · 已选 {selected} 个',
    detectedProfiles: '你的浏览器',
    found: '找到 {count} 个',
    historyFound: '就绪',
    actionRequired: '需要处理',
    versionUnknown: '版本未知',
    browserEngineLabel: '{version} · {engine} 内核',
    safariAccessHint:
      'Safari 需要完全磁盘访问权限。打开系统设置 → 隐私与安全性 → 完全磁盘访问权限。',
    cannotReadHint: '暂时无法读取 {fileName}，请检查文件权限。',
    firefoxSafariInfo:
      '当前公开验证的设置路径是 Google Chrome；在 macOS 上授予完全磁盘访问权限后，Safari 也属于已验证的基础支持。Firefox 系和其他适配器可能会先显示在这里，但还不算公开支持承诺。',
    backButton: '← 返回',
    continueButton: '继续 →',
    storageTitle: '数据存储位置',
    storageDesc: '所有数据都存在你的电脑上，以下是存档保存的位置。',
    archiveRoot: '存档位置',
    localFirst: '仅限本地',
    sizeEstimates: '预估大小',
    projected: '预估',
    estimateArchiveDb: '存档数据库',
    estimateManifest: '审计日志',
    estimateSnapshots: '原始快照',
    estimateTotal: '合计',
    estimateExplanation:
      '基于 {count} 个可读取的浏览器配置（检测到 {source} 浏览器数据），再加上存档和审计开销估算。',
    securityTitle: '加密',
    securityDesc: '是否加密存档？之后可以在设置中修改。',
    encryptionModeLabel: '加密模式',
    encryptedOption: '加密',
    encryptedSelectLabel: '使用加密',
    recommended: '推荐',
    encryptedDesc: 'AES-256 加密，每次访问需要输入密码。',
    masterPasswordLabel: '密码',
    masterPasswordPlaceholder: '输入密码',
    confirmPasswordLabel: '确认密码',
    confirmPasswordPlaceholder: '再次输入密码',
    storeInKeyring: '保存密码到系统钥匙串，免去每次输入',
    plaintextOption: '不加密',
    plaintextSelectLabel: '跳过加密',
    plaintextDesc:
      '不加密数据库。仅在你的磁盘已有加密保护（如 FileVault、BitLocker）时选择。',
    tradeoffNoPassword: '不需要密码',
    tradeoffEasyInspect: '可以用任何 SQLite 工具查看',
    tradeoffVisible: '能访问文件的人都能看到你的浏览历史',
    tradeoffNoUpgrade: '之后切换到加密需要重新设置密码',
    passwordWarningTitle: '密码无法找回。',
    passwordWarningBody:
      '忘记密码意味着数据无法恢复。PathKeep 不保存密码，也没有后门。请务必把密码记在安全的地方。',
    scheduleTitle: '定时备份',
    scheduleDesc: '多久检查一次新的浏览历史？只有发现新数据时才会执行备份。',
    backupInterval: '检查间隔',
    selectHours: '小时数',
    intervalChipLabel: '{hours} 小时',
    previewingSchedule: '生成定时备份预览…',
    schedulePreview: '预览',
    scheduleManualStepLaunchAgentSave:
      '将 plist 保存到 ~/Library/LaunchAgents/{label}.plist。',
    scheduleManualStepLaunchAgentBootstrap:
      '运行 `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/{label}.plist` 以加载新的计划。',
    scheduleManualStepLaunchAgentReviewInstalled:
      '打开桌面应用，检查 LaunchAgent 文件和安装状态。',
    scheduleManualStepLaunchAgentRemove:
      '如果你不再需要自动备份，请移除这个 LaunchAgent。',
    scheduleManualStepWindowsSaveXml:
      '保存 XML 文件，然后在任务计划程序中导入。',
    scheduleManualStepWindowsCreateTask:
      '也可以运行 `schtasks /Create /TN {label} /XML {label}.task.xml`。',
    scheduleManualStepLinuxCopy: '把这些文件复制到 ~/.config/systemd/user/。',
    scheduleManualStepLinuxReload: '运行 `systemctl --user daemon-reload`。',
    scheduleManualStepLinuxEnable:
      '运行 `systemctl --user enable --now {label}.timer`。',
    scheduleManualStepLinuxVerify:
      '运行 `systemctl --user list-timers {label}.timer`，确认下一次计划执行时间。',
    readyTitle: '一切就绪',
    readyDesc: '检查下方的设置，然后开始首次备份。',
    configSummary: '你的设置',
    reviewBeforeInit: '检查',
    configProfiles: '浏览器',
    configProfilesValue: '已选 {count} 个',
    configStorage: '存储',
    configEncryption: '加密',
    configSchedule: '定时备份',
    configScheduleValue: '每 {hours} 小时',
    initSteps: '接下来会发生什么',
    whatHappensNext: '点击下方按钮后',
    initStep1Action: '创建存档',
    initStep1DetailEncrypted: '使用 SQLCipher 加密',
    initStep1DetailPlaintext: '不加密',
    initStep2Action: '保存设置',
    initStep2Detail: '同时创建审计日志',
    initStep3Action: '运行首次备份',
    initStep3Detail: '从 {count} 个浏览器备份历史记录',
    initButton: '创建存档并备份 →',
  },
  'zh-TW': {
    stepWelcome: '歡迎',
    stepBrowsers: '瀏覽器',
    stepStorage: '儲存',
    stepSecurity: '加密',
    stepSchedule: '定時',
    stepReady: '完成',
    loadingDecisions: '載入中…',
    errorTitle: '設定資料無法使用',
    emptyEyebrow: '設定',
    emptyTitle: '正在準備…',
    emptyDescription: 'PathKeep 正在準備設定選項，稍等一下就好。',
    schedulePreviewFallbackError:
      '暫時無法預覽定時備份，你可以稍後在設定中調整。',
    errorSelectProfile: '請至少選擇一個瀏覽器來備份。',
    errorNeedPassword: '選擇加密模式需要設定密碼。',
    errorPasswordMismatch: '兩次輸入的密碼不一致，請重試。',
    errorFinishFailed: '設定過程出了問題，可以再試一次。',
    welcomeTagline1: '你的瀏覽歷史屬於你。',
    welcomeTagline2: '備份它，搜尋它，從中發現規律。',
    featureBackupTitle: '自動備份',
    featureBackupDesc:
      '目前先以 Google Chrome 作為已驗證路徑；在 macOS 上授予完整磁碟取用權限後，也可驗證 Safari 的基礎備份。其他瀏覽器 adapter 可能會先出現在設定裡，但還不算公開支援承諾。',
    featureSearchTitle: '強大的搜尋',
    featureSearchDesc:
      '搜尋你所有的瀏覽歷史，即使是好幾年前看過的頁面，也能透過關鍵字或自然語言找到。',
    featureInsightsTitle: '智慧分析分層',
    featureInsightsDesc:
      '先用本機 Core Intelligence 看規律、搜尋旅程和重找頁面；之後如果需要，再開啟 AI 做語意搜尋和助手工作流。',
    trustLocalFirst: '本地優先 — 資料只在你的電腦上',
    trustOpenSource: '開源 — GPL v3 授權，程式碼完全公開',
    trustBuiltWith: '基於 Tauri + Rust + SQLite',
    versionLine: 'v{version} · Tauri 桌面應用',
    beginSetup: '開始設定 →',
    browserDetectionTitle: '選擇瀏覽器',
    browserDetectionDesc:
      '我們在這台裝置上找到了瀏覽器 profile，選擇要納入首次備份審查的來源。',
    scanStatus: '找到 {count} 個 · 已選 {selected} 個',
    detectedProfiles: '你的瀏覽器',
    found: '找到 {count} 個',
    historyFound: '就緒',
    actionRequired: '需要處理',
    versionUnknown: '版本未知',
    browserEngineLabel: '{version} · {engine} 核心',
    safariAccessHint:
      'Safari 需要完整磁碟取用權限。前往系統設定 → 隱私權與安全性 → 完整磁碟取用權限。',
    cannotReadHint: '暫時無法讀取 {fileName}，請確認檔案權限。',
    firefoxSafariInfo:
      '目前公開驗證的設定路徑是 Google Chrome；在 macOS 上授予完整磁碟取用權限後，Safari 也屬於已驗證的基礎支援。Firefox 系與其他 adapter 可能會先顯示在這裡，但還不算公開支援承諾。',
    backButton: '← 返回',
    continueButton: '繼續 →',
    storageTitle: '資料儲存位置',
    storageDesc: '所有資料都存在你的電腦上，以下是封存儲存的位置。',
    archiveRoot: '封存位置',
    localFirst: '僅限本機',
    sizeEstimates: '預估大小',
    projected: '預估',
    estimateArchiveDb: '封存資料庫',
    estimateManifest: '稽核日誌',
    estimateSnapshots: '原始快照',
    estimateTotal: '合計',
    estimateExplanation:
      '根據 {count} 個可讀取的瀏覽器設定檔（偵測到 {source} 瀏覽器資料），再加上封存與稽核開銷估算。',
    securityTitle: '加密',
    securityDesc: '是否加密封存？之後可以在設定中修改。',
    encryptionModeLabel: '加密模式',
    encryptedOption: '加密',
    encryptedSelectLabel: '使用加密',
    recommended: '推薦',
    encryptedDesc: 'AES-256 加密，每次存取需要輸入密碼。',
    masterPasswordLabel: '密碼',
    masterPasswordPlaceholder: '輸入密碼',
    confirmPasswordLabel: '確認密碼',
    confirmPasswordPlaceholder: '再次輸入密碼',
    storeInKeyring: '儲存密碼到系統鑰匙圈，免去每次輸入',
    plaintextOption: '不加密',
    plaintextSelectLabel: '略過加密',
    plaintextDesc:
      '不加密資料庫。僅在你的磁碟已有加密保護（如 FileVault、BitLocker）時選擇。',
    tradeoffNoPassword: '不需要密碼',
    tradeoffEasyInspect: '可以用任何 SQLite 工具查看',
    tradeoffVisible: '能存取檔案的人都能看到你的瀏覽歷史',
    tradeoffNoUpgrade: '之後切換到加密需要重新設定密碼',
    passwordWarningTitle: '密碼無法找回。',
    passwordWarningBody:
      '忘記密碼代表資料無法復原。PathKeep 不儲存密碼，也沒有後門。請務必把密碼記在安全的地方。',
    scheduleTitle: '定時備份',
    scheduleDesc: '多久檢查一次新的瀏覽歷史？只有發現新資料時才會執行備份。',
    backupInterval: '檢查間隔',
    selectHours: '小時數',
    intervalChipLabel: '{hours} 小時',
    previewingSchedule: '產生定時備份預覽…',
    schedulePreview: '預覽',
    scheduleManualStepLaunchAgentSave:
      '將 plist 儲存到 ~/Library/LaunchAgents/{label}.plist。',
    scheduleManualStepLaunchAgentBootstrap:
      '執行 `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/{label}.plist` 以載入新的排程。',
    scheduleManualStepLaunchAgentReviewInstalled:
      '打開桌面應用，檢查 LaunchAgent 檔案與安裝狀態。',
    scheduleManualStepLaunchAgentRemove:
      '如果你不再需要自動備份，請移除這個 LaunchAgent。',
    scheduleManualStepWindowsSaveXml: '儲存 XML 檔案，然後在工作排程器中匯入。',
    scheduleManualStepWindowsCreateTask:
      '也可以執行 `schtasks /Create /TN {label} /XML {label}.task.xml`。',
    scheduleManualStepLinuxCopy: '把這些檔案複製到 ~/.config/systemd/user/。',
    scheduleManualStepLinuxReload: '執行 `systemctl --user daemon-reload`。',
    scheduleManualStepLinuxEnable:
      '執行 `systemctl --user enable --now {label}.timer`。',
    scheduleManualStepLinuxVerify:
      '執行 `systemctl --user list-timers {label}.timer`，確認下一次排程執行時間。',
    readyTitle: '一切就緒',
    readyDesc: '檢查下方的設定，然後開始首次備份。',
    configSummary: '你的設定',
    reviewBeforeInit: '檢查',
    configProfiles: '瀏覽器',
    configProfilesValue: '已選 {count} 個',
    configStorage: '儲存',
    configEncryption: '加密',
    configSchedule: '定時備份',
    configScheduleValue: '每 {hours} 小時',
    initSteps: '接下來會發生什麼',
    whatHappensNext: '點擊下方按鈕後',
    initStep1Action: '建立封存',
    initStep1DetailEncrypted: '使用 SQLCipher 加密',
    initStep1DetailPlaintext: '不加密',
    initStep2Action: '儲存設定',
    initStep2Detail: '同時建立稽核日誌',
    initStep3Action: '執行首次備份',
    initStep3Detail: '從 {count} 個瀏覽器備份歷史紀錄',
    initButton: '建立封存並備份 →',
  },
} as const
