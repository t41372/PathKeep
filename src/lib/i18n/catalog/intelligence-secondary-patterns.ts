/**
 * @file intelligence-secondary-patterns.ts
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
export const intelligenceSecondaryPatternsNamespace = {
  en: {
    stableSourcesTitle: 'Stable Answer Sources',
    stableSourcesEmpty: 'Not enough data to show stable sources.',
    stableSourcesEntry: 'Frequent Entry Sources',
    stableSourcesLanding: 'Frequent Landing Sources',
    stableSourcesHelp:
      'Only evidence inside explicit search trails counts here: entry shows where trails often began, landing shows where they often settled.',
    stableSourcesEntryHelp: 'Sites that often brought you into a search trail.',
    stableSourcesLandingHelp:
      'Sites that often became the final landing point of a search trail.',
    stableSourcesNoEntry: 'No strong entry sources surfaced for this window.',
    stableSourcesNoLanding:
      'No strong landing sources surfaced for this window.',
    stableSourcesEntryCount: '{count} search trails started here',
    stableSourcesLandingCount: '{count} stable landings',
    stableSourcesTrails: 'trails',
    stableSourcesLandings: 'landings',
    searchEffectivenessTitle: 'Search Effectiveness',
    searchEffectivenessEmpty: 'Not enough data to show search effectiveness.',
    searchEffectivenessReformulations: 'avg reformulations',
    searchEffectivenessTrails: '{count} search trails',
    searchEffectivenessRewrites: '{count} rewrites',
    searchEffectivenessDepth: 'Avg depth {count}',
    searchEffectivenessHelp:
      'Only explicit search trails count here. Fewer rewrites and lower depth usually mean you reached a useful page faster.',
    searchEffectivenessRewritesLabel: 'Avg rewrites',
    searchEffectivenessDepthLabel: 'Avg depth',
    searchEffectivenessTrailsLabel: 'Search trails',
    searchEffectivenessEngineRewrites:
      'Each trail was rewritten about {count} times on average.',
    searchEffectivenessEngineDepth:
      'People usually stopped around depth {count}.',
    searchEffectivenessEngineTrails:
      'This window produced {count} search trails.',
    searchEffectivenessSources: 'Sources that often resolve searches',
    searchEffectivenessSourcesHelp:
      'These sites more often became a page worth stopping on after search.',
    searchEffectivenessHardest: 'Searches that needed the most rewrites',
    searchEffectivenessHardestHelp:
      'These topics usually needed multiple rewrites or were reopened days later.',
    searchEffectivenessLag: 'Searched again after {days} days',
    frictionTitle: 'Friction & Dead Ends',
    frictionEmpty: 'No friction signals detected.',
    frictionStrong: 'Strong',
    frictionWeak: 'Weak',
    reopenedTitle: 'Reopened Investigations',
    reopenedEmpty: 'No reopened investigations detected.',
    reopenedAnchorQuery: 'Query',
    reopenedAnchorPage: 'Page',
    reopenedOccurrences: '{count} occurrences',
    reopenedDistinctDays: 'across {days} days',
    discoveryTrendTitle: 'Discovery Trend',
    discoveryTrendEmpty: 'Not enough data to show discovery trend.',
    discoveryTrendHelp:
      'Discovery rate = new sites divided by total visits. Use it to see whether recent browsing leaned toward exploration or familiar sites.',
    discoveryTrendNewDomains: 'new sites',
    discoveryTrendRateLabel: 'Discovery Rate %',
    discoveryTrendDomainsLabel: 'New Sites',
    discoveryTrendWeekLabel: '{year} Week {week}',
    discoveryTrendRatePercent: '{count}%',
    discoveryTrendVisitsLabel: '{count} visits',
    domainInsightsTitle: 'Domain Insights',
    domainInsightsSubtitle:
      'Review this domain as a full entity inside the current scope and time window.',
    domainInsightsOpenExplorer: 'Open domain evidence',
    domainDeepDiveEmpty: "Could not load this site's deep analysis.",
    domainDeepDiveBack: 'Back',
    domainDeepDiveVisits: 'Visits',
    domainDeepDiveActiveDays: 'Active Days',
    domainDeepDiveTrails: 'Trails',
    domainDeepDiveArrival: 'How You Arrived',
    domainDeepDiveArrival_search: 'Search',
    domainDeepDiveArrival_link: 'Link',
    domainDeepDiveArrival_typed: 'Typed',
    domainDeepDiveTopPages: 'Top Pages',
    domainDeepDiveReferrers: 'Top Referrers',
    domainDeepDiveExits: 'Top Exits',
    domainDeepDiveTrend: 'Visit Trend',
    domainSearchKeywordsTitle: 'What You Searched On This Site',
    domainSearchKeywordsHelp:
      'Only shows keyword searches that were actually performed on this site inside the current window.',
  },
  'zh-CN': {
    stableSourcesTitle: '稳定答案来源',
    stableSourcesEmpty: '数据不足，暂无法显示稳定来源。',
    stableSourcesEntry: '常作为入口的来源',
    stableSourcesLanding: '常作为落地点的来源',
    stableSourcesHelp:
      '只统计明确落在搜索路径里的证据：入口看你常从哪里进入，落地点看你最后常停在哪个站。',
    stableSourcesEntryHelp: '经常把你带进一条搜索路径的网站。',
    stableSourcesLandingHelp: '经常成为搜索最终落点的网站。',
    stableSourcesNoEntry: '这段时间没有足够的入口来源。',
    stableSourcesNoLanding: '这段时间没有足够的稳定落点。',
    stableSourcesEntryCount: '{count} 条搜索路径从这里进入',
    stableSourcesLandingCount: '{count} 次成为稳定落点',
    stableSourcesTrails: '条旅程',
    stableSourcesLandings: '次落地',
    searchEffectivenessTitle: '搜索效率分析',
    searchEffectivenessEmpty: '数据不足，暂无法显示搜索效率。',
    searchEffectivenessReformulations: '次平均改写',
    searchEffectivenessTrails: '{count} 条搜索路径',
    searchEffectivenessRewrites: '改写 {count} 次',
    searchEffectivenessDepth: '平均深度 {count}',
    searchEffectivenessHelp:
      '这里只统计明确的搜索路径。改写越少、平均深度越低，通常越快找到结果。',
    searchEffectivenessRewritesLabel: '平均改写',
    searchEffectivenessDepthLabel: '平均深度',
    searchEffectivenessTrailsLabel: '搜索路径',
    searchEffectivenessEngineRewrites: '平均每条搜索路径改写 {count} 次',
    searchEffectivenessEngineDepth: '通常点到第 {count} 层才停下来',
    searchEffectivenessEngineTrails: '这段时间共形成 {count} 条搜索路径',
    searchEffectivenessSources: '常带来结果的来源',
    searchEffectivenessSourcesHelp:
      '这些网站更常在搜索后成为可停留、可继续阅读的结果页。',
    searchEffectivenessHardest: '改写最多的搜索题目',
    searchEffectivenessHardestHelp:
      '这些题目通常需要多次换词，或隔几天又回来继续搜。',
    searchEffectivenessLag: '{days} 天后又回来搜',
    frictionTitle: '碰壁与高摩擦侦测',
    frictionEmpty: '没有检测到碰壁信号。',
    frictionStrong: '强证据',
    frictionWeak: '弱证据',
    reopenedTitle: '反复回来查的问题',
    reopenedEmpty: '没有检测到反复查找的问题。',
    reopenedAnchorQuery: '查询',
    reopenedAnchorPage: '页面',
    reopenedOccurrences: '出现 {count} 次',
    reopenedDistinctDays: '跨 {days} 天',
    discoveryTrendTitle: '探索率趋势',
    discoveryTrendEmpty: '数据不足，暂无法显示探索率。',
    discoveryTrendHelp:
      '探索率 = 新网站数 / 总浏览次数。用它看最近是更常去新站，还是更常回到熟悉网站。',
    discoveryTrendNewDomains: '个新网站',
    discoveryTrendRateLabel: '探索率 %',
    discoveryTrendDomainsLabel: '新网站数',
    discoveryTrendWeekLabel: '{year} 第{week}周',
    discoveryTrendRatePercent: '{count}%',
    discoveryTrendVisitsLabel: '{count} 次浏览',
    domainInsightsTitle: '域名洞察',
    domainInsightsSubtitle: '查看这个网站在当前范围内的完整行为与流向。',
    domainInsightsOpenExplorer: '打开域名证据',
    domainDeepDiveEmpty: '无法加载该网站的深度分析。',
    domainDeepDiveBack: '返回',
    domainDeepDiveVisits: '访问次数',
    domainDeepDiveActiveDays: '活跃天数',
    domainDeepDiveTrails: '搜索旅程',
    domainDeepDiveArrival: '到达方式',
    domainDeepDiveArrival_search: '搜索',
    domainDeepDiveArrival_link: '链接',
    domainDeepDiveArrival_typed: '直接输入',
    domainDeepDiveTopPages: '热门页面',
    domainDeepDiveReferrers: '主要来源',
    domainDeepDiveExits: '主要去向',
    domainDeepDiveTrend: '访问趋势',
    domainSearchKeywordsTitle: '在这个网站上搜索过什么',
    domainSearchKeywordsHelp:
      '只显示当前时间范围内在这个网站上实际搜索过的关键词。',
  },
  'zh-TW': {
    stableSourcesTitle: '穩定答案來源',
    stableSourcesEmpty: '資料不足，暫無法顯示穩定來源。',
    stableSourcesEntry: '常作為入口的來源',
    stableSourcesLanding: '常作為落地點的來源',
    stableSourcesHelp:
      '只統計明確落在搜尋路徑裡的證據：入口看你常從哪裡進入，落地點看你最後常停在哪個站。',
    stableSourcesEntryHelp: '經常把你帶進一條搜尋路徑的網站。',
    stableSourcesLandingHelp: '經常成為搜尋最終落地點的網站。',
    stableSourcesNoEntry: '這段時間沒有足夠的入口來源。',
    stableSourcesNoLanding: '這段時間沒有足夠的穩定落地點。',
    stableSourcesEntryCount: '{count} 條搜尋路徑從這裡進入',
    stableSourcesLandingCount: '{count} 次成為穩定落地點',
    stableSourcesTrails: '條旅程',
    stableSourcesLandings: '次落地',
    searchEffectivenessTitle: '搜尋效率分析',
    searchEffectivenessEmpty: '資料不足，暫無法顯示搜尋效率。',
    searchEffectivenessReformulations: '次平均改寫',
    searchEffectivenessTrails: '{count} 條搜尋路徑',
    searchEffectivenessRewrites: '改寫 {count} 次',
    searchEffectivenessDepth: '平均深度 {count}',
    searchEffectivenessHelp:
      '這裡只統計明確的搜尋路徑。改寫越少、平均深度越低，通常越快找到結果。',
    searchEffectivenessRewritesLabel: '平均改寫',
    searchEffectivenessDepthLabel: '平均深度',
    searchEffectivenessTrailsLabel: '搜尋路徑',
    searchEffectivenessEngineRewrites: '平均每條搜尋路徑改寫 {count} 次',
    searchEffectivenessEngineDepth: '通常點到第 {count} 層才停下來',
    searchEffectivenessEngineTrails: '這段時間共形成 {count} 條搜尋路徑',
    searchEffectivenessSources: '常帶來結果的來源',
    searchEffectivenessSourcesHelp:
      '這些網站更常在搜尋後成為可停留、可繼續閱讀的結果頁。',
    searchEffectivenessHardest: '改寫最多的搜尋題目',
    searchEffectivenessHardestHelp:
      '這些題目通常需要多次換詞，或隔幾天又回來繼續搜。',
    searchEffectivenessLag: '{days} 天後又回來搜',
    frictionTitle: '碰壁與高摩擦偵測',
    frictionEmpty: '沒有檢測到碰壁信號。',
    frictionStrong: '強證據',
    frictionWeak: '弱證據',
    reopenedTitle: '反覆回來查的問題',
    reopenedEmpty: '沒有檢測到反覆查找的問題。',
    reopenedAnchorQuery: '查詢',
    reopenedAnchorPage: '頁面',
    reopenedOccurrences: '出現 {count} 次',
    reopenedDistinctDays: '跨 {days} 天',
    discoveryTrendTitle: '探索率趨勢',
    discoveryTrendEmpty: '資料不足，暫無法顯示探索率。',
    discoveryTrendHelp:
      '探索率 = 新網站數 / 總瀏覽次數。用它看最近是更常去新站，還是更常回到熟悉網站。',
    discoveryTrendNewDomains: '個新網站',
    discoveryTrendRateLabel: '探索率 %',
    discoveryTrendDomainsLabel: '新網站數',
    discoveryTrendWeekLabel: '{year} 第{week}周',
    discoveryTrendRatePercent: '{count}%',
    discoveryTrendVisitsLabel: '{count} 次瀏覽',
    domainInsightsTitle: '網域洞察',
    domainInsightsSubtitle: '查看這個網站在目前範圍內的完整行為與流向。',
    domainInsightsOpenExplorer: '打開網域證據',
    domainDeepDiveEmpty: '無法載入該網站的深度分析。',
    domainDeepDiveBack: '返回',
    domainDeepDiveVisits: '造訪次數',
    domainDeepDiveActiveDays: '活躍天數',
    domainDeepDiveTrails: '搜尋旅程',
    domainDeepDiveArrival: '到達方式',
    domainDeepDiveArrival_search: '搜尋',
    domainDeepDiveArrival_link: '鏈接',
    domainDeepDiveArrival_typed: '直接輸入',
    domainDeepDiveTopPages: '熱門頁面',
    domainDeepDiveReferrers: '主要來源',
    domainDeepDiveExits: '主要去向',
    domainDeepDiveTrend: '造訪趨勢',
    domainSearchKeywordsTitle: '在這個網站上搜尋過什麼',
    domainSearchKeywordsHelp:
      '只顯示目前時間範圍內在這個網站上實際搜尋過的關鍵詞。',
  },
} as const
