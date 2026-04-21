/**
 * Shared display copy helpers for Intelligence surfaces.
 *
 * Why this file exists:
 * - The live desktop route should degrade to human-readable copy even if one
 *   translation key falls back to its raw namespace path.
 * - Domain deep dives also need one place to normalize encoded page paths
 *   before they become visible UI text.
 *
 * Main declarations:
 * - `intelligenceText`
 * - `intelligenceCategoryLabel`
 * - `formatDomainPagePath`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/features/intelligence-current-state.md`.
 * - These helpers are a UI repair layer, not a replacement for the catalog.
 */

import type { ResolvedLanguage } from '../../lib/i18n'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

export type IntelligenceTextKey =
  | 'archiveWideBadge'
  | 'archiveWideBody'
  | 'externalOutputsReviewBody'
  | 'storageAnalytics'
  | 'growthSignal'
  | 'storageAnalyticsDescription'
  | 'trackedStorage'
  | 'reclaimableSpace'
  | 'dominantStorage'
  | 'latestRunGrowthBody'
  | 'noGrowthEvidenceDescription'
  | 'openGrowthAuditRun'

export type CommonHealthTextKey =
  | 'coreHistory'
  | 'otherData'
  | 'canonicalArchive'
  | 'sourceEvidence'
  | 'searchProjection'
  | 'intelligenceProjection'
  | 'semanticIndex'
  | 'contentBlobs'
  | 'auditArtifacts'
  | 'exports'
  | 'temporaryFiles'

const intelligenceFallbackCopy: Record<
  ResolvedLanguage,
  Record<IntelligenceTextKey | 'category_community', string>
> = {
  en: {
    archiveWideBadge: 'Archive-wide metrics',
    archiveWideBody:
      'You are looking at archive-wide Core Intelligence results. Switch to one browser profile when you want the analysis to narrow with it.',
    externalOutputsReviewBody:
      'If you need export-ready summaries or local host bundles, review them in Settings. This page stays focused on live Core Intelligence results.',
    storageAnalytics: 'Storage',
    growthSignal: 'Recent growth',
    storageAnalyticsDescription:
      'See what uses disk space, what can be reclaimed, and what the latest backup added.',
    trackedStorage: 'Total used',
    reclaimableSpace: 'Reclaimable',
    dominantStorage: 'Largest category',
    latestRunGrowthBody:
      'Added {visits} visits, {urls} URLs, and {downloads} downloads.',
    noGrowthEvidenceDescription:
      'Run at least one backup to see how your archive grows over time.',
    openGrowthAuditRun: 'Open this backup in Audit',
    category_community: 'Community',
  },
  'zh-CN': {
    archiveWideBadge: '全部存档统计',
    archiveWideBody:
      '当前显示的是整份存档的 Core Intelligence 结果。如果切到单一浏览器，分析内容也会跟着缩小范围。',
    externalOutputsReviewBody:
      '如果你要查看或建立可供外部使用的摘要，请到设置页面。这里会继续只显示 Core Intelligence 本身的分析结果。',
    storageAnalytics: '存储空间',
    growthSignal: '数据增长',
    storageAnalyticsDescription:
      '查看本地数据占用情况，包括哪些数据占空间最多、有多少可以清理，以及最近一次备份新增了多少。',
    trackedStorage: '已用空间',
    reclaimableSpace: '可清理',
    dominantStorage: '最大占用',
    latestRunGrowthBody:
      '最近一次备份新增了 {visits} 条访问、{urls} 个网址和 {downloads} 条下载记录。',
    noGrowthEvidenceDescription:
      '完成第一次备份后，这里会显示每次备份新增的数据量。',
    openGrowthAuditRun: '在审计日志中查看这次备份',
    category_community: '社区',
  },
  'zh-TW': {
    archiveWideBadge: '全部封存統計',
    archiveWideBody:
      '目前顯示的是整份封存的 Core Intelligence 結果。如果切到單一瀏覽器，分析內容也會跟著縮小範圍。',
    externalOutputsReviewBody:
      '如果你要查看或建立可供外部使用的摘要，請到設定頁面。這裡會繼續只顯示 Core Intelligence 本身的分析結果。',
    storageAnalytics: '儲存空間',
    growthSignal: '資料成長',
    storageAnalyticsDescription:
      '查看本機資料佔用情況，包括哪些資料佔用最多、有多少可以清理，以及最近一次備份新增了多少。',
    trackedStorage: '已用空間',
    reclaimableSpace: '可清理',
    dominantStorage: '最大佔用',
    latestRunGrowthBody:
      '最近一次備份新增了 {visits} 筆瀏覽、{urls} 個網址和 {downloads} 筆下載紀錄。',
    noGrowthEvidenceDescription:
      '完成第一次備份後，這裡會顯示每次備份新增的資料量。',
    openGrowthAuditRun: '在稽核日誌中查看這次備份',
    category_community: '社群',
  },
}

