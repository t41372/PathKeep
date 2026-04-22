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
      'Nothing is waiting right now. New rebuilds, enrichment fetches, and index jobs will appear here.',
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
    overviewHeadline:
      'Finish deterministic evidence first, then refill page text in the background',
    overviewBody:
      "A huge content-fetch backlog doesn't automatically mean PathKeep is stuck. It intentionally finishes the deterministic rebuild first so cards and derived evidence stay reviewable sooner.",
    queueSummaryTitle: 'AI queue',
    queueSummaryBody:
      'Embedding and assistant jobs are stored in the archive queue so they can finish after the foreground action ends.',
    runtimeSummaryTitle: 'Derived-data queue',
    runtimeSummaryBody:
      'Deterministic rebuilds and enrichment jobs keep their own trace so you can review what ran and what still needs attention.',
    recoveryTitle: 'Recovery',
    recoveryBody:
      'Queued work is saved in the archive. If the app closes unexpectedly, unfinished items stay here so you can review or retry them after reopening PathKeep.',
    noRecoveryNotes: 'No recovery notes right now.',
    pluginsTitle: 'Enrichment plugins',
    modulesTitle: 'Deterministic modules',
    recentAiJobs: 'Recent AI jobs',
    recentRuntimeJobs: 'Recent derived-data jobs',
    recentJobsEmpty: 'No background jobs recorded yet.',
    queuedCount: 'Queued',
    runningCount: 'Running',
    failedCount: 'Failed',
    concurrency: 'Workers',
    queueStatePaused: 'Paused',
    queueStateLive: 'Live',
    lastActivity: 'Last activity',
    lastCompletedAt: 'Last completed',
    derivedTables: 'Derived tables',
    retryJob: 'Retry',
    cancelJob: 'Cancel',
    createdAt: 'Created',
    startedAt: 'Started',
    finishedAt: 'Finished',
    noErrorDetails: 'No details yet.',
    savedReadableContent: 'Readable pages saved',
    storedRecordsLabel: 'Stored rows',
    focusNow: 'Moving now',
    focusNowBacklog:
      'Background work is still active. Refresh to load the newest live item.',
    focusNowIdle: 'No active work is running right now.',
    needsReviewNow: 'Needs your attention',
    needsReviewIdle: 'Nothing needs manual review right now.',
    needsReviewBacklog: '{count} failed job(s) still need review or retry.',
    contentFetchTitle: 'Page content fetch',
    contentFetchBacklogBody:
      'PathKeep already saved {stored} readable pages. The remaining {queued} network fetches stay in the queue so rebuilds can finish without waiting on every site.',
    contentFetchRunningBody:
      'PathKeep already saved {stored} readable pages and is still fetching more page text in the background.',
    contentFetchReadyBody:
      'PathKeep already saved {stored} readable pages and the fetch queue is currently clear.',
    contentFetchFallbackBody:
      'Page text fetches add deeper local evidence for summaries and deterministic insights.',
    titleNormalizationBody:
      'Title cleanup runs locally and quickly so search, summaries, and insight cards can use steadier evidence labels first.',
    moduleAttentionBody:
      '{count} deterministic module(s) still need a rebuild or another review pass.',
    moduleHealthyBody:
      'Every enabled deterministic module is currently up to date.',
    moduleReadyCount: 'Modules ready',
    moduleAttentionCount: 'Need rebuild',
    runtimeHealthTitle: 'Runtime health',
    runtimeHealthBody:
      'Plugins, deterministic modules, and recovery notes are split apart here so you can tell the difference between queued, running, failed, and intentionally deferred work.',
    recentActivityTitle: 'Recent activity',
    recentActivityBody:
      'Review recent AI and derived-data jobs by family so you can retry failures, cancel queued work, or confirm the queue is still moving.',
    errorPdf:
      'This page returned a PDF, so PathKeep skipped readable-text extraction.',
    errorUnsupportedContent:
      'This page did not return readable HTML, so PathKeep skipped page-text extraction.',
    errorRedirectBlocked:
      'This page redirected into a sign-in or redirect boundary, so PathKeep could not keep a stable readable copy.',
    errorRateLimited:
      'The upstream site temporarily refused the fetch. Retry it later.',
    deterministicRuntimeSummary:
      'PathKeep is rebuilding deterministic evidence first, then it will continue the slower network-backed work.',
    contentFetchQueuedSummaryHost:
      'This page-text fetch is waiting in the queue and will revisit {host} later.',
    contentFetchQueuedSummary:
      'This page-text fetch is waiting in the queue and will revisit the source page later.',
    contentFetchRunningSummaryHost:
      'PathKeep is revisiting {host} now to capture readable page text.',
    contentFetchRunningSummary:
      'PathKeep is capturing readable page text for this record right now.',
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
    readyBody: '当前没有排队任务。新的重建、内容补全和索引任务会显示在这里。',
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
    overviewHeadline: '先完成确定性分析，再慢慢补抓网页正文',
    overviewBody:
      '大批量的内容抓取 backlog 并不代表系统卡住了。PathKeep 会先完成可立即交付的确定性重建，再继续处理网络型网页内容抓取。',
    queueSummaryTitle: 'AI 队列',
    queueSummaryBody:
      'embedding 和助手任务会保存在存档队列里，所以前台操作结束后也能继续完成。',
    runtimeSummaryTitle: '衍生数据队列',
    runtimeSummaryBody:
      '确定性重建和 enrichment 任务会保留自己的运行记录，方便你查看哪些已经完成，哪些还需要处理。',
    recoveryTitle: '恢复',
    recoveryBody:
      '排队任务会保存在存档里。如果应用意外关闭，未完成的任务仍会留在这里，重新打开 PathKeep 后可以继续查看或重试。',
    noRecoveryNotes: '当前没有恢复备注。',
    pluginsTitle: 'Enrichment 插件',
    modulesTitle: '确定性模块',
    recentAiJobs: '最近的 AI 任务',
    recentRuntimeJobs: '最近的衍生数据任务',
    recentJobsEmpty: '还没有后台任务记录。',
    queuedCount: '排队中',
    runningCount: '运行中',
    failedCount: '失败',
    concurrency: 'Worker 数',
    queueStatePaused: '已暂停',
    queueStateLive: '运行中',
    lastActivity: '最近活动',
    lastCompletedAt: '上次完成',
    derivedTables: '衍生表',
    retryJob: '重试',
    cancelJob: '取消',
    createdAt: '创建时间',
    startedAt: '开始时间',
    finishedAt: '结束时间',
    noErrorDetails: '还没有详细信息。',
    savedReadableContent: '已保存正文',
    storedRecordsLabel: '已保存行数',
    focusNow: '正在推进',
    focusNowBacklog: '后台仍有任务在运行，请刷新以查看最新的活动项。',
    focusNowIdle: '当前没有活跃工作，队列会在下一次重建后继续。',
    needsReviewNow: '需要你处理',
    needsReviewIdle: '目前没有需要人工处理的失败项。',
    needsReviewBacklog: '还有 {count} 个失败任务等待你查看或重试。',
    contentFetchTitle: '网页内容抓取',
    contentFetchBacklogBody:
      '已保存 {stored} 条可读网页内容。剩余 {queued} 条网络抓取会在后台慢慢补齐，所以重建不会被大队列拖慢。',
    contentFetchRunningBody:
      '已保存 {stored} 条可读网页内容。PathKeep 正在继续补抓网页正文。',
    contentFetchReadyBody:
      '已保存 {stored} 条可读网页内容。当前没有待处理的网页正文抓取。',
    contentFetchFallbackBody:
      '网页内容抓取会在后台为摘要和洞察补充更多可引用的本机证据。',
    titleNormalizationBody:
      '标题规范化会先在本机快速完成，帮助搜索、摘要和洞察尽快得到更稳定的证据标签。',
    moduleAttentionBody: '{count} 个确定性模块正在等待重建或需要重新检查。',
    moduleHealthyBody: '所有已启用的确定性模块都处于最新状态。',
    moduleReadyCount: '已就绪模块',
    moduleAttentionCount: '待处理模块',
    runtimeHealthTitle: '运行时边界',
    runtimeHealthBody:
      '这里会把插件、确定性模块和恢复线索分开显示，让你分辨现在是在排队、运行、失败，还是只是被延后执行。',
    recentActivityTitle: '最近活动',
    recentActivityBody:
      '按任务家族查看最近的 AI 与衍生数据工作，方便你重试失败项、取消排队项，或确认后台仍在继续前进。',
    errorPdf: '这个页面返回的是 PDF，当前网页正文抓取会跳过非 HTML 内容。',
    errorUnsupportedContent:
      '这个页面不是可读取的 HTML 正文，所以 PathKeep 跳过了正文抓取。',
    errorRedirectBlocked:
      '这个页面跳进了登录或跳转边界，PathKeep 目前拿不到稳定的正文。',
    errorRateLimited: '上游网站暂时拒绝了抓取请求，稍后可以再试一次。',
    deterministicRuntimeSummary:
      'PathKeep 正在重建确定性证据，这一步会先完成，再继续处理网络型抓取。',
    contentFetchQueuedSummaryHost:
      '这条网页正文抓取还在排队，轮到时会重新访问 {host}。',
    contentFetchQueuedSummary:
      '这条网页正文抓取还在排队，轮到时会重新访问原页面。',
    contentFetchRunningSummaryHost:
      'PathKeep 正在重新访问 {host}，为这条记录补抓可读正文。',
    contentFetchRunningSummary: 'PathKeep 正在为这条记录补抓可读正文。',
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
    readyBody: '目前沒有排隊任務。新的重建、內容補抓與索引任務會顯示在這裡。',
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
    overviewHeadline: '先完成確定性分析，再慢慢補抓網頁正文',
    overviewBody:
      '大量的內容抓取 backlog 不代表系統卡住了。PathKeep 會先完成可以立即交付的確定性重建，再繼續處理網路型的網頁內容抓取。',
    queueSummaryTitle: 'AI 佇列',
    queueSummaryBody:
      'embedding 和助手任務會保存在封存佇列裡，所以前景操作結束後也能繼續完成。',
    runtimeSummaryTitle: '衍生資料佇列',
    runtimeSummaryBody:
      '確定性重建和 enrichment 任務會保留自己的執行記錄，方便你查看哪些已完成、哪些還需要處理。',
    recoveryTitle: '恢復',
    recoveryBody:
      '排隊任務會保存在封存裡。如果應用意外關閉，未完成的任務仍會留在這裡，重新開啟 PathKeep 後可以繼續查看或重試。',
    noRecoveryNotes: '目前沒有恢復備註。',
    pluginsTitle: 'Enrichment 外掛',
    modulesTitle: '確定性模組',
    recentAiJobs: '最近的 AI 任務',
    recentRuntimeJobs: '最近的衍生資料任務',
    recentJobsEmpty: '還沒有背景任務紀錄。',
    queuedCount: '排隊中',
    runningCount: '執行中',
    failedCount: '失敗',
    concurrency: 'Worker 數',
    queueStatePaused: '已暫停',
    queueStateLive: '執行中',
    lastActivity: '最近活動',
    lastCompletedAt: '上次完成',
    derivedTables: '衍生資料表',
    retryJob: '重試',
    cancelJob: '取消',
    createdAt: '建立時間',
    startedAt: '開始時間',
    finishedAt: '結束時間',
    noErrorDetails: '目前還沒有詳細資訊。',
    savedReadableContent: '已保存正文',
    storedRecordsLabel: '已保存列數',
    focusNow: '正在推進',
    focusNowBacklog: '背景仍有任務在執行，請重新整理以查看最新的活動項目。',
    focusNowIdle: '目前沒有活躍工作，佇列會在下一次重建後繼續。',
    needsReviewNow: '需要你處理',
    needsReviewIdle: '目前沒有需要人工處理的失敗項。',
    needsReviewBacklog: '還有 {count} 個失敗任務等你查看或重試。',
    contentFetchTitle: '網頁內容擷取',
    contentFetchBacklogBody:
      '已保存 {stored} 筆可讀網頁內容。剩下 {queued} 筆網路抓取會在背景慢慢補齊，所以重建不會被大佇列拖慢。',
    contentFetchRunningBody:
      '已保存 {stored} 筆可讀網頁內容。PathKeep 正在繼續補抓網頁正文。',
    contentFetchReadyBody:
      '已保存 {stored} 筆可讀網頁內容。現在沒有待處理的網頁正文抓取。',
    contentFetchFallbackBody:
      '網頁內容抓取會在背景替摘要與洞察補上更多可引用的本機證據。',
    titleNormalizationBody:
      '標題正規化會先在本機快速完成，幫助搜尋、摘要與洞察更快拿到穩定的證據標籤。',
    moduleAttentionBody: '{count} 個確定性模組仍在等待重建或需要重新檢查。',
    moduleHealthyBody: '所有已啟用的確定性模組目前都已是最新狀態。',
    moduleReadyCount: '已就緒模組',
    moduleAttentionCount: '待處理模組',
    runtimeHealthTitle: '執行邊界',
    runtimeHealthBody:
      '這裡把插件、確定性模組和恢復線索拆開顯示，讓你分辨現在是在排隊、執行、失敗，還是只是被延後處理。',
    recentActivityTitle: '最近活動',
    recentActivityBody:
      '按任務家族查看最近的 AI 與衍生資料工作，方便你重試失敗項、取消排隊項，或確認背景工作仍在前進。',
    errorPdf: '這個頁面回傳的是 PDF，目前網頁正文抓取會跳過非 HTML 內容。',
    errorUnsupportedContent:
      '這個頁面不是可讀的 HTML 正文，所以 PathKeep 跳過了正文抓取。',
    errorRedirectBlocked:
      '這個頁面跳進了登入或重新導向邊界，PathKeep 目前拿不到穩定的正文。',
    errorRateLimited: '上游網站暫時拒絕了抓取請求，稍後可以再試一次。',
    deterministicRuntimeSummary:
      'PathKeep 正在重建確定性證據，這一步會先完成，再繼續處理較慢的網路抓取。',
    contentFetchQueuedSummaryHost:
      '這筆網頁正文抓取還在排隊，輪到時會重新造訪 {host}。',
    contentFetchQueuedSummary:
      '這筆網頁正文抓取還在排隊，輪到時會重新造訪原頁面。',
    contentFetchRunningSummaryHost:
      'PathKeep 正在重新造訪 {host}，為這筆紀錄補抓可讀正文。',
    contentFetchRunningSummary: 'PathKeep 正在為這筆紀錄補抓可讀正文。',
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
