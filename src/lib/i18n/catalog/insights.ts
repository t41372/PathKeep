/**
 * @file insights.ts
 * @description Owns legacy insights-facing copy that still ships across locales.
 * @module i18n/catalog
 *
 * ## Responsibilities
 * - Keep the `insights` namespace aligned across `en`, `zh-CN`, and `zh-TW`.
 * - Preserve the exact shipped keys and values while the monolithic catalog is being decomposed.
 *
 * ## Not responsible for
 * - Translator runtime behavior such as interpolation, locale detection, or fallback resolution.
 * - Copy that belongs to other namespaces.
 *
 * ## Dependencies
 * - None. This module is intentionally data-only so remaining insights copy can be moved without translator drift.
 *
 * ## Performance notes
 * - Static literal data only. Isolating this namespace keeps copy churn out of translator/runtime helper modules.
 */

/**
 * Provides the canonical `insights` namespace payload for the shipped locales.
 *
 * This split exists so future copy edits can stay local to one namespace owner without reopening
 * the monolithic catalog file. Keep the nested key structure and literal values exactly aligned
 * with the legacy source until the barrel assembly cutover happens.
 */
export const insightsNamespaceCatalog = {
  en: {
    archiveNotInitializedTitle: 'Archive not set up',
    archiveNotInitializedDescription:
      'Run at least one backup before insights can analyze your history.',
    loadingLabel: 'Loading insights',
    unavailableTitle: 'Insights unavailable',
    emptyTitle: 'No insights yet',
    emptyDescription:
      'Insights will appear here after your first analysis run.',
    intelligenceEyebrow: 'INSIGHTS',
    goToSetup: 'Go to setup',
    refreshInsights: 'Refresh',
    openExplorer: 'Browse history',
    askAssistant: 'Ask assistant',
    assistantSummaryPrompt:
      'What are the biggest changes in my recent browsing?',
    scopedViewTitle: 'Profile-scoped view',
    scopedViewBody:
      'Cards, topics, threads, and summaries are limited to {profile}. Coverage, storage analytics, and growth signals still use the full archive.',
    archiveWideBadge: 'Archive-wide metrics',
    refreshAttentionTitle: 'Refresh needs attention',
    refreshQueuedTitle: 'Refresh queued',
    refreshQueuedBody:
      'Deterministic rebuild job #{jobId} is now in Background Jobs. Keep browsing while PathKeep refreshes the latest derived evidence.',
    overviewTitle: 'Latest analysis',
    overviewHeadline:
      'Start with the clearest signals, then decide where to dig deeper',
    overviewBody:
      'Insights should first tell you what changed recently, then open into query groups, research threads, reference pages, and source patterns. This page now follows that reading order.',
    archiveWideBody:
      'You are viewing archive-wide analysis. If you switch to one browser profile, cards and research signals narrow with it, while storage and growth metrics stay archive-wide.',
    queueReviewBody:
      'Rebuilds and page-text fetches keep moving in the background. This surface only keeps the most important runtime clue visible here; use Jobs for the full queue review.',
    coreHistory: 'Core history',
    otherData: 'Other data',
    canonicalArchive: 'Canonical archive',
    sourceEvidence: 'Source evidence',
    searchProjection: 'Search projection',
    intelligenceProjection: 'Intelligence projection',
    semanticIndex: 'Semantic index',
    contentBlobs: 'Content blobs',
    auditArtifacts: 'Audit artifacts',
    temporaryFiles: 'Temporary files',
    window: 'TIME RANGE',
    windowDaysCompact: '{days}d',
    cards: 'HIGHLIGHTS',
    topics: 'TOPICS',
    coverage: 'COVERAGE',
    generatedAt: 'Updated {time}',
    cardsDescription: 'Key patterns from your history',
    topicsDescription: "Topics you've been exploring",
    coverageDescription: 'How much of this period has been analyzed',
    onThisDay: 'ON THIS DAY',
    nothingForDayEyebrow: 'ON THIS DAY',
    nothingForDayTitle: 'Nothing from this day',
    nothingForDayDescription:
      'No browsing history was found for this date in earlier years.',
    siteAnalytics: 'TOP SITES',
    currentEvidenceSample: 'From this analysis',
    noSiteAnalyticsEyebrow: 'TOP SITES',
    noSiteAnalyticsTitle: 'No site data yet',
    noSiteAnalyticsDescription:
      'Site stats appear after the first insights run.',
    queryEvolution: 'QUERY EVOLUTION',
    queryEvolutionDescription:
      'Recent search changes grouped into a trail you can follow.',
    queryEvolutionEmptyTitle: 'No query ladders yet',
    queryEvolutionEmptyDescription:
      'Search trails appear after Chromium search terms are recorded.',
    queryGroups: 'QUERY GROUPS',
    queryGroupsEmptyTitle: 'No query groups yet',
    queryGroupsEmptyDescription:
      'Grouped search keywords appear after the search evidence is rebuilt.',
    queryEvolutionSteps: '{count} steps',
    queryStageBroad: 'Broad',
    queryStageNarrowing: 'Narrowing',
    queryStageBroadening: 'Broadening',
    queryStageCompare: 'Compare',
    queryStageSiteRestrict: 'Site restrict',
    queryStageErrorDriven: 'Error driven',
    periodicSummary: 'SUMMARY',
    periodicSummaryFallbackWindow:
      'Captured {visits} visits across {domains} domains in the current window.',
    periodicSummaryFallbackDomains: 'Most activity clustered around {domains}.',
    snapshotLabel: 'As of {time}',
    referencePages: 'REFERENCE PAGES',
    referencePagesEmptyTitle: 'No reference pages yet',
    referencePagesEmptyDescription:
      'Pages that keep resurfacing across searches will show up here.',
    referencePagesBody:
      'Reused across {groups} groups, {threads} threads, and {revisits} revisits.',
    sourceEffectiveness: 'SOURCE EFFECTIVENESS',
    sourceEffectivenessEmptyTitle: 'No source effectiveness data yet',
    sourceEffectivenessEmptyDescription:
      'This view fills in once PathKeep can compare which sources keep leading to useful results.',
    sourceEffectivenessBody:
      'Appeared in {groups} groups, {references} reference pages, and {landings} stable landings.',
    deterministicModules: 'DETERMINISTIC MODULES',
    deterministicModulesDescription:
      'Review which deterministic modules are fresh, stale, disabled, or waiting for rebuild.',
    deterministicModulesEmptyTitle: 'No module status yet',
    deterministicModulesEmptyDescription:
      'Run a deterministic rebuild to populate module status and trace details.',
    threads: 'Threads',
    cardsStat: 'Highlights',
    topicsStat: 'Topics',
    topicTimeline: 'TOPIC TIMELINE',
    lastDays: 'Last {days} days',
    windowAxis: '{days}-day window',
    insightCards: 'HIGHLIGHTS',
    explainable: 'Tap to see sources',
    evidenceItems: '{count} pages · {days} days',
    chromiumEnhanced: 'Chromium-enhanced',
    crossBrowserSafe: 'All browsers',
    explain: 'Why this?',
    explainability: 'WHY THIS INSIGHT',
    explainCardPrompt: 'Why does "{title}" appear in my insights?',
    usedToExplain: 'Pages used to generate this insight.',
    refreshingAction: 'Refreshing…',
    explainingAction: 'Analyzing…',
    openLoopSignal: 'Unfinished research',
    revisitSignal: 'Frequently revisited',
    focusBalanceSignal: 'Focus check',
    genericCard: 'Insight',
    storageAnalytics: 'STORAGE',
    growthSignal: 'RECENT GROWTH',
    storageAnalyticsDescription:
      'See what uses disk space, what can be reclaimed, and what the latest backup added.',
    trackedStorage: 'Total used',
    reclaimableSpace: 'Reclaimable',
    dominantStorage: 'Largest category',
    coreStorage: 'Archive',
    auditStorage: 'Audit logs',
    exportStorage: 'Exports',
    rebuildableStorage: 'Temporary files',
    latestRunGrowth: 'Last backup added',
    latestRunGrowthBody:
      'Added {visits} visits, {urls} URLs, and {downloads} downloads.',
    openGrowthAuditRun: 'Open this backup in Audit',
    noGrowthEvidenceTitle: 'No growth data yet',
    noGrowthEvidenceDescription:
      'Run at least one backup to see how your archive grows over time.',
    spotlightTitle: 'Start here',
    spotlightBody:
      'These panels surface the most immediately useful read-back: the strongest highlights, site patterns, and the current summary window.',
    researchSignalsTitle: 'Research signals',
    researchSignalsBody:
      'Query groups, topic momentum, and query evolution show how your recent browsing narrowed a question, reopened a thread, or shifted focus.',
    evidenceLibraryTitle: 'Evidence and system health',
    evidenceLibraryBody:
      'Reference pages, source effectiveness, storage growth, and module status explain where the insights came from and whether the runtime is keeping up.',
  },
  'zh-CN': {
    archiveNotInitializedTitle: '数据库尚未初始化',
    archiveNotInitializedDescription:
      '需要先初始化数据库并完成至少一次备份，才能生成洞察。',
    loadingLabel: '正在加载洞察',
    unavailableTitle: '洞察暂不可用',
    emptyTitle: '还没有洞察数据',
    emptyDescription: '完成首次备份后，PathKeep 就会开始为你生成浏览洞察。',
    intelligenceEyebrow: '智能分析',
    goToSetup: '前往设置',
    refreshInsights: '刷新洞察',
    openExplorer: '打开历史浏览器',
    askAssistant: '问问 AI 助手',
    assistantSummaryPrompt: '总结一下我最近浏览洞察中最明显的变化。',
    scopedViewTitle: '当前为浏览器范围视图',
    scopedViewBody:
      '洞察卡片、主题、线程和摘要只会显示 {profile}。覆盖率、存储统计和增长信号仍然使用全部存档。',
    archiveWideBadge: '全部存档统计',
    refreshAttentionTitle: '洞察刷新遇到问题',
    refreshQueuedTitle: '刷新已加入队列',
    refreshQueuedBody:
      '确定性重新生成任务 #{jobId} 已进入后台任务。你可以继续浏览，PathKeep 会在后台刷新最新的派生证据。',
    overviewTitle: '分析快照',
    overviewHeadline: '先看结论，再决定要不要继续深挖',
    overviewBody:
      '这一页应该先告诉你最近发生了什么，再把查询组、线索、参考页和来源效果分层展开，而不是把所有模块一次性堆在你面前。',
    archiveWideBody:
      '当前显示的是整个 archive 的分析结果。切换浏览器范围后，卡片与线索会跟着收窄，但存储与增长指标仍保持 archive-wide。',
    queueReviewBody:
      '重建与网页内容抓取会继续在后台推进。这里只保留最重要的运行线索，完整进度与失败处理请到 Jobs 页面查看。',
    coreHistory: '核心浏览记录',
    otherData: '其他数据',
    canonicalArchive: '规范化存档',
    sourceEvidence: '来源证据',
    searchProjection: '搜索投影',
    intelligenceProjection: '智能投影',
    semanticIndex: '语义索引',
    contentBlobs: '正文缓存',
    auditArtifacts: '审计产物',
    temporaryFiles: '临时文件',
    window: '时间范围',
    windowDaysCompact: '{days} 天',
    cards: '洞察卡片',
    topics: '话题',
    coverage: '覆盖率',
    generatedAt: '生成于 {time}',
    cardsDescription: '基于浏览记录的洞察摘要',
    topicsDescription: '你关注的话题分类',
    coverageDescription: '当前时间范围内的数据覆盖率',
    onThisDay: '历史上的今天',
    nothingForDayEyebrow: '历史上的今天',
    nothingForDayTitle: '今天没有历史记录',
    nothingForDayDescription:
      '在过去的年份里，没有找到和今天同日期的浏览记录。',
    siteAnalytics: '网站统计',
    currentEvidenceSample: '来自这次分析',
    noSiteAnalyticsEyebrow: '网站',
    noSiteAnalyticsTitle: '还没有网站统计',
    noSiteAnalyticsDescription:
      '首次生成洞察并包含浏览数据后，这里会显示你最常访问的网站。',
    queryEvolution: '搜索演化',
    queryEvolutionDescription: '把最近的搜索变化整理成一条可追踪的路径。',
    queryEvolutionEmptyTitle: '还没有搜索演化',
    queryEvolutionEmptyDescription:
      'Chromium 记录到搜索词后，这里才会显示搜索轨迹。',
    queryGroups: '查询组',
    queryGroupsEmptyTitle: '还没有查询组',
    queryGroupsEmptyDescription:
      '搜索证据重建完成后，这里会显示分组后的关键词。',
    queryEvolutionSteps: '{count} 步',
    queryStageBroad: '宽泛',
    queryStageNarrowing: '收窄',
    queryStageBroadening: '放宽',
    queryStageCompare: '比较',
    queryStageSiteRestrict: '站点限定',
    queryStageErrorDriven: '错误驱动',
    periodicSummary: '阶段总结',
    periodicSummaryFallbackWindow:
      '当前时间范围共捕获了 {visits} 条访问记录，覆盖 {domains} 个网站域名。',
    periodicSummaryFallbackDomains: '主要活动集中在 {domains}。',
    snapshotLabel: '快照 {time}',
    referencePages: '参考页',
    referencePagesEmptyTitle: '还没有参考页',
    referencePagesEmptyDescription:
      '当页面在不同搜索里反复出现后，这里就会显示。',
    referencePagesBody:
      '出现在 {groups} 个查询组、{threads} 条线索里，并被重访了 {revisits} 次。',
    sourceEffectiveness: '来源效果',
    sourceEffectivenessEmptyTitle: '还没有来源效果数据',
    sourceEffectivenessEmptyDescription:
      '等 PathKeep 能比较哪些来源最常带来有用结果后，这里才会显示。',
    sourceEffectivenessBody:
      '出现在 {groups} 个查询组、{references} 个参考页和 {landings} 次稳定落点中。',
    deterministicModules: '确定性模块',
    deterministicModulesDescription:
      '查看哪些确定性模块是最新、已过期、已关闭，或还在等待重建。',
    deterministicModulesEmptyTitle: '还没有模块状态',
    deterministicModulesEmptyDescription:
      '先运行一次确定性重建，这里才会显示模块状态和 trace 细节。',
    threads: '研究线索',
    cardsStat: '卡片',
    topicsStat: '话题',
    topicTimeline: '话题趋势',
    lastDays: '最近 {days} 天',
    windowAxis: '{days} 天',
    insightCards: '洞察卡片',
    explainable: '可查看详情和来源',
    evidenceItems: '{count} 条记录 · {days} 天',
    chromiumEnhanced: 'Chromium 浏览器增强',
    crossBrowserSafe: '全浏览器通用',
    explain: '查看详情',
    explainability: '详情解读',
    explainCardPrompt: '解释一下为什么「{title}」会出现在我的洞察卡片里。',
    usedToExplain: '以下记录用于解读当前选中的洞察。',
    refreshingAction: '正在刷新洞察',
    explainingAction: '正在分析洞察',
    openLoopSignal: '反复查看但未完成',
    revisitSignal: '频繁重复访问',
    focusBalanceSignal: '专注度变化',
    genericCard: '洞察卡片',
    storageAnalytics: '存储空间',
    growthSignal: '数据增长',
    storageAnalyticsDescription:
      '查看本地数据占用情况，包括哪些数据占空间最多、有多少可以清理，以及最近一次备份新增了多少。',
    trackedStorage: '已用空间',
    reclaimableSpace: '可清理',
    dominantStorage: '最大占用',
    coreStorage: '核心数据',
    auditStorage: '审计记录',
    exportStorage: '导出文件',
    rebuildableStorage: '可重建的临时数据',
    latestRunGrowth: '最近备份新增',
    latestRunGrowthBody:
      '最近一次备份新增了 {visits} 条访问、{urls} 个网址和 {downloads} 条下载记录。',
    openGrowthAuditRun: '在审计日志中查看这次备份',
    noGrowthEvidenceTitle: '还没有数据增长记录',
    noGrowthEvidenceDescription:
      '完成第一次备份后，这里会显示每次备份新增的数据量。',
    spotlightTitle: '先看这里',
    spotlightBody:
      '把最值得打开的回顾、站点模式与阶段总结放在最前面，帮助你先抓住重点。',
    researchSignalsTitle: '研究信号',
    researchSignalsBody:
      '查询组、话题趋势和搜索演化展示的是最近这段时间你如何缩小问题、重新打开线索，以及主题怎么升温或降温。',
    evidenceLibraryTitle: '证据与健康状态',
    evidenceLibraryBody:
      '参考页、来源效果、存储增长和模块状态更适合用来解释这些洞察从哪里来，以及系统现在健不健康。',
  },
  'zh-TW': {
    archiveNotInitializedTitle: '資料庫尚未初始化',
    archiveNotInitializedDescription:
      '需要先初始化資料庫並完成至少一次備份，才能產生洞察。',
    loadingLabel: '正在載入洞察',
    unavailableTitle: '洞察暫時無法使用',
    emptyTitle: '還沒有洞察資料',
    emptyDescription: '完成首次備份後，PathKeep 就會開始為你產生瀏覽洞察。',
    intelligenceEyebrow: '智慧分析',
    goToSetup: '前往設定',
    refreshInsights: '重新整理洞察',
    openExplorer: '開啟歷史瀏覽器',
    askAssistant: '問問 AI 助手',
    assistantSummaryPrompt: '總結一下我最近瀏覽洞察中最明顯的變化。',
    scopedViewTitle: '目前為瀏覽器範圍視圖',
    scopedViewBody:
      '洞察卡片、主題、執行緒和摘要只會顯示 {profile}。涵蓋率、儲存統計和成長訊號仍然使用全部封存。',
    archiveWideBadge: '全部封存統計',
    refreshAttentionTitle: '洞察重新整理遇到問題',
    refreshQueuedTitle: '重新整理已加入佇列',
    refreshQueuedBody:
      '確定性重新產生任務 #{jobId} 已進入背景工作。你可以繼續瀏覽，PathKeep 會在背景重新整理最新的派生證據。',
    overviewTitle: '分析快照',
    overviewHeadline: '先看結論，再決定要不要繼續深挖',
    overviewBody:
      '這一頁應該先告訴你最近發生了什麼，再把查詢群組、線索、參考頁與來源效果分層展開，而不是把所有模組一次堆給你。',
    archiveWideBody:
      '目前顯示的是整個 archive 的分析結果。切換瀏覽器範圍後，卡片與研究訊號會跟著收窄，但儲存與成長指標仍維持 archive-wide。',
    queueReviewBody:
      '重建與網頁內容抓取會繼續在背景推進。這裡只保留最重要的執行線索，完整進度與失敗處理請到 Jobs 頁面查看。',
    coreHistory: '核心瀏覽紀錄',
    otherData: '其他資料',
    canonicalArchive: '規範化封存',
    sourceEvidence: '來源證據',
    searchProjection: '搜尋投影',
    intelligenceProjection: '智慧投影',
    semanticIndex: '語意索引',
    contentBlobs: '正文快取',
    auditArtifacts: '稽核產物',
    temporaryFiles: '暫存檔',
    window: '時間範圍',
    windowDaysCompact: '{days} 天',
    cards: '洞察卡片',
    topics: '主題',
    coverage: '涵蓋率',
    generatedAt: '產生於 {time}',
    cardsDescription: '根據瀏覽紀錄產生的洞察摘要',
    topicsDescription: '你關注的主題分類',
    coverageDescription: '目前時間範圍內的資料涵蓋率',
    onThisDay: '歷史上的今天',
    nothingForDayEyebrow: '歷史上的今天',
    nothingForDayTitle: '今天沒有歷史紀錄',
    nothingForDayDescription:
      '在過去的年份裡，沒有找到和今天同日期的瀏覽紀錄。',
    siteAnalytics: '網站統計',
    currentEvidenceSample: '來自這次分析',
    noSiteAnalyticsEyebrow: '網站',
    noSiteAnalyticsTitle: '還沒有網站統計',
    noSiteAnalyticsDescription:
      '首次產生洞察並包含瀏覽資料後，這裡會顯示你最常造訪的網站。',
    queryEvolution: '搜尋演化',
    queryEvolutionDescription: '把最近的搜尋變化整理成一條可追蹤的路徑。',
    queryEvolutionEmptyTitle: '還沒有搜尋演化',
    queryEvolutionEmptyDescription:
      'Chromium 記錄到搜尋詞後，這裡才會顯示搜尋軌跡。',
    queryGroups: '查詢群組',
    queryGroupsEmptyTitle: '還沒有查詢群組',
    queryGroupsEmptyDescription:
      '搜尋證據重建完成後，這裡會顯示分組後的關鍵字。',
    queryEvolutionSteps: '{count} 步',
    queryStageBroad: '寬泛',
    queryStageNarrowing: '收窄',
    queryStageBroadening: '放寬',
    queryStageCompare: '比較',
    queryStageSiteRestrict: '站點限定',
    queryStageErrorDriven: '錯誤驅動',
    periodicSummary: '階段總結',
    periodicSummaryFallbackWindow:
      '目前時間範圍共捕獲了 {visits} 筆瀏覽紀錄，涵蓋 {domains} 個網站網域。',
    periodicSummaryFallbackDomains: '主要活動集中在 {domains}。',
    snapshotLabel: '快照 {time}',
    referencePages: '參考頁',
    referencePagesEmptyTitle: '還沒有參考頁',
    referencePagesEmptyDescription:
      '當頁面在不同搜尋裡反覆出現後，這裡就會顯示。',
    referencePagesBody:
      '出現在 {groups} 個查詢群組、{threads} 條線索裡，並被重訪了 {revisits} 次。',
    sourceEffectiveness: '來源效果',
    sourceEffectivenessEmptyTitle: '還沒有來源效果資料',
    sourceEffectivenessEmptyDescription:
      '等 PathKeep 能比較哪些來源最常帶來有用結果後，這裡才會顯示。',
    sourceEffectivenessBody:
      '出現在 {groups} 個查詢群組、{references} 個參考頁和 {landings} 次穩定落點中。',
    deterministicModules: '確定性模組',
    deterministicModulesDescription:
      '查看哪些確定性模組是最新、已過期、已關閉，或仍在等待重建。',
    deterministicModulesEmptyTitle: '還沒有模組狀態',
    deterministicModulesEmptyDescription:
      '先執行一次確定性重建，這裡才會顯示模組狀態和 trace 細節。',
    threads: '研究線索',
    cardsStat: '卡片',
    topicsStat: '主題',
    topicTimeline: '主題趨勢',
    lastDays: '最近 {days} 天',
    windowAxis: '{days} 天',
    insightCards: '洞察卡片',
    explainable: '可查看詳情和來源',
    evidenceItems: '{count} 筆紀錄 · {days} 天',
    chromiumEnhanced: 'Chromium 瀏覽器增強',
    crossBrowserSafe: '全瀏覽器通用',
    explain: '查看詳情',
    explainability: '詳情解讀',
    explainCardPrompt: '解釋一下為什麼「{title}」會出現在我的洞察卡片裡。',
    usedToExplain: '以下紀錄用於解讀目前選取的洞察。',
    refreshingAction: '正在重新整理洞察',
    explainingAction: '正在分析洞察',
    openLoopSignal: '反覆查看但未完成',
    revisitSignal: '頻繁重複瀏覽',
    focusBalanceSignal: '專注度變化',
    genericCard: '洞察卡片',
    storageAnalytics: '儲存空間',
    growthSignal: '資料成長',
    storageAnalyticsDescription:
      '查看本機資料佔用情況，包括哪些資料佔用最多、有多少可以清理，以及最近一次備份新增了多少。',
    trackedStorage: '已用空間',
    reclaimableSpace: '可清理',
    dominantStorage: '最大佔用',
    coreStorage: '核心資料',
    auditStorage: '審計紀錄',
    exportStorage: '匯出檔案',
    rebuildableStorage: '可重建的暫存資料',
    latestRunGrowth: '最近備份新增',
    latestRunGrowthBody:
      '最近一次備份新增了 {visits} 筆瀏覽、{urls} 個網址和 {downloads} 筆下載紀錄。',
    openGrowthAuditRun: '在稽核日誌中查看這次備份',
    noGrowthEvidenceTitle: '還沒有資料成長紀錄',
    noGrowthEvidenceDescription:
      '完成第一次備份後，這裡會顯示每次備份新增的資料量。',
    spotlightTitle: '先看這裡',
    spotlightBody:
      '把最值得打開的回顧、站點模式與階段總結放在最前面，幫助你先抓住重點。',
    researchSignalsTitle: '研究訊號',
    researchSignalsBody:
      '查詢群組、主題趨勢與搜尋演化展示的是最近這段時間你如何收斂問題、重新打開線索，以及主題怎麼升溫或降溫。',
    evidenceLibraryTitle: '證據與健康狀態',
    evidenceLibraryBody:
      '參考頁、來源效果、儲存成長與模組狀態更適合用來解釋這些洞察從哪裡來，以及系統現在健不健康。',
  },
} as const
