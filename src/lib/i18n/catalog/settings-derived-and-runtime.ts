/**
 * @file settings-derived-and-runtime.ts
 * @description Defines one focused settings translation owner so the overall settings namespace no longer lives in a single mega-file.
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Provide one bounded subsection of the settings namespace for en, zh-CN, and zh-TW.
 * - Keep related settings copy together without reintroducing a second language-specific owner.
 *
 * ## Not responsible for
 * - Other settings copy that belongs to different subgroup owners
 * - Translator creation, flattening, or language resolution
 *
 * ## Dependencies
 * - No runtime dependencies; `catalog-runtime.ts` imports this static dictionary during catalog assembly.
 *
 * ## Performance notes
 * - Static dictionary data only; keep this file side-effect free so large locale loads stay cheap.
 */

/**
 * Keeps the settings namespace subsection aligned across shipping locales so copy updates stay in one owner.
 */
export const settingsDerivedAndRuntimeNamespace = {
  en: {
    enrichmentDerivedState: 'CONTENT ENRICHMENT',
    derivedOnly: 'SAFE TO CLEAR',
    derivedStateBoundaryTitle: 'What gets affected',
    derivedStateBoundaryBody:
      'These actions only affect enrichment and insights data. Your original history, audit logs, and undo history stay untouched.',
    firstPartyRuntimeTitle: 'Built-in plugins only',
    firstPartyRuntimeBody:
      'PathKeep only runs its own enrichment plugins here. Third-party runtime access stays off until we support it safely.',
    searchRulesTitle: 'Search rules',
    searchRulesBody:
      'Review built-in search detection rules and add custom site-search mappings before rebuilding derived search activity.',
    searchRulesAdd: 'Add custom rule',
    searchRulesBuiltin: 'Built-in rules',
    searchRulesReadOnly: 'Read-only',
    searchRulesBuiltinBody:
      'These shipped rules stay on by default and define the baseline search-engine recognition contract.',
    searchRulesCustom: 'Custom rules',
    searchRulesCustomCount: '{count} custom rules',
    searchRulesCustomBody:
      'Custom rules let you recognize site search pages without reopening the shipped route grammar.',
    searchRulesCustomEmpty: 'No custom search rules yet.',
    searchRulesEdit: 'Edit rule',
    searchRulesDelete: 'Delete rule',
    searchRulesEditorTitle: 'Rule editor',
    searchRulesEditing: 'Editing',
    searchRulesNew: 'New',
    searchRulesDisplayName: 'Display name',
    searchRulesEngineId: 'Engine ID',
    searchRulesHostPattern: 'Host pattern',
    searchRulesPathPrefix: 'Path prefix',
    searchRulesQueryParam: 'Query param',
    searchRulesEnabled: 'Enabled',
    searchRulesExampleUrl: 'Example URL',
    searchRulesNote: 'Notes',
    searchRulesSave: 'Save rule',
    searchRulesSaving: 'Saving search rule…',
    searchRulesDeleting: 'Deleting search rule…',
    runtimeQueueTitle: 'Runtime queue',
    runtimeQueueBody:
      'A full rebuild refreshes cards and derived evidence. Enrichment jobs fill optional data and can fail, be retried, or be cancelled without changing your original history.',
    runtimeQueueSummary:
      '{queued} queued / {running} running / {failed} failed',
    rebuildDerivedState: 'Rebuild',
    clearDerivedState: 'Clear all',
    savingDeterministicModules: 'Saving module settings…',
    titleNormalizationPlugin: 'Title normalization',
    titleNormalizationDescription:
      'Cleans up page titles locally so duplicate tabs, redirects, and noisy suffixes collapse into clearer evidence labels.',
    readableContentRefetch: 'Readable content fetcher',
    readableContentRefetchBody:
      'Tracked for v0.3 and not available in v0.2.0. This future worker will revisit pages only after the feature is reliable enough to ship.',
    readableContentPlugin: 'Readable content fetcher',
    readableContentDescription:
      'This future worker will fetch readable webpage bodies. It is disabled in v0.2.0 so PathKeep does not pretend it can save page text today.',
    readableContentDeferredBadge: 'Coming in v0.3',
    readableContentDeferredTooltip:
      'Webpage body fetching ships in v0.3 once it can be rate-limited and audited safely.',
    visitDerivedFactsModule: 'Visit-derived facts',
    visitDerivedFactsModuleDescription:
      'Normalizes visit-level evidence, site dictionary fields, and search metadata before downstream rebuild stages run.',
    dailyRollupsModule: 'Daily rollups',
    dailyRollupsModuleDescription:
      'Composes day-level rollups for domains, categories, engines, and digest summaries.',
    sessionsModule: 'Sessions',
    sessionsModuleDescription:
      'Groups nearby visits into browsing sessions without guessing hidden dwell time.',
    searchTrailsModule: 'Search trails',
    searchTrailsModuleDescription:
      'Builds search trails, trail members, events, and query families from normalized visits.',
    refindPagesModule: 'Refind pages',
    refindPagesModuleDescription:
      'Tracks pages and sources that repeatedly help you return to the same work.',
    activityMixModule: 'Activity mix',
    activityMixModuleDescription:
      'Keeps digest metrics and period-over-period activity summaries in sync with daily rollups.',
    searchEffectivenessModule: 'Search effectiveness',
    searchEffectivenessModuleDescription:
      'Explains which search trails reopen, converge, or lead to useful follow-up results.',
    domainDeepDiveModule: 'Domain deep dive',
    domainDeepDiveModuleDescription:
      'Maintains domain rhythm, habit, and path-flow surfaces for deeper deterministic review.',
    deterministicModuleFallbackDescription:
      'Check the saved module trace before you rely on this result.',
    deterministicModuleVersion: 'Module version',
    deterministicModuleDependsOn: 'Depends on',
    deterministicModuleTables: 'Derived tables',
    deterministicModuleLastBuilt: 'Last built',
    deterministicModuleRebuiltTailNote:
      'Rebuilt structural tail entities for {profile}.',
    deterministicModuleNoVisibleVisitsClearedVisitFacts:
      'No visible visits remained for {profile}; cleared visit-derived facts.',
    deterministicModuleVisitFactsUpToDate:
      'Visit-derived facts for {profile} were already up to date.',
    deterministicModuleVisitFactsRefreshed:
      'Incrementally refreshed visit-derived facts for {profile}.',
    deterministicModuleVisitFactsRebuilt:
      'Rebuilt visit-derived facts for {profile} with a scoped full refresh.',
    deterministicModuleNoVisibleVisitsClearedDailyRollups:
      'No visible visits remained for {profile}; cleared daily rollups.',
    deterministicModuleDailyRollupsUpToDate:
      'Daily rollups for {profile} were already up to date.',
    deterministicModuleDailyRollupsRefreshed:
      'Refreshed dirty daily rollups for {profile}.',
    deterministicModuleDailyRollupsRebuilt:
      'Rebuilt all daily rollups for {profile}.',
    deterministicModuleDailyRollupsManualRebuild:
      'Manual full rebuild requested for daily rollups.',
    deterministicModuleDailyRollupsVisibilityRegressed:
      'Archive visibility regressed or source counters moved backwards for daily rollups.',
    deterministicModuleNoVisibleVisitsClearedStructural:
      'No visible visits remained for {profile}; cleared structural entities.',
    deterministicModuleStructuralUpToDate:
      'Structural entities for {profile} were already up to date.',
    deterministicModuleStructuralTailRebuilt:
      'Rebuilt structural tail entities for {profile}.',
    deterministicModuleStructuralRebuilt:
      'Rebuilt all structural entities for {profile}.',
    deterministicModuleStaleReason: 'Stale reason',
    deterministicModuleReady: 'Ready',
    deterministicModuleStale: 'Stale',
    deterministicModuleDisabled: 'Disabled',
    deterministicModuleIdle: 'Idle',
    enrichmentPluginFallbackDescription:
      'Check the plugin boundary before you turn it on for everyday use.',
    pluginBoundary: 'Boundary',
    pluginVersion: 'Version',
    pluginQueue: 'Queue',
    pluginQueueCounts: '{queued} queued / {running} running / {failed} failed',
    pluginFreshness: 'Refresh after',
    daysFreshness: '{days} days',
    pluginDerivedTables: 'Data tables',
    pluginStoredRecords: 'Stored rows',
    pluginStorageImpact: 'Disk usage',
    pluginLastCompleted: 'Last completed',
    pluginLastError: 'Last error',
    networkAccess: 'Network',
    localOnly: 'Local only',
    readableContentRefetchImpact: 'No webpage body text is fetched in v0.2.0.',
    disablePlugin: 'Disable',
    enablePlugin: 'Enable',
    runtimeRecentJobs: 'Recent runtime jobs',
    runtimeQueueDetailsTitle: 'Runtime job details live in Background Jobs',
    runtimeQueueDetailsBody:
      'Maintenance shows module and plugin settings plus rebuild controls. Retry, cancel, logs, and recent job walls stay on the Jobs page.',
    runtimeNoJobs:
      'Recent queue activity will appear here after a deterministic refresh.',
    deterministicRebuildJobLabel: 'Deterministic rebuild',
    runtimeJobAttempt: 'Attempt {attempt}',
    retryRuntimeJob: 'Retry',
    cancelRuntimeJob: 'Cancel',
    runtimeUnavailableTitle: 'Runtime status unavailable',
    runtimeUnavailableBody:
      'PathKeep could not load the enrichment runtime review surface right now.',
    runtimeStateQueued: 'Queued',
    runtimeStateRunning: 'Running',
    runtimeStateSucceeded: 'Succeeded',
    runtimeStateFailed: 'Failed',
    runtimeStateCancelled: 'Cancelled',
    latestGrowthSignal: 'Recent activity',
    openAuditRun: 'View in audit log',
    latestGrowthSignalBody:
      'Backup #{runId} added {visits} visits, {urls} URLs, and {downloads} downloads.',
    rebuildCompletedTitle: 'Rebuild complete',
    rebuildCompletedBody:
      'Processed {visits} visits, refreshed {enriched} enriched rows, and created {cards} insight cards.',
    rebuildQueuedTitle: 'Rebuild queued',
    rebuildQueuedBody:
      'Deterministic rebuild job #{jobId} is now in Background Jobs. Follow progress there while PathKeep refreshes cards and derived evidence.',
    clearCompletedTitle: 'Data cleared',
    clearCompletedBody:
      'Cleared {visitDerivedFacts} visit-derived rows, {dailyRollups} daily rollup rows, {structural} structural rows, and {runtime} runtime rows. Your original history was not affected.',
    savingEnrichmentSettings: 'Saving…',
    rebuildingDerivedState: 'Rebuilding…',
    clearingDerivedState: 'Clearing…',
    enabled: 'On',
    disabled: 'Off',
    historyFound: 'History found',
    noHistoryDetected: 'No history file found',
    platformTroubleshooting: 'TROUBLESHOOTING',
    platformDescription:
      'Check the health of your schedule, encryption, and import pipeline.',
    platformBody:
      'Quick links to check your schedule, encryption, and import status.',
    reviewSchedule: 'Scheduled Backup Settings',
    reviewSecurity: 'Security',
    reviewImports: 'Imports',
  },
  'zh-CN': {
    enrichmentDerivedState: '内容增强',
    derivedOnly: '可安全清除',
    derivedStateBoundaryTitle: '影响范围',
    derivedStateBoundaryBody:
      '只会影响分析和洞察资料。你的原始历史纪录、稽核日志和复原功能都不会受影响。',
    firstPartyRuntimeTitle: '仅限内建插件',
    firstPartyRuntimeBody:
      'PathKeep 这里只运行自己的增强插件。第三方运行时访问会先保持关闭，直到我们安全支持它。',
    searchRulesTitle: '搜索规则',
    searchRulesBody:
      '先审查内建搜索识别规则，再补充自定义站内搜索映射，然后再重建搜索活动派生数据。',
    searchRulesAdd: '新增自定义规则',
    searchRulesBuiltin: '内建规则',
    searchRulesReadOnly: '只读',
    searchRulesBuiltinBody:
      '这些随产品出货的规则默认开启，定义了搜索引擎识别的基线契约。',
    searchRulesCustom: '自定义规则',
    searchRulesCustomCount: '{count} 条自定义规则',
    searchRulesCustomBody:
      '自定义规则可让 PathKeep 识别站内搜索页面，而不用重新打开已接受的 route grammar。',
    searchRulesCustomEmpty: '还没有自定义搜索规则。',
    searchRulesEdit: '编辑规则',
    searchRulesDelete: '删除规则',
    searchRulesEditorTitle: '规则编辑器',
    searchRulesEditing: '编辑中',
    searchRulesNew: '新规则',
    searchRulesDisplayName: '显示名称',
    searchRulesEngineId: '引擎 ID',
    searchRulesHostPattern: 'Host 规则',
    searchRulesPathPrefix: '路径前缀',
    searchRulesQueryParam: '查询参数',
    searchRulesEnabled: '启用',
    searchRulesExampleUrl: '示例 URL',
    searchRulesNote: '备注',
    searchRulesSave: '保存规则',
    searchRulesSaving: '正在保存搜索规则…',
    searchRulesDeleting: '正在删除搜索规则…',
    runtimeQueueTitle: '运行队列',
    runtimeQueueBody:
      '完整重建会刷新卡片和派生证据。增强任务只补充可选资料，失败、重试或取消都不会改动原始历史纪录。',
    runtimeQueueSummary: '{queued} 个排队 / {running} 个运行 / {failed} 个失败',
    rebuildDerivedState: '重新生成',
    clearDerivedState: '清除所有',
    savingDeterministicModules: '正在保存模块设置…',
    titleNormalizationPlugin: '标题规范化',
    titleNormalizationDescription:
      '在本地整理网页标题，让重复标签页、跳转页和多余尾码变成更清楚的证据标签。',
    readableContentRefetch: '网页可读内容抓取',
    readableContentRefetchBody:
      '已排入 v0.3，v0.2.0 暂不开放。这个未来任务会在足够可靠后，再重新访问页面并提取正文。',
    readableContentPlugin: '网页可读内容抓取',
    readableContentDescription:
      '这个未来任务会抓取网页正文。v0.2.0 中它保持禁用，PathKeep 不会假装现在已经能保存网页正文。',
    readableContentDeferredBadge: 'v0.3 开放',
    readableContentDeferredTooltip:
      '网页正文抓取在 v0.3 提供，等到可以安全限速和审计后再开放。',
    visitDerivedFactsModule: '访问派生事实',
    visitDerivedFactsModuleDescription:
      '先把 visit 级别的证据、站点词典字段和搜索元数据标准化，再交给后续重建阶段使用。',
    dailyRollupsModule: '每日汇总',
    dailyRollupsModuleDescription:
      '生成 domain、category、engine 和 digest summary 的日级 rollup。',
    sessionsModule: '会话',
    sessionsModuleDescription:
      '在不猜测隐藏停留时间的前提下，把相邻访问整理成浏览会话。',
    searchTrailsModule: '搜索轨迹',
    searchTrailsModuleDescription:
      '根据标准化后的 visit 构建搜索轨迹、轨迹成员、搜索事件和查询家族。',
    refindPagesModule: '重访页面',
    refindPagesModuleDescription:
      '追踪那些反复帮助你回到同一项工作的页面与来源。',
    activityMixModule: '活动构成',
    activityMixModuleDescription:
      '让 digest 指标与按周期比较的活动摘要持续跟每日 rollup 对齐。',
    searchEffectivenessModule: '搜索效果',
    searchEffectivenessModuleDescription:
      '解释哪些搜索轨迹会重新打开问题、收敛，或带来有用的后续结果。',
    domainDeepDiveModule: '域名深挖',
    domainDeepDiveModuleDescription:
      '维护域名节奏、习惯和路径流等更深入的确定性复核界面。',
    deterministicModuleFallbackDescription:
      '在依赖这个结果之前，先检查保存下来的模块跟踪记录。',
    deterministicModuleVersion: '模块版本',
    deterministicModuleDependsOn: '依赖',
    deterministicModuleTables: '派生数据表',
    deterministicModuleLastBuilt: '上次构建',
    deterministicModuleRebuiltTailNote: '已为 {profile} 重建结构化尾部实体。',
    deterministicModuleNoVisibleVisitsClearedVisitFacts:
      '{profile} 没有剩余可见访问，已清除访问派生事实。',
    deterministicModuleVisitFactsUpToDate:
      '{profile} 的访问派生事实已经是最新。',
    deterministicModuleVisitFactsRefreshed:
      '已为 {profile} 增量刷新访问派生事实。',
    deterministicModuleVisitFactsRebuilt:
      '已为 {profile} 用限定范围的完整刷新重建访问派生事实。',
    deterministicModuleNoVisibleVisitsClearedDailyRollups:
      '{profile} 没有剩余可见访问，已清除每日汇总。',
    deterministicModuleDailyRollupsUpToDate: '{profile} 的每日汇总已经是最新。',
    deterministicModuleDailyRollupsRefreshed:
      '已为 {profile} 刷新变更过的每日汇总。',
    deterministicModuleDailyRollupsRebuilt: '已为 {profile} 重建全部每日汇总。',
    deterministicModuleDailyRollupsManualRebuild:
      '已请求手动完整重建每日汇总。',
    deterministicModuleDailyRollupsVisibilityRegressed:
      '存档可见性回退，或每日汇总的来源计数出现倒退。',
    deterministicModuleNoVisibleVisitsClearedStructural:
      '{profile} 没有剩余可见访问，已清除结构化实体。',
    deterministicModuleStructuralUpToDate: '{profile} 的结构化实体已经是最新。',
    deterministicModuleStructuralTailRebuilt:
      '已为 {profile} 重建结构化尾部实体。',
    deterministicModuleStructuralRebuilt: '已为 {profile} 重建全部结构化实体。',
    deterministicModuleStaleReason: '过期原因',
    deterministicModuleReady: '已就绪',
    deterministicModuleStale: '已过期',
    deterministicModuleDisabled: '已关闭',
    deterministicModuleIdle: '待构建',
    enrichmentPluginFallbackDescription:
      '启用前先确认这个插件的边界是否适合日常使用。',
    pluginBoundary: '边界',
    pluginVersion: '版本',
    pluginQueue: '队列',
    pluginQueueCounts: '{queued} 个排队 / {running} 个运行 / {failed} 个失败',
    pluginFreshness: '刷新周期',
    daysFreshness: '{days} 天',
    pluginDerivedTables: '数据表',
    pluginStoredRecords: '已存储行数',
    pluginStorageImpact: '磁盘占用',
    pluginLastCompleted: '上次完成',
    pluginLastError: '最近错误',
    networkAccess: '网络',
    localOnly: '仅本地',
    readableContentRefetchImpact: 'v0.2.0 不会抓取网页正文。',
    disablePlugin: '关闭',
    enablePlugin: '开启',
    runtimeRecentJobs: '最近运行任务',
    runtimeQueueDetailsTitle: '运行任务详情在后台任务页',
    runtimeQueueDetailsBody:
      '维护页只显示模块 / 插件设置和重建控制。重试、取消、日志和最近任务列表统一留在后台任务页。',
    runtimeNoJobs: '下一次确定性刷新后，这里会显示最近的队列活动。',
    deterministicRebuildJobLabel: '确定性重建',
    runtimeJobAttempt: '第 {attempt} 次尝试',
    retryRuntimeJob: '重试',
    cancelRuntimeJob: '取消',
    runtimeUnavailableTitle: '无法加载运行状态',
    runtimeUnavailableBody: 'PathKeep 目前无法加载增强运行时复核界面。',
    runtimeStateQueued: '排队中',
    runtimeStateRunning: '运行中',
    runtimeStateSucceeded: '已完成',
    runtimeStateFailed: '失败',
    runtimeStateCancelled: '已取消',
    latestGrowthSignal: '最近活动',
    openAuditRun: '查看日志',
    latestGrowthSignalBody:
      '备份 #{runId} 新增了 {visits} 次浏览、{urls} 个网址和 {downloads} 条下载。',
    rebuildCompletedTitle: '重新生成完成',
    rebuildCompletedBody:
      '处理了 {visits} 条浏览记录，刷新了 {enriched} 条增强数据，生成了 {cards} 张洞察卡片。',
    rebuildQueuedTitle: '重新生成已加入队列',
    rebuildQueuedBody:
      '确定性重新生成任务 #{jobId} 已进入后台任务。PathKeep 会在后台刷新卡片和派生证据，你可以到 Jobs 页面查看进度。',
    clearCompletedTitle: '数据已清除',
    clearCompletedBody:
      '清除了 {visitDerivedFacts} 条访问派生数据、{dailyRollups} 条每日汇总数据、{structural} 条结构化数据和 {runtime} 条运行时数据。原始历史记录未受影响。',
    savingEnrichmentSettings: '保存中…',
    rebuildingDerivedState: '重新生成中…',
    clearingDerivedState: '清除中…',
    enabled: '已开启',
    disabled: '已关闭',
    historyFound: '已找到历史记录',
    noHistoryDetected: '未找到历史文件',
    platformTroubleshooting: '问题排查',
    platformDescription: '检查定时备份、加密和导入管线的运行状态。',
    platformBody: '快速检查定时备份、加密和导入状态。',
    reviewSchedule: '定时备份设置',
    reviewSecurity: '安全',
    reviewImports: '导入',
  },
  'zh-TW': {
    enrichmentDerivedState: '內容增強',
    derivedOnly: '可安全清除',
    derivedStateBoundaryTitle: '影響範圍',
    derivedStateBoundaryBody:
      '只會影響分析和洞察資料。你的原始歷史紀錄、稽核日誌和復原功能都不會受影響。',
    firstPartyRuntimeTitle: '僅限內建插件',
    firstPartyRuntimeBody:
      'PathKeep 這裡只執行自己的增強插件。第三方 runtime 存取會先保持關閉，直到我們安全支援它。',
    searchRulesTitle: '搜尋規則',
    searchRulesBody:
      '先審查內建搜尋識別規則，再補上自訂站內搜尋對應，之後再重建搜尋活動衍生資料。',
    searchRulesAdd: '新增自訂規則',
    searchRulesBuiltin: '內建規則',
    searchRulesReadOnly: '唯讀',
    searchRulesBuiltinBody:
      '這些隨產品出貨的規則預設開啟，定義了搜尋引擎識別的基線契約。',
    searchRulesCustom: '自訂規則',
    searchRulesCustomCount: '{count} 條自訂規則',
    searchRulesCustomBody:
      '自訂規則可讓 PathKeep 辨識站內搜尋頁面，而不用重開已接受的 route grammar。',
    searchRulesCustomEmpty: '還沒有自訂搜尋規則。',
    searchRulesEdit: '編輯規則',
    searchRulesDelete: '刪除規則',
    searchRulesEditorTitle: '規則編輯器',
    searchRulesEditing: '編輯中',
    searchRulesNew: '新規則',
    searchRulesDisplayName: '顯示名稱',
    searchRulesEngineId: '引擎 ID',
    searchRulesHostPattern: 'Host 規則',
    searchRulesPathPrefix: '路徑前綴',
    searchRulesQueryParam: '查詢參數',
    searchRulesEnabled: '啟用',
    searchRulesExampleUrl: '示例 URL',
    searchRulesNote: '備註',
    searchRulesSave: '儲存規則',
    searchRulesSaving: '正在儲存搜尋規則…',
    searchRulesDeleting: '正在刪除搜尋規則…',
    runtimeQueueTitle: '執行佇列',
    runtimeQueueBody:
      '完整重建會刷新卡片和衍生證據。增強工作只補充可選資料，失敗、重試或取消都不會改動原始歷史紀錄。',
    runtimeQueueSummary:
      '{queued} 個排隊 / {running} 個執行中 / {failed} 個失敗',
    rebuildDerivedState: '重新產生',
    clearDerivedState: '清除全部',
    savingDeterministicModules: '正在儲存模組設定…',
    titleNormalizationPlugin: '標題正規化',
    titleNormalizationDescription:
      '在本機整理網頁標題，讓重複分頁、跳轉頁和多餘尾碼變成更清楚的證據標籤。',
    readableContentRefetch: '網頁可讀內容擷取',
    readableContentRefetchBody:
      '已排入 v0.3，v0.2.0 暫不開放。這個未來工作會在足夠可靠後，再重新造訪頁面並提取正文。',
    readableContentPlugin: '網頁可讀內容擷取',
    readableContentDescription:
      '這個未來工作會擷取網頁正文。v0.2.0 中它保持停用，PathKeep 不會假裝現在已經能保存網頁正文。',
    readableContentDeferredBadge: 'v0.3 開放',
    readableContentDeferredTooltip:
      '網頁正文擷取在 v0.3 提供，等到可以安全限速與稽核後再開放。',
    visitDerivedFactsModule: '造訪衍生事實',
    visitDerivedFactsModuleDescription:
      '先把 visit 層級的證據、站點詞典欄位與搜尋中繼資料標準化，再交給後續重建階段使用。',
    dailyRollupsModule: '每日彙總',
    dailyRollupsModuleDescription:
      '產生 domain、category、engine 與 digest summary 的日級 rollup。',
    sessionsModule: '工作階段',
    sessionsModuleDescription:
      '在不猜測隱藏停留時間的前提下，把相鄰造訪整理成瀏覽工作階段。',
    searchTrailsModule: '搜尋軌跡',
    searchTrailsModuleDescription:
      '根據標準化後的 visit 建立搜尋軌跡、軌跡成員、搜尋事件與查詢家族。',
    refindPagesModule: '重訪頁面',
    refindPagesModuleDescription:
      '追蹤那些反覆幫助你回到同一項工作的頁面與來源。',
    activityMixModule: '活動構成',
    activityMixModuleDescription:
      '讓 digest 指標與分期比較的活動摘要持續和每日 rollup 對齊。',
    searchEffectivenessModule: '搜尋效果',
    searchEffectivenessModuleDescription:
      '解釋哪些搜尋軌跡會重新打開問題、收斂，或帶來有用的後續結果。',
    domainDeepDiveModule: '網域深挖',
    domainDeepDiveModuleDescription:
      '維護網域節奏、習慣與路徑流等更深入的確定性複核介面。',
    deterministicModuleFallbackDescription:
      '在依賴這個結果之前，先檢查保存下來的模組追蹤記錄。',
    deterministicModuleVersion: '模組版本',
    deterministicModuleDependsOn: '依賴',
    deterministicModuleTables: '衍生資料表',
    deterministicModuleLastBuilt: '上次建置',
    deterministicModuleRebuiltTailNote: '已為 {profile} 重建結構化尾端實體。',
    deterministicModuleNoVisibleVisitsClearedVisitFacts:
      '{profile} 沒有剩餘可見造訪，已清除造訪衍生事實。',
    deterministicModuleVisitFactsUpToDate:
      '{profile} 的造訪衍生事實已經是最新。',
    deterministicModuleVisitFactsRefreshed:
      '已為 {profile} 增量刷新造訪衍生事實。',
    deterministicModuleVisitFactsRebuilt:
      '已為 {profile} 用限定範圍的完整刷新重建造訪衍生事實。',
    deterministicModuleNoVisibleVisitsClearedDailyRollups:
      '{profile} 沒有剩餘可見造訪，已清除每日彙總。',
    deterministicModuleDailyRollupsUpToDate: '{profile} 的每日彙總已經是最新。',
    deterministicModuleDailyRollupsRefreshed:
      '已為 {profile} 刷新變更過的每日彙總。',
    deterministicModuleDailyRollupsRebuilt: '已為 {profile} 重建全部每日彙總。',
    deterministicModuleDailyRollupsManualRebuild:
      '已要求手動完整重建每日彙總。',
    deterministicModuleDailyRollupsVisibilityRegressed:
      '封存可見性回退，或每日彙總的來源計數出現倒退。',
    deterministicModuleNoVisibleVisitsClearedStructural:
      '{profile} 沒有剩餘可見造訪，已清除結構化實體。',
    deterministicModuleStructuralUpToDate: '{profile} 的結構化實體已經是最新。',
    deterministicModuleStructuralTailRebuilt:
      '已為 {profile} 重建結構化尾端實體。',
    deterministicModuleStructuralRebuilt: '已為 {profile} 重建全部結構化實體。',
    deterministicModuleStaleReason: '過期原因',
    deterministicModuleReady: '已就緒',
    deterministicModuleStale: '已過期',
    deterministicModuleDisabled: '已關閉',
    deterministicModuleIdle: '待建置',
    enrichmentPluginFallbackDescription:
      '啟用前先確認這個插件的邊界是否適合日常使用。',
    pluginBoundary: '邊界',
    pluginVersion: '版本',
    pluginQueue: '佇列',
    pluginQueueCounts: '{queued} 個排隊 / {running} 個執行中 / {failed} 個失敗',
    pluginFreshness: '重新整理週期',
    daysFreshness: '{days} 天',
    pluginDerivedTables: '資料表',
    pluginStoredRecords: '已儲存列數',
    pluginStorageImpact: '磁碟用量',
    pluginLastCompleted: '上次完成',
    pluginLastError: '最近錯誤',
    networkAccess: '網路',
    localOnly: '僅限本機',
    readableContentRefetchImpact: 'v0.2.0 不會擷取網頁正文。',
    disablePlugin: '關閉',
    enablePlugin: '開啟',
    runtimeRecentJobs: '最近執行工作',
    runtimeQueueDetailsTitle: '執行工作詳情在背景工作頁',
    runtimeQueueDetailsBody:
      '維護頁只顯示模組 / 外掛設定和重建控制。重試、取消、日誌和最近工作列表統一留在背景工作頁。',
    runtimeNoJobs: '下一次確定性重新整理後，這裡會顯示最近的佇列活動。',
    deterministicRebuildJobLabel: '確定性重建',
    runtimeJobAttempt: '第 {attempt} 次嘗試',
    retryRuntimeJob: '重試',
    cancelRuntimeJob: '取消',
    runtimeUnavailableTitle: '無法載入執行狀態',
    runtimeUnavailableBody: 'PathKeep 目前無法載入增強執行狀態複核介面。',
    runtimeStateQueued: '排隊中',
    runtimeStateRunning: '執行中',
    runtimeStateSucceeded: '已完成',
    runtimeStateFailed: '失敗',
    runtimeStateCancelled: '已取消',
    latestGrowthSignal: '最近活動',
    openAuditRun: '查看日誌',
    latestGrowthSignalBody:
      '備份 #{runId} 新增了 {visits} 次瀏覽、{urls} 個網址和 {downloads} 筆下載。',
    rebuildCompletedTitle: '重新產生完成',
    rebuildCompletedBody:
      '處理了 {visits} 筆瀏覽紀錄，重新整理了 {enriched} 筆增強資料，產生了 {cards} 張洞察卡片。',
    rebuildQueuedTitle: '重新產生已加入佇列',
    rebuildQueuedBody:
      '確定性重新產生任務 #{jobId} 已進入背景工作。PathKeep 會在背景重新整理卡片和派生證據，你可以到 Jobs 頁面查看進度。',
    clearCompletedTitle: '資料已清除',
    clearCompletedBody:
      '清除了 {visitDerivedFacts} 筆造訪衍生資料、{dailyRollups} 筆每日彙總資料、{structural} 筆結構化資料和 {runtime} 筆執行階段資料。原始歷史紀錄未受影響。',
    savingEnrichmentSettings: '儲存中…',
    rebuildingDerivedState: '重新產生中…',
    clearingDerivedState: '清除中…',
    enabled: '已開啟',
    disabled: '已關閉',
    historyFound: '已找到歷史紀錄',
    noHistoryDetected: '未找到歷史檔案',
    platformTroubleshooting: '問題排查',
    platformDescription: '檢查定時備份、加密和匯入管線的運作狀態。',
    platformBody: '快速檢查定時備份、加密和匯入狀態。',
    reviewSchedule: '定時備份設定',
    reviewSecurity: '安全',
    reviewImports: '匯入',
  },
} as const
