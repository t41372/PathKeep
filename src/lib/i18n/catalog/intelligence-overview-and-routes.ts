/**
 * @file intelligence-overview-and-routes.ts
 * @description Defines one focused intelligence translation owner so the overall intelligence namespace no longer lives in a single mega-file.
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Provide one bounded subsection of the intelligence namespace for en, zh-CN, and zh-TW.
 * - Keep related intelligence copy together without reintroducing a second language-specific owner.
 *
 * ## Not responsible for
 * - Other intelligence copy that belongs to different subgroup owners
 * - Translator creation, flattening, or language resolution
 *
 * ## Dependencies
 * - No runtime dependencies; `catalog-runtime.ts` imports this static dictionary during catalog assembly.
 *
 * ## Performance notes
 * - Static dictionary data only; keep this file side-effect free so large locale loads stay cheap.
 */

/**
 * Keeps the intelligence namespace subsection aligned across shipping locales so copy updates stay in one owner.
 */
export const intelligenceOverviewAndRoutesNamespace = {
  en: {
    statusReadyLabel: 'Smart search ready',
    statusReadyDescription:
      '{count} records indexed — natural language search is available.',
    statusRebuildingLabel: 'Rebuilding search index',
    statusRebuildingDescription:
      'The search index is being rebuilt. Keyword search still works in the meantime.',
    statusQueuedLabel: 'Index update queued',
    statusQueuedDescription:
      'Index work is in the queue and will start once current tasks finish.',
    statusPausedLabel: 'AI tasks paused',
    statusPausedDescription:
      'AI tasks are paused. Resume them in Settings to continue indexing.',
    statusFailedLabel: 'Index needs attention',
    statusFailedDescription:
      'The last indexing job failed. Check your AI provider settings and try again.',
    statusStaleLabel: 'Index needs refresh',
    statusStaleDescription:
      'The semantic index is behind recent imports or enrichment changes. Rebuild it to restore full semantic coverage.',
    statusDegradedLabel: 'Smart search limited',
    statusDegradedDescription:
      'Keyword search still works, but the AI provider is having trouble right now.',
    statusBlockedLabel: 'Archive not ready',
    statusBlockedDescription:
      'Set up and unlock the archive before using smart search.',
    statusDisabledLabel: 'AI features off',
    statusDisabledDescription:
      'Backup and search still work. Turn on AI in Settings for smart search and the assistant.',
    statusEmptyLabel: 'No index yet',
    statusEmptyDescription:
      'Build the search index first to enable natural language search.',
    noScore: 'No score',
    highConfidence: 'High confidence',
    relevant: 'Relevant',
    weakMatch: 'Weak match',
    answerReady: 'Answer ready',
    queued: 'Queued',
    evidenceMissing: 'Not enough evidence',
    assistantFailed: 'Something went wrong',
    cancelled: 'Cancelled',
    inProgress: 'Working',
    timeRangeLabel: 'Time range',
    rangeDay: 'Day',
    rangeWeek: 'Week',
    rangeMonth: 'Month',
    rangeQuarter: 'Quarter',
    rangeYear: 'Year',
    rangeAll: 'All time',
    rangeCustom: 'Custom',
    customStart: 'Start date',
    customEnd: 'End date',
    applyRange: 'Apply',
    digestTitle: 'Period Summary',
    digestUnavailable:
      'Summary data is not available yet. Complete a backup to start.',
    digestVisits: 'Visits',
    digestSearches: 'Searches',
    digestNewSites: 'New Sites',
    digestDeepRead: 'Deep Reads',
    digestRefind: 'Refinds',
    trendLabel: '{direction} {percent}%',
    visits: 'visits',
    onThisDayTitle: 'On This Day',
    onThisDayEyebrow: 'ON THIS DAY',
    onThisDayEmpty: 'No history found for this date in past years.',
    onThisDayVisits: '{count} pages visited',
    onThisDayMore: 'See more years →',
    onThisDayCollapse: 'Collapse years',
    onThisDayDeepDive: '{count} deep-dive sessions',
    topSitesTitle: 'Top Sites',
    topSitesEmpty: 'Not enough browsing data to show top sites yet.',
    insightAccessEyebrow: 'Quick Access',
    insightAccessTitle: 'Open Full Insights',
    insightAccessDayLabel: 'Open by day',
    insightAccessDomainLabel: 'Open by domain',
    openDayInsights: 'Open day insights',
    openDomainInsights: 'Open domain insights',
    dayInsightsTitle: 'Day Insights',
    dayInsightsSubtitle:
      'Review the full browsing rhythm, standout sites, and research signals for this exact local day.',
    dayInsightsInvalidDate: 'This date could not be recognized.',
    dayInsightsEmpty: 'No reviewable insights are available for this day yet.',
    dayInsightsBack: 'Back to overview',
    dayInsightsOpenExplorer: 'Open exact-day evidence',
    entityBackToOverview: 'Back to overview',
    entityOpenExplorer: 'Open evidence in Explorer',
    queryFamilyRouteTitle: 'Query Family',
    queryFamilyRouteSubtitle:
      'Review one search family as a reusable route-first entity inside the current time window.',
    queryFamilyRelatedTrails: 'Related trails',
    queryFamilyQueriesTitle: 'Queries in this family',
    queryFamilyTrailsTitle: 'Related search trails',
    refindRouteTitle: 'Refind Page',
    refindRouteSubtitle:
      'Review why this page keeps resurfacing, when it came back, and which trails brought it in.',
    refindFactorsTitle: 'Refind factors',
    refindRecentDaysTitle: 'Recent days',
    sessionRouteTitle: 'Session Insights',
    sessionRouteSubtitle:
      'Review one browsing session as a reusable entity instead of re-opening it only inside Explorer.',
    sessionRouteVisitsTitle: 'Session visits',
    sessionRouteTrailsTitle: 'Trails inside this session',
    sessionRouteOpenInsights: 'Open session insights',
    trailRouteTitle: 'Trail Insights',
    trailRouteSubtitle:
      'Review this search trail as a shared route-first entity with query evolution and member evidence.',
    trailRouteMembersTitle: 'Trail members',
    trailRouteOpenInsights: 'Open trail insights',
    trailRouteOpenSession: 'Open session insights',
    trailRouteVisitCount: '{count} visits',
    trailRouteDepthLabel: 'Depth',
    compareSetRouteTitle: 'Compare Set',
    compareSetRouteSubtitle:
      'Review this side-by-side comparison as a shared entity with reusable trail, day, and domain context.',
    compareSetRoutePagesTitle: 'Compared pages',
    compareSetRouteRecentDaysTitle: 'Recent compare days',
    compareSetRouteOpenTrail: 'Open trail insights',
    compareSetRouteOpenSession: 'Open session insights',
    compareSetRouteDomainsLabel: 'Domains',
    compareSetFocusTitle: 'Focused compare set',
    compareSetFocusBody:
      'This trail is being viewed through the compare set for "{query}" across {count} pages.',
    compareSetDayFocusBody:
      'This day is part of the compare set for "{query}" across {count} pages.',
    compareSetDomainFocusBody:
      'This domain participates in the compare set for "{query}" across {count} pages.',
    compareSetFocusBadge: 'Focused',
    pathFlowFocusTitle: 'Focused path flow',
    pathFlowFocusBody:
      'This domain is being viewed through the repeating path flow: {flow}',
    dayInsightsHourlyTitle: 'Hourly Activity',
    dayInsightsTopSitesTitle: 'Standout Sites',
    dayInsightsActivityMixTitle: 'Activity Mix',
    dayInsightsQueryFamiliesTitle: 'Query Evolution',
    dayInsightsRefindsTitle: 'Refinds',
  },
  'zh-CN': {
    statusReadyLabel: '智能搜索已就绪',
    statusReadyDescription:
      '已为 {count} 条记录建立索引，可以用自然语言搜索了。',
    statusRebuildingLabel: '正在重建搜索索引',
    statusRebuildingDescription:
      '正在重建智能搜索索引，期间搜索会暂时回到关键词模式。',
    statusQueuedLabel: '索引任务排队中',
    statusQueuedDescription:
      '索引任务已加入队列，会在当前 AI 任务完成后自动开始。',
    statusPausedLabel: 'AI 任务已暂停',
    statusPausedDescription:
      'AI 任务已暂停，需要你手动恢复或清空队列才会继续。',
    statusFailedLabel: '索引出现问题',
    statusFailedDescription:
      '最近一次索引任务失败了。请检查 AI 服务配置，然后重试。',
    statusStaleLabel: '索引需要刷新',
    statusStaleDescription:
      '语义索引落后于当前导入或增强数据。重新构建后才能覆盖最新语义内容。',
    statusDegradedLabel: '智能搜索受限',
    statusDegradedDescription:
      '目前只能用关键词搜索，AI 服务暂时无法正常工作。',
    statusBlockedLabel: '需要先完成初始设置',
    statusBlockedDescription:
      '请先完成初始设置并解锁数据库，然后才能使用智能功能。',
    statusDisabledLabel: 'AI 功能已关闭',
    statusDisabledDescription:
      '备份和搜索等核心功能正常可用，智能搜索和 AI 助手目前已关闭。',
    statusEmptyLabel: '索引为空',
    statusEmptyDescription:
      '还没有建立搜索索引。请先构建索引，才能使用智能搜索。',
    noScore: '无评分',
    highConfidence: '高度匹配',
    relevant: '相关',
    weakMatch: '弱匹配',
    answerReady: '回答已完成',
    queued: '排队中',
    evidenceMissing: '找不到相关记录',
    assistantFailed: '回答失败',
    cancelled: '已取消',
    inProgress: '处理中',
    timeRangeLabel: '时间范围',
    rangeDay: '日',
    rangeWeek: '周',
    rangeMonth: '月',
    rangeQuarter: '季',
    rangeYear: '年',
    rangeAll: '全部时间',
    rangeCustom: '自定义',
    customStart: '开始日期',
    customEnd: '结束日期',
    applyRange: '应用',
    digestTitle: '时段概览',
    digestUnavailable: '概览数据暂不可用，完成一次备份后即可查看。',
    digestVisits: '页面访问',
    digestSearches: '搜索次数',
    digestNewSites: '新网站',
    digestDeepRead: '深度阅读',
    digestRefind: '重找页面',
    trendLabel: '{direction} {percent}%',
    visits: '次',
    onThisDayTitle: '历史上的今天',
    onThisDayEyebrow: '历史上的今天',
    onThisDayEmpty: '过去的年份中没有找到今天的浏览记录。',
    onThisDayVisits: '浏览了 {count} 个页面',
    onThisDayMore: '查看更多年份 →',
    onThisDayCollapse: '收起年份',
    onThisDayDeepDive: '{count} 次深度研究会话',
    topSitesTitle: '最常访问',
    topSitesEmpty: '浏览数据不足，暂无法显示最常访问网站。',
    insightAccessEyebrow: '直达入口',
    insightAccessTitle: '打开完整洞察',
    insightAccessDayLabel: '按日期查看',
    insightAccessDomainLabel: '按域名查看',
    openDayInsights: '打开当日洞察',
    openDomainInsights: '打开域名洞察',
    dayInsightsTitle: '当日洞察',
    dayInsightsSubtitle: '查看这一天的完整浏览热度、重点网站与研究信号。',
    dayInsightsInvalidDate: '无法识别这个日期。',
    dayInsightsEmpty: '这一天暂时没有可显示的洞察。',
    dayInsightsBack: '返回概览',
    dayInsightsOpenExplorer: '打开当天证据',
    entityBackToOverview: '返回概览',
    entityOpenExplorer: '在 Explorer 中打开证据',
    queryFamilyRouteTitle: '查询族',
    queryFamilyRouteSubtitle:
      '把这一组相关搜索当成可复用的 route-first 实体来审阅，而不是只留在卡片里。',
    queryFamilyRelatedTrails: '相关旅程',
    queryFamilyQueriesTitle: '这一族里的查询',
    queryFamilyTrailsTitle: '相关搜索旅程',
    refindRouteTitle: '重找页面',
    refindRouteSubtitle:
      '查看这页为何反复出现、最近在哪些日期回来，以及哪些搜索旅程把它带进来。',
    refindFactorsTitle: '重找因素',
    refindRecentDaysTitle: '最近出现的日期',
    sessionRouteTitle: '会话洞察',
    sessionRouteSubtitle:
      '把这段浏览会话当成共享实体来查看，而不是只能在 Explorer 里临时展开。',
    sessionRouteVisitsTitle: '会话内页面',
    sessionRouteTrailsTitle: '会话中的搜索旅程',
    sessionRouteOpenInsights: '打开会话洞察',
    trailRouteTitle: '旅程洞察',
    trailRouteSubtitle:
      '把这条搜索旅程当成共享 route-first 实体来查看它的查询演化与成员证据。',
    trailRouteMembersTitle: '旅程成员',
    trailRouteOpenInsights: '打开旅程洞察',
    trailRouteOpenSession: '打开会话洞察',
    trailRouteVisitCount: '{count} 次访问',
    trailRouteDepthLabel: '深度',
    compareSetRouteTitle: '比较页面组',
    compareSetRouteSubtitle:
      '把这组并排比较页面当成共享实体来审阅，并保留可复用的旅程、日期与域名上下文。',
    compareSetRoutePagesTitle: '比较中的页面',
    compareSetRouteRecentDaysTitle: '最近比较日期',
    compareSetRouteOpenTrail: '打开旅程洞察',
    compareSetRouteOpenSession: '打开会话洞察',
    compareSetRouteDomainsLabel: '域名数',
    compareSetFocusTitle: '当前聚焦比较页面组',
    compareSetFocusBody:
      '这条旅程正在按 “{query}” 的比较页面组上下文查看，共涉及 {count} 个页面。',
    compareSetDayFocusBody:
      '这一天属于 “{query}” 的比较页面组上下文，共涉及 {count} 个页面。',
    compareSetDomainFocusBody:
      '这个域名属于 “{query}” 的比较页面组上下文，共涉及 {count} 个页面。',
    compareSetFocusBadge: '聚焦中',
    pathFlowFocusTitle: '当前聚焦常见路径',
    pathFlowFocusBody: '这个域名正在按重复路径查看：{flow}',
    dayInsightsHourlyTitle: '小时活动',
    dayInsightsTopSitesTitle: '当天重点网站',
    dayInsightsActivityMixTitle: '当天活动构成',
    dayInsightsQueryFamiliesTitle: '当天搜索演化',
    dayInsightsRefindsTitle: '当天重找页面',
  },
  'zh-TW': {
    statusReadyLabel: '智慧搜尋已就緒',
    statusReadyDescription:
      '已為 {count} 筆記錄建立索引，可以使用自然語言搜尋。',
    statusRebuildingLabel: '正在重建搜尋索引',
    statusRebuildingDescription:
      '正在重建智慧搜尋索引，期間搜尋會暫時使用關鍵字模式。',
    statusQueuedLabel: '索引工作排隊中',
    statusQueuedDescription:
      '索引工作已加入佇列，會在目前的 AI 工作完成後自動開始。',
    statusPausedLabel: 'AI 工作已暫停',
    statusPausedDescription:
      'AI 工作已暫停，需要你手動恢復或清空佇列才會繼續。',
    statusFailedLabel: '索引出現問題',
    statusFailedDescription:
      '最近一次索引工作失敗了。請檢查 AI 服務設定，然後重試。',
    statusStaleLabel: '索引需要刷新',
    statusStaleDescription:
      '語義索引落後於目前的匯入或增強資料。重新建立後才能涵蓋最新語義內容。',
    statusDegradedLabel: '智慧搜尋受限',
    statusDegradedDescription:
      '目前只能使用關鍵字搜尋，AI 服務暫時無法正常運作。',
    statusBlockedLabel: '需要先完成初始設定',
    statusBlockedDescription:
      '請先完成初始設定並解鎖資料庫，然後才能使用智慧功能。',
    statusDisabledLabel: 'AI 功能已關閉',
    statusDisabledDescription:
      '備份和搜尋等核心功能正常可用，智慧搜尋和 AI 助手目前已關閉。',
    statusEmptyLabel: '索引為空',
    statusEmptyDescription:
      '還沒有建立搜尋索引。請先建立索引，才能使用智慧搜尋。',
    noScore: '無評分',
    highConfidence: '高度吻合',
    relevant: '相關',
    weakMatch: '弱吻合',
    answerReady: '回答已完成',
    queued: '排隊中',
    evidenceMissing: '找不到相關記錄',
    assistantFailed: '回答失敗',
    cancelled: '已取消',
    inProgress: '處理中',
    timeRangeLabel: '時間範圍',
    rangeDay: '日',
    rangeWeek: '週',
    rangeMonth: '月',
    rangeQuarter: '季',
    rangeYear: '年',
    rangeAll: '全部時間',
    rangeCustom: '自訂',
    customStart: '開始日期',
    customEnd: '結束日期',
    applyRange: '套用',
    digestTitle: '時段概覽',
    digestUnavailable: '概覽資料暫不可用，完成一次備份後即可查看。',
    digestVisits: '頁面造訪',
    digestSearches: '搜尋次數',
    digestNewSites: '新網站',
    digestDeepRead: '深度閱讀',
    digestRefind: '重找頁面',
    trendLabel: '{direction} {percent}%',
    visits: '次',
    onThisDayTitle: '歷史上的今天',
    onThisDayEyebrow: '歷史上的今天',
    onThisDayEmpty: '過去的年份中沒有找到今天的瀏覽紀錄。',
    onThisDayVisits: '瀏覽了 {count} 個頁面',
    onThisDayMore: '查看更多年份 →',
    onThisDayCollapse: '收起年份',
    onThisDayDeepDive: '{count} 次深度研究會話',
    topSitesTitle: '最常造訪',
    topSitesEmpty: '瀏覽資料不足，暫無法顯示最常造訪網站。',
    insightAccessEyebrow: '直達入口',
    insightAccessTitle: '打開完整洞察',
    insightAccessDayLabel: '按日期查看',
    insightAccessDomainLabel: '按網域查看',
    openDayInsights: '打開當日洞察',
    openDomainInsights: '打開網域洞察',
    dayInsightsTitle: '當日洞察',
    dayInsightsSubtitle: '查看這一天的完整瀏覽熱度、重點網站與研究訊號。',
    dayInsightsInvalidDate: '無法辨識這個日期。',
    dayInsightsEmpty: '這一天暫時沒有可顯示的洞察。',
    dayInsightsBack: '返回概覽',
    dayInsightsOpenExplorer: '打開當天證據',
    entityBackToOverview: '返回概覽',
    entityOpenExplorer: '在 Explorer 中打開證據',
    queryFamilyRouteTitle: '查詢族',
    queryFamilyRouteSubtitle:
      '把這組相關搜尋當成可重用的 route-first 實體來審閱，而不是只留在卡片裡。',
    queryFamilyRelatedTrails: '相關旅程',
    queryFamilyQueriesTitle: '這一族裡的查詢',
    queryFamilyTrailsTitle: '相關搜尋旅程',
    refindRouteTitle: '重找頁面',
    refindRouteSubtitle:
      '查看這頁為何反覆出現、最近在哪些日期回來，以及哪些搜尋旅程把它帶進來。',
    refindFactorsTitle: '重找因素',
    refindRecentDaysTitle: '最近出現的日期',
    sessionRouteTitle: '會話洞察',
    sessionRouteSubtitle:
      '把這段瀏覽會話當成共享實體來查看，而不是只能在 Explorer 裡臨時展開。',
    sessionRouteVisitsTitle: '會話內頁面',
    sessionRouteTrailsTitle: '會話中的搜尋旅程',
    sessionRouteOpenInsights: '打開會話洞察',
    trailRouteTitle: '旅程洞察',
    trailRouteSubtitle:
      '把這條搜尋旅程當成共享 route-first 實體來查看它的查詢演化與成員證據。',
    trailRouteMembersTitle: '旅程成員',
    trailRouteOpenInsights: '打開旅程洞察',
    trailRouteOpenSession: '打開會話洞察',
    trailRouteVisitCount: '{count} 次造訪',
    trailRouteDepthLabel: '深度',
    compareSetRouteTitle: '比較頁面組',
    compareSetRouteSubtitle:
      '把這組並排比較頁面當成共享實體來審閱，並保留可重用的旅程、日期與網域上下文。',
    compareSetRoutePagesTitle: '比較中的頁面',
    compareSetRouteRecentDaysTitle: '最近比較日期',
    compareSetRouteOpenTrail: '打開旅程洞察',
    compareSetRouteOpenSession: '打開會話洞察',
    compareSetRouteDomainsLabel: '網域數',
    compareSetFocusTitle: '目前聚焦比較頁面組',
    compareSetFocusBody:
      '這條旅程正在按「{query}」的比較頁面組上下文查看，共涉及 {count} 個頁面。',
    compareSetDayFocusBody:
      '這一天屬於「{query}」的比較頁面組上下文，共涉及 {count} 個頁面。',
    compareSetDomainFocusBody:
      '這個網域屬於「{query}」的比較頁面組上下文，共涉及 {count} 個頁面。',
    compareSetFocusBadge: '聚焦中',
    pathFlowFocusTitle: '目前聚焦常見路徑',
    pathFlowFocusBody: '這個網域正在按重複路徑查看：{flow}',
    dayInsightsHourlyTitle: '小時活動',
    dayInsightsTopSitesTitle: '當天重點網站',
    dayInsightsActivityMixTitle: '當天活動構成',
    dayInsightsQueryFamiliesTitle: '當天搜尋演化',
    dayInsightsRefindsTitle: '當天重找頁面',
  },
} as const
