/**
 * @file intelligence-secondary-meta.ts
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
export const intelligenceSecondaryMetaNamespace = {
  en: {
    breadthTitle: 'Breadth Index',
    breadthEmpty: 'Not enough data to compute your breadth score.',
    breadthScoreLabel: 'Breadth score',
    breadthScoreHelp:
      'Breadth score only looks at how recent visits were distributed across sites. Higher scores mean the overall spread was wider, not necessarily that topics were broader.',
    breadthConcentrationLabel: 'Top half of browsing',
    breadthHhiKey: 'HHI',
    breadthAxisFocused: 'More focused',
    breadthAxisBroad: 'More spread',
    breadthVerdictBroad: 'Your browsing spans a wide set of sites and topics.',
    breadthVerdictBalanced: 'Your browsing balances focus with exploration.',
    breadthVerdictFocused:
      'Your browsing concentrates on a small set of core sites.',
    breadthConcentrationDetail:
      'Half of your browsing lives in {count} domains. That describes top-heavy concentration, which is a different lens than the breadth score.',
    breadthHhiLabel: 'HHI: {value}',
    breadthHhiHelp:
      'Lower HHI means more spread; higher HHI means more concentrated.',
    pathFlowsTitle: 'Common Paths',
    pathFlowsEmpty: 'Not enough data to surface repeating paths yet.',
    pathFlowsOccurrences: '{count} occurrences',
    pathFlowsStepLabel: 'Steps',
    pathFlowsStep2: '2 hops',
    pathFlowsStep3: '3 hops',
    pathFlowsStep4: '4 hops',
    habitsTitle: 'Habits',
    habitsEmpty: 'No recurring visit habits detected yet.',
    habitsInterruptedTitle: 'Interrupted Habits',
    habitsPatternsTitle: 'Detected Patterns',
    habitType_daily_habit: 'Daily',
    habitType_weekly_habit: 'Weekly',
    habitType_periodic_reference: 'Periodic',
    habitCadence: 'About every {interval} days',
    habitActiveDays: 'Seen on {count} days',
    habitVisits: '{count} visits',
    habitInterruptedBadge: 'Interrupted',
    habitInterruptedDetail:
      '{days} days since last visit (expected {expected})',
    habitPatternSummary:
      'About every {interval} days, seen on {days} different days',
    habitInterruptedSummary:
      '{days} days since the last visit (it used to return about every {expected} days)',
    habitLastSeen: 'Last seen: {date}',
    compareSetsTitle: 'Compare Sets',
    compareSetsEmpty: 'No side-by-side comparison patterns detected.',
    compareSetsPages: '{count} pages',
    compareSetsLanding: 'Landed',
    multiBrowserTitle: 'Multi-Browser Comparison',
    multiBrowserEmpty:
      'Need at least two backed-up browser profiles to compare.',
    multiBrowserVisits: '{count} visits',
    multiBrowserDomains: '{count} domains',
    multiBrowserShared: 'Shared Domains ({count})',
    multiBrowserExclusive: 'Exclusive to each profile',
    multiBrowserCategories: 'Category distribution',
    observedTitle: 'Observed Interactions',
    observedCapabilityBadge: 'Capability-gated',
    observedDisclaimer:
      'These values come directly from fields that your browser history files reported. Not every browser emits them, and missing fields are left blank rather than inferred.',
    observedEmpty:
      'This profile has no supported interaction evidence available.',
    observedForeground: 'Foreground {duration}',
    observedScroll: 'Scrolled {duration}',
    observedKeyPresses: '{count} key presses',
    observedLoadFailed: 'Load failed',
    scopedViewTitle: 'Profile-scoped view',
    scopedViewBody:
      'Core Intelligence is only reading {profile} right now. Clear the shared profile scope to return to the whole archive.',
    archiveWideBadge: 'Archive-wide metrics',
    archiveWideBody:
      'You are looking at archive-wide Core Intelligence results. Switch to one browser profile when you want the analysis to narrow with it.',
    externalOutputsReviewTitle: 'Manual output review moved to Settings',
    externalOutputsReviewBody:
      'If you need export-ready summaries or local host bundles, review them in Settings. This page stays focused on live Core Intelligence results.',
    externalOutputsReviewAction: 'Review in Settings',
    sectionMetaTitle: 'Evidence & freshness',
    sectionMetaStateDegraded: 'Degraded',
    sectionMetaGeneratedAt: 'Generated at',
    sectionMetaScope: 'Scope',
    sectionMetaWindow: 'Window',
    sectionMetaModules: 'Owning modules',
    sectionMetaSourceTables: 'Source tables',
    sectionMetaEnrichment: 'Includes enrichment',
    sectionMetaEnrichmentEnabled: 'Yes',
    sectionMetaEnrichmentDisabled: 'No',
    sectionMetaStateReason: 'State reason',
    sectionMetaNotes: 'Review notes',
    sectionMetaDirectRead: 'Direct read',
    sectionMetaOpenPanelAria: 'Open evidence and freshness details',
    sectionMetaClosePanelAria: 'Close evidence and freshness details',
    sectionMetaWindowDateRange: '{start} → {end}',
    sectionMetaWindowCalendarDayHistory:
      'Same calendar day across previous years ({date})',
    sectionMetaMetadataFallback:
      'This section metadata is incomplete, so PathKeep is showing a degraded review state instead of crashing the page.',
    secondarySectionErrorTitle: 'This insight could not load',
    secondarySectionErrorBody:
      'PathKeep could not load this section. Your data is safe — this is only a read.',
    secondarySectionRetry: 'Retry',
    runtimeDigestTitle: 'Runtime Digest',
    runtimeDigestNeedsArchiveTitle: 'Archive setup required',
    runtimeDigestNeedsArchiveBody:
      'Finish setup and unlock the archive before PathKeep can review Core Intelligence work.',
    runtimeDigestUnavailableTitle: 'Runtime review unavailable',
    runtimeDigestUnavailableBody:
      'PathKeep could not load the latest Core Intelligence queue summary right now.',
    runtimeDigestFailedTitle: '{count} jobs need review',
    runtimeDigestFailedBody:
      'Some rebuild or enrichment work still needs attention. Use Jobs for retry, cancel, and recovery details.',
    runtimeDigestRunningTitle: '{count} jobs running',
    runtimeDigestRunningBody:
      '{queued} more jobs are still queued behind the active work.',
    runtimeDigestQueuedTitle: '{count} jobs queued',
    runtimeDigestQueuedBody:
      'Core Intelligence refresh work is queued and will continue in the background.',
    runtimeDigestReadyTitle: 'Runtime looks healthy',
    runtimeDigestReadyBody:
      'No queued or running Core Intelligence work needs review right now.',
    runtimeDigestLastActivity: 'Last activity {relative}',
    runtimeDigestIdleMeta: 'No recent queue activity',
    externalOutputsDeferredTitle: 'Saved snippets and widgets are deferred',
    externalOutputsDeferredBody:
      'PathKeep can prepare internal payloads for future embed cards, widgets, and public snapshots, but no external host integrations ship in this release yet.',
  },
  'zh-CN': {
    breadthTitle: '集中度 / 广度指数',
    breadthEmpty: '数据不足，暂无法计算集中度。',
    breadthScoreLabel: '广度分',
    breadthScoreHelp:
      '广度分只看最近窗口内各网站访问份额的分布。分数越高，代表整体越分散，不代表主题一定更多。',
    breadthConcentrationLabel: '前半数浏览集中',
    breadthHhiKey: 'HHI',
    breadthAxisFocused: '更集中',
    breadthAxisBroad: '更分散',
    breadthVerdictBroad: '你的浏览分布很广，接触了多样的网站。',
    breadthVerdictBalanced: '你的浏览在集中与分散之间保持平衡。',
    breadthVerdictFocused: '你的浏览集中在少数几个核心网站上。',
    breadthConcentrationDetail:
      '前半数浏览集中在 {count} 个网站。这个数字看头部集中度，和广度分看的不是同一件事。',
    breadthHhiLabel: 'HHI: {value}',
    breadthHhiHelp: 'HHI 越低越分散，越高越集中。',
    pathFlowsTitle: '常见浏览路线',
    pathFlowsEmpty: '还没有足够数据识别固定路线。',
    pathFlowsOccurrences: '出现 {count} 次',
    pathFlowsStepLabel: '步数',
    pathFlowsStep2: '2 步',
    pathFlowsStep3: '3 步',
    pathFlowsStep4: '4 步',
    habitsTitle: '习惯模式',
    habitsEmpty: '还没有检测到规律性的访问习惯。',
    habitsInterruptedTitle: '中断的习惯',
    habitsPatternsTitle: '识别到的规律',
    habitType_daily_habit: '每日',
    habitType_weekly_habit: '每周',
    habitType_periodic_reference: '周期参考',
    habitCadence: '平均每 {interval} 天访问一次',
    habitActiveDays: '出现在 {count} 天',
    habitVisits: '{count} 次访问',
    habitInterruptedBadge: '已中断',
    habitInterruptedDetail: '{days} 天未访问 (预期 {expected} 天)',
    habitPatternSummary: '大约每 {interval} 天来一次，出现在 {days} 天',
    habitInterruptedSummary:
      '{days} 天没来了（原本大约每 {expected} 天来一次）',
    habitLastSeen: '最近一次：{date}',
    compareSetsTitle: '比较页面组',
    compareSetsEmpty: '还没有检测到并排比较的搜索行为。',
    compareSetsPages: '{count} 个页面',
    compareSetsLanding: '落地',
    multiBrowserTitle: '多浏览器对比',
    multiBrowserEmpty: '需要至少两个已备份的浏览器档案才能对比。',
    multiBrowserVisits: '{count} 次访问',
    multiBrowserDomains: '{count} 个网站',
    multiBrowserShared: '共享网站 ({count})',
    multiBrowserExclusive: '各档案独有',
    multiBrowserCategories: '类别分布对比',
    observedTitle: '浏览器直接报告的互动',
    observedCapabilityBadge: '能力受限',
    observedDisclaimer:
      '这些数据直接来自浏览器历史记录里报告的字段。不是所有浏览器都提供；没有报告的会直接留空。',
    observedEmpty: '这个档案没有可用的互动数据。',
    observedForeground: '前台 {duration}',
    observedScroll: '滚动 {duration}',
    observedKeyPresses: '{count} 次按键',
    observedLoadFailed: '加载失败',
    scopedViewTitle: '当前为浏览器范围视图',
    scopedViewBody:
      '当前只显示 {profile} 的 Core Intelligence 结果。清除共享浏览器筛选后，就会回到整份存档。',
    archiveWideBadge: '全部存档统计',
    archiveWideBody:
      '当前显示的是整份存档的 Core Intelligence 结果。如果切到单一浏览器，分析内容会跟着缩小范围。',
    externalOutputsReviewTitle: '手动输出审查已移到设置',
    externalOutputsReviewBody:
      '如果你要查看或建立可供外部使用的摘要，请到设置页面。这里会继续只显示 Core Intelligence 本身的分析结果。',
    externalOutputsReviewAction: '去设置查看',
    sectionMetaTitle: '证据与新鲜度',
    sectionMetaStateDegraded: '已降级',
    sectionMetaGeneratedAt: '生成时间',
    sectionMetaScope: '范围',
    sectionMetaWindow: '窗口',
    sectionMetaModules: '所属模块',
    sectionMetaSourceTables: '来源表',
    sectionMetaEnrichment: '包含 enrichment',
    sectionMetaEnrichmentEnabled: '是',
    sectionMetaEnrichmentDisabled: '否',
    sectionMetaStateReason: '状态原因',
    sectionMetaNotes: '审查备注',
    sectionMetaDirectRead: '直接读取',
    sectionMetaOpenPanelAria: '打开证据与新鲜度详情',
    sectionMetaClosePanelAria: '关闭证据与新鲜度详情',
    sectionMetaWindowDateRange: '{start} → {end}',
    sectionMetaWindowCalendarDayHistory: '过去几年同一日历日（{date}）',
    sectionMetaMetadataFallback:
      '这部分元数据不完整，PathKeep 已改为降级显示，而不是让页面崩溃。',
    secondarySectionErrorTitle: '这个洞察暂时加载失败',
    secondarySectionErrorBody:
      'PathKeep 无法加载这个板块。你的数据没有问题——这里只是读取。',
    secondarySectionRetry: '重试',
    runtimeDigestTitle: '运行摘要',
    runtimeDigestNeedsArchiveTitle: '需要先完成存档设置',
    runtimeDigestNeedsArchiveBody:
      '先完成设置并解锁存档，PathKeep 才能检查 Core Intelligence 的后台工作。',
    runtimeDigestUnavailableTitle: '运行检查暂不可用',
    runtimeDigestUnavailableBody:
      'PathKeep 目前无法加载最新的 Core Intelligence 队列摘要。',
    runtimeDigestFailedTitle: '{count} 个任务需要处理',
    runtimeDigestFailedBody:
      '仍有重建或增强任务需要处理。重试、取消和恢复细节请到 Jobs 页面查看。',
    runtimeDigestRunningTitle: '{count} 个任务正在运行',
    runtimeDigestRunningBody: '当前活跃工作后面还有 {queued} 个任务正在排队。',
    runtimeDigestQueuedTitle: '{count} 个任务正在排队',
    runtimeDigestQueuedBody:
      'Core Intelligence 刷新任务已经入队，会继续在后台推进。',
    runtimeDigestReadyTitle: '运行状态正常',
    runtimeDigestReadyBody:
      '当前没有需要处理的 Core Intelligence 排队或运行中任务。',
    runtimeDigestLastActivity: '最近活动 {relative}',
    runtimeDigestIdleMeta: '最近没有队列活动',
    externalOutputsDeferredTitle: '保存片段和小组件仍在后续版本',
    externalOutputsDeferredBody:
      'PathKeep 目前可以为未来的嵌入卡片、小组件和公开快照准备内部 payload，但这一版还没有交付任何外部宿主集成。',
  },
  'zh-TW': {
    breadthTitle: '集中度 / 廣度指數',
    breadthEmpty: '資料不足，暫無法計算集中度。',
    breadthScoreLabel: '廣度分',
    breadthScoreHelp:
      '廣度分只看最近視窗內各網站造訪份額的分布。分數越高，代表整體越分散，不代表主題一定更多。',
    breadthConcentrationLabel: '前半數瀏覽集中',
    breadthHhiKey: 'HHI',
    breadthAxisFocused: '更集中',
    breadthAxisBroad: '更分散',
    breadthVerdictBroad: '你的瀏覽分布很廣，接觸到多樣化的網站。',
    breadthVerdictBalanced: '你的瀏覽在集中與分散之間保持平衡。',
    breadthVerdictFocused: '你的瀏覽集中在少數幾個核心網站上。',
    breadthConcentrationDetail:
      '前半數瀏覽集中在 {count} 個網站。這個數字看頭部集中度，和廣度分看的不是同一件事。',
    breadthHhiLabel: 'HHI: {value}',
    breadthHhiHelp: 'HHI 越低越分散，越高越集中。',
    pathFlowsTitle: '常見瀏覽路線',
    pathFlowsEmpty: '還沒有足夠資料識別固定路線。',
    pathFlowsOccurrences: '出現 {count} 次',
    pathFlowsStepLabel: '步數',
    pathFlowsStep2: '2 步',
    pathFlowsStep3: '3 步',
    pathFlowsStep4: '4 步',
    habitsTitle: '習慣模式',
    habitsEmpty: '還沒有偵測到規律性的造訪習慣。',
    habitsInterruptedTitle: '中斷的習慣',
    habitsPatternsTitle: '識別到的規律',
    habitType_daily_habit: '每日',
    habitType_weekly_habit: '每週',
    habitType_periodic_reference: '週期參考',
    habitCadence: '平均每 {interval} 天造訪一次',
    habitActiveDays: '出現在 {count} 天',
    habitVisits: '{count} 次造訪',
    habitInterruptedBadge: '已中斷',
    habitInterruptedDetail: '{days} 天未造訪 (預期 {expected} 天)',
    habitPatternSummary: '大約每 {interval} 天回來一次，出現在 {days} 天',
    habitInterruptedSummary:
      '{days} 天沒來了（原本大約每 {expected} 天回來一次）',
    habitLastSeen: '最近一次：{date}',
    compareSetsTitle: '比較頁面組',
    compareSetsEmpty: '還沒有偵測到並排比較的搜尋行為。',
    compareSetsPages: '{count} 個頁面',
    compareSetsLanding: '落地',
    multiBrowserTitle: '多瀏覽器對比',
    multiBrowserEmpty: '至少需要兩個已備份的瀏覽器設定檔才能對比。',
    multiBrowserVisits: '{count} 次造訪',
    multiBrowserDomains: '{count} 個網站',
    multiBrowserShared: '共享網站 ({count})',
    multiBrowserExclusive: '各設定檔獨有',
    multiBrowserCategories: '類別分佈對比',
    observedTitle: '瀏覽器直接回報的互動',
    observedCapabilityBadge: '能力受限',
    observedDisclaimer:
      '這些資料直接來自瀏覽器歷史記錄中回報的欄位。不是所有瀏覽器都提供；沒有回報的欄位會直接留空。',
    observedEmpty: '這個設定檔沒有可用的互動資料。',
    observedForeground: '前景 {duration}',
    observedScroll: '滾動 {duration}',
    observedKeyPresses: '{count} 次按鍵',
    observedLoadFailed: '載入失敗',
    scopedViewTitle: '目前為瀏覽器範圍視圖',
    scopedViewBody:
      '目前只顯示 {profile} 的 Core Intelligence 結果。清除共享瀏覽器篩選後，就會回到整份封存。',
    archiveWideBadge: '全部封存統計',
    archiveWideBody:
      '目前顯示的是整份封存的 Core Intelligence 結果。如果切到單一瀏覽器，分析內容也會跟著縮小範圍。',
    externalOutputsReviewTitle: '手動輸出審查已移到設定',
    externalOutputsReviewBody:
      '如果你要查看或建立可供外部使用的摘要，請到設定頁面。這裡會繼續只顯示 Core Intelligence 本身的分析結果。',
    externalOutputsReviewAction: '去設定查看',
    sectionMetaTitle: '證據與新鮮度',
    sectionMetaStateDegraded: '已降級',
    sectionMetaGeneratedAt: '產生時間',
    sectionMetaScope: '範圍',
    sectionMetaWindow: '視窗',
    sectionMetaModules: '所屬模組',
    sectionMetaSourceTables: '來源表',
    sectionMetaEnrichment: '包含 enrichment',
    sectionMetaEnrichmentEnabled: '是',
    sectionMetaEnrichmentDisabled: '否',
    sectionMetaStateReason: '狀態原因',
    sectionMetaNotes: '審查備註',
    sectionMetaDirectRead: '直接讀取',
    sectionMetaOpenPanelAria: '打開證據與新鮮度詳情',
    sectionMetaClosePanelAria: '關閉證據與新鮮度詳情',
    sectionMetaWindowDateRange: '{start} → {end}',
    sectionMetaWindowCalendarDayHistory: '過去幾年同一個日曆日（{date}）',
    sectionMetaMetadataFallback:
      '這部分中繼資料不完整，PathKeep 已改為降級顯示，而不是讓頁面直接崩潰。',
    secondarySectionErrorTitle: '這個洞察暫時載入失敗',
    secondarySectionErrorBody:
      'PathKeep 無法載入這個區塊。你的資料沒有問題——這裡只是讀取。',
    secondarySectionRetry: '重試',
    runtimeDigestTitle: '執行摘要',
    runtimeDigestNeedsArchiveTitle: '需要先完成封存設定',
    runtimeDigestNeedsArchiveBody:
      '先完成設定並解鎖封存，PathKeep 才能檢查 Core Intelligence 的背景工作。',
    runtimeDigestUnavailableTitle: '執行檢查暫時無法使用',
    runtimeDigestUnavailableBody:
      'PathKeep 目前無法載入最新的 Core Intelligence 佇列摘要。',
    runtimeDigestFailedTitle: '{count} 個工作需要處理',
    runtimeDigestFailedBody:
      '仍有重建或增強工作需要處理。重試、取消和恢復細節請到 Jobs 頁面查看。',
    runtimeDigestRunningTitle: '{count} 個工作正在執行',
    runtimeDigestRunningBody:
      '目前活躍工作的後面還有 {queued} 個工作正在排隊。',
    runtimeDigestQueuedTitle: '{count} 個工作正在排隊',
    runtimeDigestQueuedBody:
      'Core Intelligence 重新整理工作已經入列，會繼續在背景推進。',
    runtimeDigestReadyTitle: '執行狀態正常',
    runtimeDigestReadyBody:
      '目前沒有需要處理的 Core Intelligence 排隊或執行中工作。',
    runtimeDigestLastActivity: '最近活動 {relative}',
    runtimeDigestIdleMeta: '最近沒有佇列活動',
    externalOutputsDeferredTitle: '儲存片段與小工具仍在後續版本',
    externalOutputsDeferredBody:
      'PathKeep 目前可以為未來的嵌入卡片、小工具與公開快照準備內部 payload，但這一版還沒有交付任何外部宿主整合。',
  },
} as const