const commonHealthFallbackCopy: Record<
  ResolvedLanguage,
  Record<CommonHealthTextKey, string>
> = {
  en: {
    coreHistory: 'Core history',
    otherData: 'Other data',
    canonicalArchive: 'Canonical archive',
    sourceEvidence: 'Source evidence',
    searchProjection: 'Search projection',
    intelligenceProjection: 'Intelligence projection',
    semanticIndex: 'Semantic index',
    contentBlobs: 'Content blobs',
    auditArtifacts: 'Audit artifacts',
    exports: 'Exports',
    temporaryFiles: 'Temporary files',
  },
  'zh-CN': {
    coreHistory: '核心浏览记录',
    otherData: '其他数据',
    canonicalArchive: '规范化存档',
    sourceEvidence: '来源证据',
    searchProjection: '搜索投影',
    intelligenceProjection: '智能投影',
    semanticIndex: '语义索引',
    contentBlobs: '正文缓存',
    auditArtifacts: '审计产物',
    exports: '导出',
    temporaryFiles: '临时文件',
  },
  'zh-TW': {
    coreHistory: '核心瀏覽紀錄',
    otherData: '其他資料',
    canonicalArchive: '規範化封存',
    sourceEvidence: '來源證據',
    searchProjection: '搜尋投影',
    intelligenceProjection: '智慧投影',
    semanticIndex: '語意索引',
    contentBlobs: '正文快取',
    auditArtifacts: '稽核產物',
    exports: '匯出',
    temporaryFiles: '暫存檔',
  },
}

function interpolate(template: string, vars?: Record<string, string | number>) {
  if (!vars) {
    return template
  }

  return template.replace(/\{(\w+)\}/g, (_, key: string) =>
    Object.prototype.hasOwnProperty.call(vars, key) ? String(vars[key]) : '',
  )
}

function translationLooksRaw(namespace: string, key: string, value: string) {
  const normalized = value.trim()
  const capitalizedNamespace =
    namespace.charAt(0).toUpperCase() + namespace.slice(1)
  return (
    normalized === `${namespace}.${key}` ||
    normalized === `${capitalizedNamespace}.${key}` ||
    normalized === key
  )
}

function translateWithFallback<K extends string>(
  fallback: Record<ResolvedLanguage, Record<K, string>>,
  language: ResolvedLanguage,
  namespace: string,
  t: Translator,
  key: K,
  vars?: Record<string, string | number>,
) {
  const translated = t(key, vars)
  if (!translationLooksRaw(namespace, key, translated)) {
    return translated
  }

  return interpolate(fallback[language][key], vars)
}

export function intelligenceText(
  language: ResolvedLanguage,
  t: Translator,
  key: IntelligenceTextKey | 'archiveWideBadge' | 'archiveWideBody',
  vars?: Record<string, string | number>,
) {
  return translateWithFallback(
    intelligenceFallbackCopy,
    language,
    'intelligence',
    t,
    key,
    vars,
  )
}

export function commonHealthText(
  language: ResolvedLanguage,
  t: Translator,
  key: CommonHealthTextKey,
) {
  return translateWithFallback(
    commonHealthFallbackCopy,
    language,
    'common',
    t,
    key,
  )
}

export function intelligenceCategoryLabel(
  language: ResolvedLanguage,
  t: Translator,
  category: string,
) {
  if (category === 'community') {
    return intelligenceFallbackCopy[language].category_community
  }

  const key = `category_${category}`
  const translated = t(key)
  const normalized = translated.trim()
  if (
    normalized !== `intelligence.${key}` &&
    normalized !== `Intelligence.${key}`
  ) {
    return translated
  }

  return (
    intelligenceFallbackCopy[language][
      key as keyof (typeof intelligenceFallbackCopy)[ResolvedLanguage]
    ] ?? category
  )
}

export function formatDomainPagePath(path: string) {
  if (!path) return '/'

  try {
    return decodeURIComponent(path)
  } catch {
    return path
  }
}
