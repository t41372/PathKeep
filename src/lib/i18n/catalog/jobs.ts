/**
 * @file jobs.ts
 * @description Owns jobs and background work queue copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `jobs` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve keys used by sidebar, shell-data, notifications, and other pages.
 * - Provide Activity center copy for the redesigned background-tasks page.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so queue wording stays separate from runtime polling and mutations.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `jobs` namespace payload for the shipped locales.
 */
export const jobsNamespaceCatalog = {
  en: {
    // ── Gates / setup ──────────────────────────────────────────────────
    statusEyebrow: 'BACKGROUND WORK',
    setupTitle: 'Background jobs start after setup',
    setupDescription:
      'Finish setup and run your first backup. Queue activity, logs, and recovery controls will appear here.',
    lockedTitle: 'Unlock the archive to review background work',
    lockedEyebrow: 'ARCHIVE LOCKED',
    lockedDetail:
      'PathKeep cannot load job history or resume queued work until the archive is unlocked.',
    refresh: 'Refresh',
    pauseQueue: 'Pause background work',
    resumeQueue: 'Resume background work',
    pageUnavailableTitle: 'Background work is unavailable',

    // ── Queue state labels (used by sidebar + tests) ───────────────────
    now: 'just now',
    jobStateQueued: 'Queued',
    jobStateRunning: 'Running',
    jobStateSucceeded: 'Completed',
    jobStateFailed: 'Failed',
    jobStateCancelled: 'Cancelled',
    jobStatePaused: 'Paused',
    jobStateStale: 'Needs replay',

    // ── Job mutation labels ────────────────────────────────────────────
    retryJob: 'Retry',
    cancelJob: 'Cancel',

    // ── Errors ────────────────────────────────────────────────────────
    errorPdf:
      'This page returned a PDF, so PathKeep skipped readable-text extraction.',
    errorUnsupportedContent:
      'This page did not return readable HTML, so PathKeep skipped page-text extraction.',
    errorRedirectBlocked:
      'This page redirected into a sign-in or redirect boundary, so PathKeep could not keep a stable readable copy.',
    errorRateLimited:
      'The upstream site temporarily refused the fetch. Retry it later.',

    // ── Runtime summaries (used by intelligence-presentation.ts) ───────
    deterministicRuntimeSummary:
      'PathKeep is refreshing local analysis first; any site-content fetches run separately in the background.',
    contentFetchQueuedSummary: 'Queued to fetch site content.',
    contentFetchQueuedSummaryHost: 'Queued to fetch site content for {host}.',
    contentFetchRunningSummary: 'Fetching site content.',
    contentFetchRunningSummaryHost: 'Fetching site content for {host}.',

    // ── Content fetch section (used by job panels / settings) ──────────
    contentFetchTitle: 'Site content',
    contentFetchDeferredBadge: 'Coming in v0.3',
    contentFetchDeferredBody:
      'Webpage body fetching is tracked for v0.3 and is not available in v0.2.0. This area stays visible for the roadmap, but PathKeep is not revisiting pages or saving readable copies yet.',
    contentFetchBacklogBody:
      'PathKeep saved {stored} readable pages. {queued} page-text fetches are waiting so local analysis does not wait on every site.',
    contentFetchRunningBody:
      'PathKeep already saved {stored} readable pages and is still fetching more page text in the background.',
    contentFetchReadyBody:
      'PathKeep already saved {stored} readable pages and the fetch queue is currently clear.',
    contentFetchOffBody:
      'Site content fetching is available but off — PathKeep is not contacting any site. Turn it on in Settings to enrich the pages you care about.',
    contentFetchOpenSettings: 'Turn on in Settings',

    // ── Sidebar ────────────────────────────────────────────────────────
    sidebarTitle: 'Background work',
    sidebarNeedsSetup: 'Background work appears after setup.',
    sidebarLocked: 'Unlock the archive first',
    sidebarLockedDetail: 'Open Security before reviewing queued work.',
    sidebarUnavailable: 'Background work is unavailable',
    sidebarPaused: '{queued} queued · paused',
    sidebarRunning: '{running} running · {queued} queued',
    sidebarFailed: '{failed} need review',
    sidebarQueued: '{queued} queued',
    sidebarIdle: 'All caught up',
    sidebarIdleDetail: 'No queued background work.',
    sidebarLastActivity: 'Last activity {relative}',
    sidebarOpenSecurity: 'Security',
    sidebarOpenJobs: 'Open Jobs',
    openJobs: 'Activity',

    // ── Archive tasks (notifications + jobs page) ──────────────────────
    archiveTaskAlreadyRunningTitle: 'Archive task already running',
    archiveTaskAlreadyRunningBody:
      '{task} is still writing archive records. Open Background Jobs to follow the live progress.',
    importTakeoutTaskTitle: 'Import Google Takeout',
    importBrowserTaskTitle: 'Import browser history',
    importTaskStartedTitle: 'Import started',
    importTaskStartedBody: 'Writing archive records from {source}.',
    importTaskCompleteTitle: 'Import finished',
    importTaskCompleteBody:
      '{imported} new record(s) written · {duplicates} duplicate(s) skipped.',
    backupTaskTitle: 'Manual backup',
    backupTaskStartedTitle: 'Backup started',
    backupTaskStartedBody:
      'PathKeep is reading selected browser profiles and writing archive records.',
    backupTaskCompleteTitle: 'Backup finished',
    archiveTaskFailedTitle: 'Archive task failed',
    archiveTaskStaleTitle: 'Interrupted archive task',
    archiveTaskStaleBody:
      'This run was still marked running when PathKeep last loaded the ledger. Live progress cannot be resumed after restart.',
    archiveTaskStarted: 'Started',
    archiveTaskUpdated: 'Updated',
    archiveTaskRecords: 'records',
    archiveTaskConsole: 'Console log',
    archiveTaskNoLogs:
      'Waiting for the next progress event. Large browser profiles can spend a while copying and validating before record counts appear.',
    archiveTaskOpenResult: 'Open result',
    archiveTaskOpenJobs: 'Open Jobs',
    archiveTasksTitle: 'Archive writes',
    archiveTasksBody:
      'Imports and backups stay visible here while they write archive records, even if you leave the starting page.',

    // ── Counts (sidebar + runtime-digest) ─────────────────────────────
    runningCount: 'Running',

    // ── Error details (intelligence-presentation.ts) ───────────────────
    noErrorDetails: 'No details yet.',

    // ── Queue summary body (jobs-no-stale-v03 test) ────────────────────
    queueSummaryBody:
      'Optional assistant and embedding work is available but off by default — enable it in Settings. Queued items show their live counts below and stay reviewable here.',

    // ── Status labels (keep for sidebar/tests) ────────────────────────
    pausedBody: 'Queued work is saved and will stay here until you resume it.',
    pausedTitle: 'Background work is paused',
    failedBody:
      'Failed jobs stay here with their error details so you can retry or cancel them.',
    failedTitle: 'Some background work needs review',
    runningBody:
      'PathKeep is processing queued work in the background. You can keep using the app.',
    runningTitle: 'Background work is running',
    queuedBody:
      'Queued work is waiting for an available worker slot or for you to resume the queue.',
    queuedTitle: 'Background work is queued',
    readyBody:
      'No work is waiting. Local analysis refreshes and archive write tasks will appear here.',
    readyTitle: 'Background work is idle',

    // ── Activity center ────────────────────────────────────────────────
    activityPageTitle: 'Activity',

    // Header summaries
    headerSummaryFailed: '{failed} need attention · {running} running',
    headerSummaryFailedIdle: '{failed} need attention · idle',
    headerSummaryRunning: '{running} running · safe to close',
    headerSummaryRunningNotSafe: '{running} running',
    headerSummaryRunningWaiting: '{running} running · {queued} waiting',
    headerSummaryPausedQueued: 'Queue paused · {queued} waiting',
    headerSummaryQueued: '{queued} waiting',
    headerSummaryAllCaughtUp: 'All caught up · last activity {time}',
    headerSummaryNoActivity: 'All caught up · no recent activity',

    // Task names
    taskIndexBuild: 'Building smart-search index',
    taskModelDownload: 'Downloading embedding model',
    taskContentFetch: 'Fetching site content',
    taskReEmbed: 'Re-embedding history',
    taskDeterministicRebuild: 'Refreshing analysis',
    taskImportRunning: 'Importing history',
    taskImportStale: 'Importing history — interrupted',
    taskBackupRunning: 'Backing up',
    taskBackupStale: 'Backup — interrupted',

    // Progress labels
    progressEmbeddedLabel: '{count} pages embedded',
    progressEmbeddedOfTotalLabel: '{processed} / {total} pages embedded',
    progressRecordsLabel: '{processed} of {total} records',
    progressDownloadLabel: '{downloaded} / {total}',

    // Interruption badges
    badgeSafeToClose: 'Safe to close · resumes where it left off',
    badgePerFileResume:
      'Restarts the current file if you quit (finished files are kept)',
    badgeRestartWhole: 'Your data is safe, but this restarts if you quit',
    badgeCannotResume: "Can't resume — was interrupted",

    // Action buttons
    actionRetry: 'Retry',
    actionOpenImport: 'Open Import',
    actionRetryBackup: 'Retry backup',
    actionPause: 'Pause',
    actionCancel: 'Cancel',
    actionResume: 'Resume',

    // Feature chips
    chipSmartSearchLabel: 'Smart-search index',
    chipSiteContentLabel: 'Site content',
    chipAnalysisLabel: 'Analysis',
    chipStateReady: 'Ready',
    chipStateBuilding: 'Building',
    chipStateIdle: 'Idle',
    chipStateOff: 'Off',
    chipStateFailed: 'Failed',
    chipStateDegraded: 'Degraded',
    chipGoToSettings: '→ Settings',
    chipSmartSearchIndexed: '{count} pages indexed',
    chipSmartSearchEmpty: 'Index not built',
    chipSmartSearchBuilding: 'Building...',
    chipSmartSearchOff: 'Not enabled',
    chipSmartSearchFailed: 'Build failed',
    chipSiteContentStored: '{count} pages stored',
    chipSiteContentOff: 'Off',
    chipSiteContentQueued: '{count} fetches queued',
    chipAnalysisReady: 'All modules ready',
    chipAnalysisAttention: '{count} module(s) need refresh',

    // Section headings
    needsAttentionTitle: 'Needs attention',
    runningNowTitle: 'Running now',
    backgroundFeaturesTitle: 'Background features',
    recentTitle: 'Recent activity',
    showRecentToggle: 'Show recent activity ({count})',
    hideRecentToggle: 'Hide recent activity',

    // Outcome pills
    outcomeSuccess: 'Completed',
    outcomeFailed: 'Failed',
    outcomeCancelled: 'Cancelled',
    outcomeInterrupted: 'Interrupted',

    // Callouts
    pausedQueueCallout: 'Queue paused — {count} item(s) waiting',
    pausedQueueBody: 'Work is saved. Resume to continue processing.',

    // Loading state
    loadingActivity: 'Loading activity',
  },

  'zh-CN': {
    statusEyebrow: '后台工作',
    setupTitle: '完成设置后才会开始后台任务',
    setupDescription:
      '先完成初始设置并运行第一次备份。这里之后会显示队列活动、日志和恢复控制。',
    lockedTitle: '先解锁存档，才能查看后台工作',
    lockedEyebrow: '存档已锁定',
    lockedDetail:
      '在存档解锁前，PathKeep 无法读取任务历史，也不能恢复排队任务。',
    refresh: '刷新',
    pauseQueue: '暂停后台工作',
    resumeQueue: '恢复后台工作',
    pageUnavailableTitle: '后台工作暂时不可用',

    now: '刚刚',
    jobStateQueued: '排队中',
    jobStateRunning: '运行中',
    jobStateSucceeded: '已完成',
    jobStateFailed: '失败',
    jobStateCancelled: '已取消',
    jobStatePaused: '已暂停',
    jobStateStale: '需要重新执行',

    retryJob: '重试',
    cancelJob: '取消',

    errorPdf: '这个页面返回的是 PDF，当前网页正文抓取会跳过非 HTML 内容。',
    errorUnsupportedContent:
      '这个页面不是可读取的 HTML 正文，所以 PathKeep 跳过了正文抓取。',
    errorRedirectBlocked:
      '这个页面跳进了登录或跳转边界，PathKeep 目前拿不到稳定的正文。',
    errorRateLimited: '上游网站暂时拒绝了抓取请求，稍后可以再试一次。',

    deterministicRuntimeSummary:
      'PathKeep 会先刷新本地分析；站点内容抓取会在后台单独运行。',
    contentFetchQueuedSummary: '已排队抓取站点内容。',
    contentFetchQueuedSummaryHost: '已排队抓取 {host} 的站点内容。',
    contentFetchRunningSummary: '正在抓取站点内容。',
    contentFetchRunningSummaryHost: '正在抓取 {host} 的站点内容。',

    contentFetchTitle: '站点内容',
    contentFetchDeferredBadge: 'v0.3 开放',
    contentFetchDeferredBody:
      '网页正文抓取已排入 v0.3，v0.2.0 暂不开放。这里先保留路线图入口，但 PathKeep 现在不会重新访问网页，也不会保存正文副本。',
    contentFetchBacklogBody:
      '已保存 {stored} 条可读网页内容；还有 {queued} 条网页正文抓取在队列中，本地分析不用等每个网站都返回。',
    contentFetchRunningBody:
      '已保存 {stored} 条可读网页内容。PathKeep 正在继续补抓网页正文。',
    contentFetchReadyBody:
      '已保存 {stored} 条可读网页内容。当前没有待处理的网页正文抓取。',
    contentFetchOffBody:
      '站点内容抓取可用，但目前关闭——PathKeep 不会联系任何站点。在设置中打开它，为你关心的页面补充内容。',
    contentFetchOpenSettings: '在设置中打开',

    sidebarTitle: '后台工作',
    sidebarNeedsSetup: '完成设置后才会显示后台工作。',
    sidebarLocked: '先解锁存档',
    sidebarLockedDetail: '先打开安全页面，再查看排队工作。',
    sidebarUnavailable: '后台工作暂时不可用',
    sidebarPaused: '{queued} 个排队 · 已暂停',
    sidebarRunning: '{running} 个运行中 · {queued} 个排队',
    sidebarFailed: '{failed} 个需要处理',
    sidebarQueued: '{queued} 个排队中',
    sidebarIdle: '已全部完成',
    sidebarIdleDetail: '当前没有排队任务。',
    sidebarLastActivity: '最近活动 {relative}',
    sidebarOpenSecurity: '安全',
    sidebarOpenJobs: '打开后台任务',
    openJobs: '活动',

    archiveTaskAlreadyRunningTitle: '已有归档任务在运行',
    archiveTaskAlreadyRunningBody:
      '{task} 仍在写入归档记录。打开后台任务可以继续查看实时进度。',
    importTakeoutTaskTitle: '导入 Google Takeout',
    importBrowserTaskTitle: '导入浏览历史',
    importTaskStartedTitle: '导入已开始',
    importTaskStartedBody: '正在从 {source} 写入归档记录。',
    importTaskCompleteTitle: '导入完成',
    importTaskCompleteBody:
      '已写入 {imported} 条新记录 · 跳过 {duplicates} 条重复。',
    backupTaskTitle: '手动备份',
    backupTaskStartedTitle: '备份已开始',
    backupTaskStartedBody: 'PathKeep 正在读取选定浏览器配置，并写入归档记录。',
    backupTaskCompleteTitle: '备份完成',
    archiveTaskFailedTitle: '归档任务失败',
    archiveTaskStaleTitle: '中断的归档任务',
    archiveTaskStaleBody:
      '上次加载记录时，这个运行仍标记为运行中。重启后无法恢复实时进度。',
    archiveTaskStarted: '开始',
    archiveTaskUpdated: '更新',
    archiveTaskRecords: '条记录',
    archiveTaskConsole: '控制台日志',
    archiveTaskNoLogs:
      '正在等待下一条进度事件。大型浏览器配置在复制和校验阶段可能会停留一会儿，之后才出现记录数。',
    archiveTaskOpenResult: '打开结果',
    archiveTaskOpenJobs: '打开后台任务',
    archiveTasksTitle: '归档写入',
    archiveTasksBody:
      '导入和备份在写入归档记录时会留在这里；离开开始页面后仍能继续查看。',

    runningCount: '运行中',

    noErrorDetails: '还没有详细信息。',

    queueSummaryBody:
      '可选的助手与嵌入任务已可用，但默认关闭——在设置中开启即可。排队项的实时计数显示在下方，并可在这里查看。',

    pausedBody: '排队任务已经保存，恢复后会继续处理。',
    pausedTitle: '后台工作已暂停',
    failedBody: '失败的任务会保留在这里，并附上错误信息，方便你重试或取消。',
    failedTitle: '有后台任务需要处理',
    runningBody:
      'PathKeep 正在后台处理排队任务。你可以继续使用应用的其他部分。',
    runningTitle: '后台工作正在运行',
    queuedBody: '这些任务正在等待可用 worker，或等待你恢复队列。',
    queuedTitle: '后台工作正在排队',
    readyBody:
      '当前没有等待的任务。新的本地分析刷新和存档写入任务会显示在这里。',
    readyTitle: '后台工作已空闲',

    // Activity center
    activityPageTitle: '活动',

    headerSummaryFailed: '{failed} 个需处理 · {running} 个运行中',
    headerSummaryFailedIdle: '{failed} 个需处理 · 空闲',
    headerSummaryRunning: '{running} 个运行中 · 可安全关闭',
    headerSummaryRunningNotSafe: '{running} 个运行中',
    headerSummaryRunningWaiting: '{running} 个运行中 · {queued} 个等待中',
    headerSummaryPausedQueued: '队列已暂停 · {queued} 个等待中',
    headerSummaryQueued: '{queued} 个等待中',
    headerSummaryAllCaughtUp: '一切就绪 · 最近活动 {time}',
    headerSummaryNoActivity: '一切就绪 · 暂无最近活动',

    taskIndexBuild: '正在建立智能搜索索引',
    taskModelDownload: '正在下载嵌入模型',
    taskContentFetch: '正在抓取网站内容',
    taskReEmbed: '正在重新嵌入历史记录',
    taskDeterministicRebuild: '正在刷新分析',
    taskImportRunning: '正在导入历史记录',
    taskImportStale: '导入历史记录 — 已中断',
    taskBackupRunning: '正在备份',
    taskBackupStale: '备份 — 已中断',

    progressEmbeddedLabel: '已嵌入 {count} 页',
    progressEmbeddedOfTotalLabel: '已嵌入 {processed} / {total} 页',
    progressRecordsLabel: '{processed} / {total} 条记录',
    progressDownloadLabel: '{downloaded} / {total}',

    badgeSafeToClose: '可以关闭 · 下次会从中断处继续',
    badgePerFileResume: '退出会重新下载当前文件（已完成的文件不受影响）',
    badgeRestartWhole: '数据安全，但退出后需重新开始',
    badgeCannotResume: '无法继续 — 已中断',

    actionRetry: '重试',
    actionOpenImport: '前往导入',
    actionRetryBackup: '重试备份',
    actionPause: '暂停',
    actionCancel: '取消',
    actionResume: '恢复',

    chipSmartSearchLabel: '智能搜索索引',
    chipSiteContentLabel: '网站内容',
    chipAnalysisLabel: '分析',
    chipStateReady: '就绪',
    chipStateBuilding: '建立中',
    chipStateIdle: '空闲',
    chipStateOff: '已关闭',
    chipStateFailed: '失败',
    chipStateDegraded: '已降级',
    chipGoToSettings: '→ 前往设置',
    chipSmartSearchIndexed: '已索引 {count} 页',
    chipSmartSearchEmpty: '索引尚未建立',
    chipSmartSearchBuilding: '正在建立…',
    chipSmartSearchOff: '未启用',
    chipSmartSearchFailed: '建立失败',
    chipSiteContentStored: '已保存 {count} 页',
    chipSiteContentOff: '已关闭',
    chipSiteContentQueued: '{count} 项抓取排队中',
    chipAnalysisReady: '所有模块已就绪',
    chipAnalysisAttention: '{count} 个模块需要重新整理',

    needsAttentionTitle: '需要处理',
    runningNowTitle: '正在运行',
    backgroundFeaturesTitle: '后台功能',
    recentTitle: '最近活动',
    showRecentToggle: '显示最近活动（{count} 项）',
    hideRecentToggle: '隐藏最近活动',

    outcomeSuccess: '已完成',
    outcomeFailed: '失败',
    outcomeCancelled: '已取消',
    outcomeInterrupted: '已中断',

    pausedQueueCallout: '队列已暂停 — {count} 项等待中',
    pausedQueueBody: '任务已保存，恢复后继续处理。',

    loadingActivity: '正在加载活动',
  },

  'zh-TW': {
    statusEyebrow: '背景工作',
    setupTitle: '完成設定後才會開始背景任務',
    setupDescription:
      '先完成初始設定並執行第一次備份。之後這裡會顯示佇列活動、日誌與恢復控制。',
    lockedTitle: '先解鎖封存，才能查看背景工作',
    lockedEyebrow: '封存已鎖定',
    lockedDetail:
      '在封存解鎖前，PathKeep 無法讀取任務歷史，也不能恢復排隊任務。',
    refresh: '重新整理',
    pauseQueue: '暫停背景工作',
    resumeQueue: '恢復背景工作',
    pageUnavailableTitle: '背景工作暫時無法使用',

    now: '剛剛',
    jobStateQueued: '排隊中',
    jobStateRunning: '執行中',
    jobStateSucceeded: '已完成',
    jobStateFailed: '失敗',
    jobStateCancelled: '已取消',
    jobStatePaused: '已暫停',
    jobStateStale: '需要重新執行',

    retryJob: '重試',
    cancelJob: '取消',

    errorPdf: '這個頁面回傳的是 PDF，目前網頁正文抓取會跳過非 HTML 內容。',
    errorUnsupportedContent:
      '這個頁面不是可讀的 HTML 正文，所以 PathKeep 跳過了正文抓取。',
    errorRedirectBlocked:
      '這個頁面跳進了登入或重新導向邊界，PathKeep 目前拿不到穩定的正文。',
    errorRateLimited: '上游網站暫時拒絕了抓取請求，稍後可以再試一次。',

    deterministicRuntimeSummary:
      'PathKeep 會先重新整理本機分析；網站內容擷取會在背景單獨執行。',
    contentFetchQueuedSummary: '已排隊擷取網站內容。',
    contentFetchQueuedSummaryHost: '已排隊擷取 {host} 的網站內容。',
    contentFetchRunningSummary: '正在擷取網站內容。',
    contentFetchRunningSummaryHost: '正在擷取 {host} 的網站內容。',

    contentFetchTitle: '網站內容',
    contentFetchDeferredBadge: 'v0.3 開放',
    contentFetchDeferredBody:
      '網頁正文擷取已排入 v0.3，v0.2.0 暫不開放。這裡先保留路線圖入口，但 PathKeep 現在不會重新造訪網頁，也不會保存正文副本。',
    contentFetchBacklogBody:
      '已保存 {stored} 筆可讀網頁內容；還有 {queued} 筆網頁正文擷取在佇列中，本機分析不用等每個網站都回應。',
    contentFetchRunningBody:
      '已保存 {stored} 筆可讀網頁內容。PathKeep 正在繼續補抓網頁正文。',
    contentFetchReadyBody:
      '已保存 {stored} 筆可讀網頁內容。現在沒有待處理的網頁正文抓取。',
    contentFetchOffBody:
      '網站內容擷取可用，但目前關閉——PathKeep 不會聯絡任何網站。在設定中打開它，為你在意的頁面補充內容。',
    contentFetchOpenSettings: '在設定中打開',

    sidebarTitle: '背景工作',
    sidebarNeedsSetup: '完成設定後才會顯示背景工作。',
    sidebarLocked: '先解鎖封存',
    sidebarLockedDetail: '先打開安全頁面，再查看排隊工作。',
    sidebarUnavailable: '背景工作暫時無法使用',
    sidebarPaused: '{queued} 個排隊 · 已暫停',
    sidebarRunning: '{running} 個執行中 · {queued} 個排隊',
    sidebarFailed: '{failed} 個需要處理',
    sidebarQueued: '{queued} 個排隊中',
    sidebarIdle: '已全部完成',
    sidebarIdleDetail: '目前沒有排隊任務。',
    sidebarLastActivity: '最近活動 {relative}',
    sidebarOpenSecurity: '安全',
    sidebarOpenJobs: '打開背景工作',
    openJobs: '活動',

    archiveTaskAlreadyRunningTitle: '已有封存任務正在執行',
    archiveTaskAlreadyRunningBody:
      '{task} 仍在寫入封存紀錄。打開背景工作可以繼續查看即時進度。',
    importTakeoutTaskTitle: '匯入 Google Takeout',
    importBrowserTaskTitle: '匯入瀏覽歷史',
    importTaskStartedTitle: '匯入已開始',
    importTaskStartedBody: '正在從 {source} 寫入封存紀錄。',
    importTaskCompleteTitle: '匯入完成',
    importTaskCompleteBody:
      '已寫入 {imported} 筆新紀錄 · 略過 {duplicates} 筆重複。',
    backupTaskTitle: '手動備份',
    backupTaskStartedTitle: '備份已開始',
    backupTaskStartedBody:
      'PathKeep 正在讀取選定瀏覽器設定檔，並寫入封存紀錄。',
    backupTaskCompleteTitle: '備份完成',
    archiveTaskFailedTitle: '封存任務失敗',
    archiveTaskStaleTitle: '中斷的封存任務',
    archiveTaskStaleBody:
      '上次載入紀錄時，這個執行仍標記為執行中。重啟後無法恢復即時進度。',
    archiveTaskStarted: '開始',
    archiveTaskUpdated: '更新',
    archiveTaskRecords: '筆紀錄',
    archiveTaskConsole: 'Console log',
    archiveTaskNoLogs:
      '正在等待下一筆進度事件。大型瀏覽器設定檔在複製和驗證階段可能會停留一會兒，之後才出現記錄數。',
    archiveTaskOpenResult: '打開結果',
    archiveTaskOpenJobs: '打開背景工作',
    archiveTasksTitle: '封存寫入',
    archiveTasksBody:
      '匯入和備份在寫入封存紀錄時會留在這裡；離開開始頁面後仍能繼續查看。',

    runningCount: '執行中',

    noErrorDetails: '目前還沒有詳細資訊。',

    queueSummaryBody:
      '可選的助手與嵌入工作已可用，但預設關閉——在設定中開啟即可。排隊項的即時計數顯示在下方，並可在這裡查看。',

    pausedBody: '排隊任務已經保存，恢復後就會繼續處理。',
    pausedTitle: '背景工作已暫停',
    failedBody: '失敗的任務會留在這裡，並附上錯誤資訊，方便你重試或取消。',
    failedTitle: '有背景任務需要處理',
    runningBody:
      'PathKeep 正在背景處理排隊任務。你可以繼續使用應用的其他部分。',
    runningTitle: '背景工作正在執行',
    queuedBody: '這些任務正在等待可用 worker，或等待你恢復佇列。',
    queuedTitle: '背景工作正在排隊',
    readyBody:
      '目前沒有等待的任務。新的本機分析重新整理和封存寫入工作會顯示在這裡。',
    readyTitle: '背景工作目前空閒',

    // Activity center
    activityPageTitle: '活動',

    headerSummaryFailed: '{failed} 個需處理 · {running} 個執行中',
    headerSummaryFailedIdle: '{failed} 個需處理 · 空閒',
    headerSummaryRunning: '{running} 個執行中 · 可安全關閉',
    headerSummaryRunningNotSafe: '{running} 個執行中',
    headerSummaryRunningWaiting: '{running} 個執行中 · {queued} 個等待中',
    headerSummaryPausedQueued: '佇列已暫停 · {queued} 個等待中',
    headerSummaryQueued: '{queued} 個等待中',
    headerSummaryAllCaughtUp: '一切就緒 · 最近活動 {time}',
    headerSummaryNoActivity: '一切就緒 · 暫無最近活動',

    taskIndexBuild: '正在建立智慧搜尋索引',
    taskModelDownload: '正在下載嵌入模型',
    taskContentFetch: '正在擷取網站內容',
    taskReEmbed: '正在重新嵌入歷史紀錄',
    taskDeterministicRebuild: '正在重新整理分析',
    taskImportRunning: '正在匯入歷史紀錄',
    taskImportStale: '匯入歷史紀錄 — 已中斷',
    taskBackupRunning: '正在備份',
    taskBackupStale: '備份 — 已中斷',

    progressEmbeddedLabel: '已嵌入 {count} 頁',
    progressEmbeddedOfTotalLabel: '已嵌入 {processed} / {total} 頁',
    progressRecordsLabel: '{processed} / {total} 筆紀錄',
    progressDownloadLabel: '{downloaded} / {total}',

    badgeSafeToClose: '可以關閉 · 下次會從中斷處繼續',
    badgePerFileResume: '退出會重新下載目前檔案（已完成的檔案不受影響）',
    badgeRestartWhole: '資料安全，但退出後需重新開始',
    badgeCannotResume: '無法繼續 — 已中斷',

    actionRetry: '重試',
    actionOpenImport: '前往匯入',
    actionRetryBackup: '重試備份',
    actionPause: '暫停',
    actionCancel: '取消',
    actionResume: '恢復',

    chipSmartSearchLabel: '智慧搜尋索引',
    chipSiteContentLabel: '網站內容',
    chipAnalysisLabel: '分析',
    chipStateReady: '就緒',
    chipStateBuilding: '建立中',
    chipStateIdle: '空閒',
    chipStateOff: '已關閉',
    chipStateFailed: '失敗',
    chipStateDegraded: '已降級',
    chipGoToSettings: '→ 前往設定',
    chipSmartSearchIndexed: '已索引 {count} 頁',
    chipSmartSearchEmpty: '索引尚未建立',
    chipSmartSearchBuilding: '正在建立…',
    chipSmartSearchOff: '未啟用',
    chipSmartSearchFailed: '建立失敗',
    chipSiteContentStored: '已保存 {count} 頁',
    chipSiteContentOff: '已關閉',
    chipSiteContentQueued: '{count} 項擷取排隊中',
    chipAnalysisReady: '所有模組已就緒',
    chipAnalysisAttention: '{count} 個模組需要重新整理',

    needsAttentionTitle: '需要處理',
    runningNowTitle: '正在執行',
    backgroundFeaturesTitle: '背景功能',
    recentTitle: '最近活動',
    showRecentToggle: '顯示最近活動（{count} 項）',
    hideRecentToggle: '隱藏最近活動',

    outcomeSuccess: '已完成',
    outcomeFailed: '失敗',
    outcomeCancelled: '已取消',
    outcomeInterrupted: '已中斷',

    pausedQueueCallout: '佇列已暫停 — {count} 項等待中',
    pausedQueueBody: '任務已保存，恢復後繼續處理。',

    loadingActivity: '正在載入活動',
  },
} as const
