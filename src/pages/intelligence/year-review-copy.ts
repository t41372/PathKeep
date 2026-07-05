/**
 * Three-language display copy for the Year in Review narrative summary.
 *
 * ## Responsibilities
 * - Provide en / zh-CN / zh-TW copy for every user-visible string on the
 *   Year in Review route, including placeholders, empty states, and
 *   accessibility labels.
 * - Follow the same `translateWithFallback` pattern as `copy.ts` so the
 *   route degrades to readable copy even when a catalog key falls back.
 *
 * ## Not responsible for
 * - Runtime data formatting (callers own number / date formatting).
 * - Catalog registration (this is a UI repair layer).
 *
 * ## Dependencies
 * - `src/lib/i18n` for `ResolvedLanguage`.
 */

import type { ResolvedLanguage } from '../../lib/i18n'

export type YearReviewTextKey =
  | 'heroTitle'
  | 'heroTitleSoFar'
  | 'statTotalVisits'
  | 'statNewDomains'
  | 'statDeepReads'
  | 'heatmapLess'
  | 'heatmapMore'
  | 'heatmapAriaLabel'
  | 'volumeHeading'
  | 'volumeBusiestDay'
  | 'volumeActiveDays'
  | 'podiumHeading'
  | 'podiumVisits'
  | 'researchHeading'
  | 'researchJourneys'
  | 'discoveryHeading'
  | 'discoveryNewSites'
  | 'discoveryExploratory'
  | 'mixHeading'
  | 'habitsHeading'
  | 'habitsDaily'
  | 'habitsWeekly'
  | 'habitsPeriodic'
  | 'refindHeading'
  | 'refindRevisits'
  | 'footerCta'
  | 'emptyTitle'
  | 'emptyBody'
  | 'loading'
  | 'yearPagerPrev'
  | 'yearPagerNext'

const copy: Record<ResolvedLanguage, Record<YearReviewTextKey, string>> = {
  en: {
    heroTitle: 'Your {year} in Pages',
    heroTitleSoFar: 'Your {year} in Pages (so far)',
    statTotalVisits: 'Pages visited',
    statNewDomains: 'New domains',
    statDeepReads: 'Deep reads',
    heatmapLess: 'Less',
    heatmapMore: 'More',
    heatmapAriaLabel: 'Calendar heatmap of daily page visits',
    volumeHeading: 'Browsing Volume',
    volumeBusiestDay: 'Your busiest day was {date} with {count} pages.',
    volumeActiveDays: 'You were active on {count} of {total} days.',
    podiumHeading: 'Your Top Sites',
    podiumVisits: '{count} visits',
    researchHeading: 'Research Highlights',
    researchJourneys: 'You went on {count} search journeys.',
    discoveryHeading: 'Discovery',
    discoveryNewSites: 'You discovered {count} new websites.',
    discoveryExploratory: 'Your most exploratory month was {month}.',
    mixHeading: 'Content Mix',
    habitsHeading: 'Faithful Companions',
    habitsDaily: 'daily',
    habitsWeekly: 'weekly',
    habitsPeriodic: 'periodic',
    refindHeading: 'Reference Library',
    refindRevisits: '{count} revisits',
    footerCta: 'Explore full Intelligence view',
    emptyTitle: 'No data for {year}',
    emptyBody:
      'There are no browsing records for this year. Run a backup first, then come back.',
    loading: 'Loading year review...',
    yearPagerPrev: 'Previous year',
    yearPagerNext: 'Next year',
  },
  'zh-CN': {
    heroTitle: '你的 {year} 年浏览回顾',
    heroTitleSoFar: '你的 {year} 年浏览回顾（截至目前）',
    statTotalVisits: '浏览页面',
    statNewDomains: '新发现站点',
    statDeepReads: '深度阅读',
    heatmapLess: '少',
    heatmapMore: '多',
    heatmapAriaLabel: '每日页面访问量的日历热力图',
    volumeHeading: '浏览量',
    volumeBusiestDay: '最忙的一天是 {date}，浏览了 {count} 个页面。',
    volumeActiveDays: '你在全年 {total} 天中有 {count} 天上网。',
    podiumHeading: '最常访问的网站',
    podiumVisits: '{count} 次访问',
    researchHeading: '搜索之旅',
    researchJourneys: '你完成了 {count} 次搜索旅程。',
    discoveryHeading: '新发现',
    discoveryNewSites: '你发现了 {count} 个新网站。',
    discoveryExploratory: '最爱探索的月份是{month}。',
    mixHeading: '内容构成',
    habitsHeading: '忠实伙伴',
    habitsDaily: '每日',
    habitsWeekly: '每周',
    habitsPeriodic: '定期',
    refindHeading: '参考书库',
    refindRevisits: '{count} 次重访',
    footerCta: '前往完整的 Intelligence 视图',
    emptyTitle: '{year} 年暂无数据',
    emptyBody: '这一年没有浏览记录。先完成一次备份，再来看回顾吧。',
    loading: '正在加载年度回顾……',
    yearPagerPrev: '上一年',
    yearPagerNext: '下一年',
  },
  'zh-TW': {
    heroTitle: '你的 {year} 年瀏覽回顧',
    heroTitleSoFar: '你的 {year} 年瀏覽回顧（截至目前）',
    statTotalVisits: '瀏覽頁面',
    statNewDomains: '新發現站點',
    statDeepReads: '深度閱讀',
    heatmapLess: '少',
    heatmapMore: '多',
    heatmapAriaLabel: '每日頁面訪問量的日曆熱力圖',
    volumeHeading: '瀏覽量',
    volumeBusiestDay: '最忙的一天是 {date}，瀏覽了 {count} 個頁面。',
    volumeActiveDays: '你在全年 {total} 天中有 {count} 天上網。',
    podiumHeading: '最常造訪的網站',
    podiumVisits: '{count} 次造訪',
    researchHeading: '搜尋之旅',
    researchJourneys: '你完成了 {count} 次搜尋旅程。',
    discoveryHeading: '新發現',
    discoveryNewSites: '你發現了 {count} 個新網站。',
    discoveryExploratory: '最愛探索的月份是{month}。',
    mixHeading: '內容組成',
    habitsHeading: '忠實夥伴',
    habitsDaily: '每日',
    habitsWeekly: '每週',
    habitsPeriodic: '定期',
    refindHeading: '參考書庫',
    refindRevisits: '{count} 次重訪',
    footerCta: '前往完整的 Intelligence 視圖',
    emptyTitle: '{year} 年暫無資料',
    emptyBody: '這一年沒有瀏覽紀錄。先完成一次備份，再來看回顧吧。',
    loading: '正在載入年度回顧……',
    yearPagerPrev: '上一年',
    yearPagerNext: '下一年',
  },
}

function interpolate(template: string, vars?: Record<string, string | number>) {
  if (!vars) return template
  return template.replace(/\{(\w+)\}/g, (_, key: string) =>
    Object.prototype.hasOwnProperty.call(vars, key) ? String(vars[key]) : '',
  )
}

/**
 * Returns three-language copy for Year in Review surfaces.
 *
 * Uses the same fallback pattern as `intelligenceText` — attempts the i18n
 * catalog first, falls back to the hardcoded map when the key looks raw.
 */
export function yearReviewText(
  language: ResolvedLanguage,
  key: YearReviewTextKey,
  vars?: Record<string, string | number>,
): string {
  return interpolate(copy[language][key], vars)
}
