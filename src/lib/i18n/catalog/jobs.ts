/**
 * @file jobs.ts
 * @description Owns jobs and background work queue copy across shipped locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `jobs` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
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
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const jobsNamespaceCatalog = {
  en: {
    statusEyebrow: 'BACKGROUND WORK',
    readyTitle: 'Background work is idle',
    readyBody:
      'No work is waiting. Local analysis refreshes and archive write tasks will appear here.',
    pausedTitle: 'Background work is paused',
    pausedBody: 'Queued work is saved and will stay here until you resume it.',
    runningTitle: 'Background work is running',
    runningBody:
      'PathKeep is processing queued work in the background. You can keep using the app.',
    queuedTitle: 'Background work is queued',
    queuedBody:
      'Queued work is waiting for an available worker slot or for you to resume the queue.',
    failedTitle: 'Some background work needs review',
    failedBody:
      'Failed jobs stay here with their error details so you can retry or cancel them.',
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
    openSettings: 'Open Settings',
    loadingPage: 'Loading background work',
    pageUnavailableTitle: 'Background work is unavailable',
    overviewTitle: 'Queue overview',
    overviewHeadline: 'Refresh local analysis first',
    overviewHeadlineIdle: 'All caught up',
    overviewHeadlineRunning: 'Running now: {count}',
    overviewHeadlineQueued: 'Waiting for a worker: {count}',
    overviewHeadlinePaused: 'Queue paused · saved: {count}',
    overviewHeadlineFailures: 'Needs review: {count}',
    jumpToFailures: 'Review failed items: {count}',
    overviewBody:
      'Local rebuilds update cards and evidence without waiting on future AI features or optional site-content fetches.',
    queueSummaryTitle: 'Assistant and embedding queue',
    queueSummaryBody:
      'Optional assistant and embedding work is available but off by default — enable it in Settings. Queued items show their live counts below and stay reviewable here.',
    runtimeSummaryTitle: 'Local analysis',
    runtimeSummaryBody:
      'Analysis refreshes and content work keep a recoverable record of what ran, failed, or still waits.',
    recoveryTitle: 'Recovery',
    recoveryBody:
      'Queued work is saved in the archive. If the app closes unexpectedly, unfinished items stay here so you can review or retry them after reopening PathKeep.',
    noRecoveryNotes: 'No recovery notes right now.',
    pluginsTitle: 'Local workers',
    modulesTitle: 'Analysis modules',
    recentAiJobs: 'Recent assistant jobs',
    recentRuntimeJobs: 'Recent derived-data jobs',
    recentJobsEmpty: 'No background jobs recorded yet.',
    queuedCount: 'Queued',
    runningCount: 'Running',
    failedCount: 'Failed',
    concurrency: 'Workers',
    queueStatePaused: 'Paused',
    queueStateLive: 'Queue active',
    lastActivity: 'Last activity',
    lastCompletedAt: 'Last completed',
    derivedTables: 'Derived tables',
    retryJob: 'Retry',
    cancelJob: 'Cancel',
    createdAt: 'Created',
    startedAt: 'Started',
    finishedAt: 'Finished',
    noErrorDetails: 'No details yet.',
    savedReadableContent: 'Stored content rows',
    storedRecordsLabel: 'Stored rows',
    focusNow: 'Current',
    focusNowBacklog:
      'Background work is active. Refresh to load the latest item.',
    focusNowIdle: 'No task is running right now.',
    needsReviewNow: 'Review',
    needsReviewIdle: 'No failed work needs review.',
    needsReviewBacklog: '{count} failed job(s) still need review or retry.',
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
    titleNormalizationBody:
      'Title cleanup runs on this device so search, summaries, and cards use steadier labels.',
    moduleAttentionBody: '{count} analysis module(s) need refresh or review.',
    moduleHealthyBody: 'Enabled analysis modules are up to date.',
    moduleReadyCount: 'Ready',
    moduleAttentionCount: 'Need refresh',
    runtimeHealthTitle: 'Queue details',
    runtimeHealthBody:
      'Content workers, analysis modules, and recovery notes stay here for deeper review.',
    recentActivityTitle: 'Recent activity',
    recentActivityBody:
      'Review recent assistant and analysis tasks by family. Retry failures, cancel queued work, or confirm the queue is still moving.',
    errorPdf:
      'This page returned a PDF, so PathKeep skipped readable-text extraction.',
    errorUnsupportedContent:
      'This page did not return readable HTML, so PathKeep skipped page-text extraction.',
    errorRedirectBlocked:
      'This page redirected into a sign-in or redirect boundary, so PathKeep could not keep a stable readable copy.',
    errorRateLimited:
      'The upstream site temporarily refused the fetch. Retry it later.',
    deterministicRuntimeSummary:
      'PathKeep is refreshing local analysis first; any site-content fetches run separately in the background.',
    contentFetchQueuedSummaryHost: 'Queued to fetch site content for {host}.',
    contentFetchQueuedSummary: 'Queued to fetch site content.',
    contentFetchRunningSummaryHost: 'Fetching site content for {host}.',
    contentFetchRunningSummary: 'Fetching site content.',
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
    openJobs: 'Jobs',
    archiveTasksTitle: 'Archive writes',
    archiveTasksBody:
      'Imports and backups stay visible here while they write archive records, even if you leave the starting page.',
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
    now: 'just now',
    jobStateQueued: 'Queued',
    jobStateRunning: 'Running',
    jobStateSucceeded: 'Completed',
    jobStateFailed: 'Failed',
    jobStateCancelled: 'Cancelled',
    jobStatePaused: 'Paused',
    jobStateStale: 'Needs replay',
  },
  'zh-CN': {
    statusEyebrow: '后台工作',
    readyTitle: '后台工作已空闲',
    readyBody:
      '当前没有等待的任务。新的本地分析刷新和存档写入任务会显示在这里。',
    pausedTitle: '后台工作已暂停',
    pausedBody: '排队任务已经保存，恢复后会继续处理。',
    runningTitle: '后台工作正在运行',
    runningBody:
      'PathKeep 正在后台处理排队任务。你可以继续使用应用的其他部分。',
    queuedTitle: '后台工作正在排队',
    queuedBody: '这些任务正在等待可用 worker，或等待你恢复队列。',
    failedTitle: '有后台任务需要处理',
    failedBody: '失败的任务会保留在这里，并附上错误信息，方便你重试或取消。',
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
    openSettings: '打开设置',
    loadingPage: '正在加载后台工作',
    pageUnavailableTitle: '后台工作暂时不可用',
    overviewTitle: '队列总览',
    overviewHeadline: '先刷新本地分析',
    overviewHeadlineIdle: '一切就绪',
    overviewHeadlineRunning: '{count} 项正在运行',
    overviewHeadlineQueued: '{count} 项等待中',
    overviewHeadlinePaused: '队列已暂停 · 已保存 {count} 项',
    overviewHeadlineFailures: '{count} 项需要复核',
    jumpToFailures: '跳到 {count} 项失败',
    overviewBody:
      '本地重建会更新卡片和证据，不会等待后续版本才开放的 AI 功能或可选的站点内容抓取。',
    queueSummaryTitle: '助手与嵌入队列',
    queueSummaryBody:
      '可选的助手与嵌入任务已可用，但默认关闭——在设置中开启即可。排队项的实时计数显示在下方，并可在这里查看。',
    runtimeSummaryTitle: '本地分析',
    runtimeSummaryBody:
      '分析刷新和内容任务会保留可恢复记录，方便查看哪些已完成、失败或仍在等待。',
    recoveryTitle: '恢复',
    recoveryBody:
      '排队任务会保存在存档里。如果应用意外关闭，未完成的任务仍会留在这里，重新打开 PathKeep 后可以继续查看或重试。',
    noRecoveryNotes: '当前没有恢复备注。',
    pluginsTitle: '本地任务',
    modulesTitle: '分析模块',
    recentAiJobs: '最近的助手任务',
    recentRuntimeJobs: '最近的衍生数据任务',
    recentJobsEmpty: '还没有后台任务记录。',
    queuedCount: '排队中',
    runningCount: '运行中',
    failedCount: '失败',
    concurrency: 'Worker 数',
    queueStatePaused: '已暂停',
    queueStateLive: '队列可用',
    lastActivity: '最近活动',
    lastCompletedAt: '上次完成',
    derivedTables: '衍生表',
    retryJob: '重试',
    cancelJob: '取消',
    createdAt: '创建时间',
    startedAt: '开始时间',
    finishedAt: '结束时间',
    noErrorDetails: '还没有详细信息。',
    savedReadableContent: '已保存内容行',
    storedRecordsLabel: '已保存行数',
    focusNow: '当前',
    focusNowBacklog: '后台有任务在运行。刷新后可查看最新项目。',
    focusNowIdle: '当前没有任务运行。',
    needsReviewNow: '需处理',
    needsReviewIdle: '当前没有失败任务需要处理。',
    needsReviewBacklog: '还有 {count} 个失败任务等待你查看或重试。',
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
    titleNormalizationBody:
      '标题规范化会在本机运行，帮助搜索、摘要和卡片使用更稳定的标签。',
    moduleAttentionBody: '{count} 个分析模块需要刷新或检查。',
    moduleHealthyBody: '已启用的分析模块都处于最新状态。',
    moduleReadyCount: '已就绪',
    moduleAttentionCount: '需刷新',
    runtimeHealthTitle: '队列详情',
    runtimeHealthBody:
      '内容任务、分析模块和恢复记录放在这里，方便需要时深入检查。',
    recentActivityTitle: '最近活动',
    recentActivityBody:
      '按任务类型查看最近的助手和分析任务；失败可重试，排队项可取消，也能确认队列是否仍在推进。',
    errorPdf: '这个页面返回的是 PDF，当前网页正文抓取会跳过非 HTML 内容。',
    errorUnsupportedContent:
      '这个页面不是可读取的 HTML 正文，所以 PathKeep 跳过了正文抓取。',
    errorRedirectBlocked:
      '这个页面跳进了登录或跳转边界，PathKeep 目前拿不到稳定的正文。',
    errorRateLimited: '上游网站暂时拒绝了抓取请求，稍后可以再试一次。',
    deterministicRuntimeSummary:
      'PathKeep 会先刷新本地分析；站点内容抓取会在后台单独运行。',
    contentFetchQueuedSummaryHost: '已排队抓取 {host} 的站点内容。',
    contentFetchQueuedSummary: '已排队抓取站点内容。',
    contentFetchRunningSummaryHost: '正在抓取 {host} 的站点内容。',
    contentFetchRunningSummary: '正在抓取站点内容。',
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
    openJobs: '后台任务',
    archiveTasksTitle: '归档写入',
    archiveTasksBody:
      '导入和备份在写入归档记录时会留在这里；离开开始页面后仍能继续查看。',
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
    now: '刚刚',
    jobStateQueued: '排队中',
    jobStateRunning: '运行中',
    jobStateSucceeded: '已完成',
    jobStateFailed: '失败',
    jobStateCancelled: '已取消',
    jobStatePaused: '已暂停',
    jobStateStale: '需要重新执行',
  },
  'zh-TW': {
    statusEyebrow: '背景工作',
    readyTitle: '背景工作目前空閒',
    readyBody:
      '目前沒有等待的任務。新的本機分析重新整理和封存寫入工作會顯示在這裡。',
    pausedTitle: '背景工作已暫停',
    pausedBody: '排隊任務已經保存，恢復後就會繼續處理。',
    runningTitle: '背景工作正在執行',
    runningBody:
      'PathKeep 正在背景處理排隊任務。你可以繼續使用應用的其他部分。',
    queuedTitle: '背景工作正在排隊',
    queuedBody: '這些任務正在等待可用 worker，或等待你恢復佇列。',
    failedTitle: '有背景任務需要處理',
    failedBody: '失敗的任務會留在這裡，並附上錯誤資訊，方便你重試或取消。',
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
    openSettings: '打開設定',
    loadingPage: '正在載入背景工作',
    pageUnavailableTitle: '背景工作暫時無法使用',
    overviewTitle: '佇列總覽',
    overviewHeadline: '先重新整理本機分析',
    overviewHeadlineIdle: '一切就緒',
    overviewHeadlineRunning: '{count} 項執行中',
    overviewHeadlineQueued: '{count} 項等待中',
    overviewHeadlinePaused: '佇列已暫停 · 已保存 {count} 項',
    overviewHeadlineFailures: '{count} 項需要複核',
    jumpToFailures: '跳到 {count} 項失敗',
    overviewBody:
      '本機重建會更新卡片和證據，不會等待後續版本才開放的 AI 功能或可選的網站內容擷取。',
    queueSummaryTitle: '助手與嵌入佇列',
    queueSummaryBody:
      '可選的助手與嵌入工作已可用，但預設關閉——在設定中開啟即可。排隊項的即時計數顯示在下方，並可在這裡查看。',
    runtimeSummaryTitle: '本機分析',
    runtimeSummaryBody:
      '分析重新整理和內容任務會保留可恢復紀錄，方便查看哪些已完成、失敗或仍在等待。',
    recoveryTitle: '恢復',
    recoveryBody:
      '排隊任務會保存在封存裡。如果應用意外關閉，未完成的任務仍會留在這裡，重新開啟 PathKeep 後可以繼續查看或重試。',
    noRecoveryNotes: '目前沒有恢復備註。',
    pluginsTitle: '本機任務',
    modulesTitle: '分析模組',
    recentAiJobs: '最近的助手任務',
    recentRuntimeJobs: '最近的衍生資料任務',
    recentJobsEmpty: '還沒有背景任務紀錄。',
    queuedCount: '排隊中',
    runningCount: '執行中',
    failedCount: '失敗',
    concurrency: 'Worker 數',
    queueStatePaused: '已暫停',
    queueStateLive: '佇列可用',
    lastActivity: '最近活動',
    lastCompletedAt: '上次完成',
    derivedTables: '衍生資料表',
    retryJob: '重試',
    cancelJob: '取消',
    createdAt: '建立時間',
    startedAt: '開始時間',
    finishedAt: '結束時間',
    noErrorDetails: '目前還沒有詳細資訊。',
    savedReadableContent: '已保存內容列',
    storedRecordsLabel: '已保存列數',
    focusNow: '目前',
    focusNowBacklog: '背景有任務正在執行。重新整理後可查看最新項目。',
    focusNowIdle: '目前沒有任務正在執行。',
    needsReviewNow: '待處理',
    needsReviewIdle: '目前沒有失敗任務需要處理。',
    needsReviewBacklog: '還有 {count} 個失敗任務等你查看或重試。',
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
    titleNormalizationBody:
      '標題正規化會在本機執行，幫助搜尋、摘要與卡片使用更穩定的標籤。',
    moduleAttentionBody: '{count} 個分析模組需要重新整理或檢查。',
    moduleHealthyBody: '已啟用的分析模組目前都已是最新狀態。',
    moduleReadyCount: '已就緒',
    moduleAttentionCount: '需重新整理',
    runtimeHealthTitle: '佇列詳情',
    runtimeHealthBody:
      '內容任務、分析模組和恢復紀錄放在這裡，方便需要時深入檢查。',
    recentActivityTitle: '最近活動',
    recentActivityBody:
      '按任務類型查看最近的助手和分析任務；失敗可重試，排隊項可取消，也能確認佇列是否仍在推進。',
    errorPdf: '這個頁面回傳的是 PDF，目前網頁正文抓取會跳過非 HTML 內容。',
    errorUnsupportedContent:
      '這個頁面不是可讀的 HTML 正文，所以 PathKeep 跳過了正文抓取。',
    errorRedirectBlocked:
      '這個頁面跳進了登入或重新導向邊界，PathKeep 目前拿不到穩定的正文。',
    errorRateLimited: '上游網站暫時拒絕了抓取請求，稍後可以再試一次。',
    deterministicRuntimeSummary:
      'PathKeep 會先重新整理本機分析；網站內容擷取會在背景單獨執行。',
    contentFetchQueuedSummaryHost: '已排隊擷取 {host} 的網站內容。',
    contentFetchQueuedSummary: '已排隊擷取網站內容。',
    contentFetchRunningSummaryHost: '正在擷取 {host} 的網站內容。',
    contentFetchRunningSummary: '正在擷取網站內容。',
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
    openJobs: '背景工作',
    archiveTasksTitle: '封存寫入',
    archiveTasksBody:
      '匯入和備份在寫入封存紀錄時會留在這裡；離開開始頁面後仍能繼續查看。',
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
    now: '剛剛',
    jobStateQueued: '排隊中',
    jobStateRunning: '執行中',
    jobStateSucceeded: '已完成',
    jobStateFailed: '失敗',
    jobStateCancelled: '已取消',
    jobStatePaused: '已暫停',
    jobStateStale: '需要重新執行',
  },
} as const
