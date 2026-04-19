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

const intelligenceFallbackCopy: Record<
  ResolvedLanguage,
  Record<string, string>
> = {
  en: {
    archiveWideBadge: 'Archive-wide metrics',
    archiveWideBody:
      'You are looking at archive-wide Core Intelligence results. Switch to one browser profile when you want the analysis to narrow with it.',
    externalOutputsReviewBody:
      'If you need export-ready summaries or local host bundles, review them in Settings. This page stays focused on live Core Intelligence results.',
    category_community: 'Community',
  },
  'zh-CN': {
    archiveWideBadge: '全部存档统计',
    archiveWideBody:
      '当前显示的是整份存档的 Core Intelligence 结果。如果切到单一浏览器，分析内容也会跟着缩小范围。',
    externalOutputsReviewBody:
      '如果你要查看或建立可供外部使用的摘要，请到设置页面。这里会继续只显示 Core Intelligence 本身的分析结果。',
    category_community: '社区',
  },
  'zh-TW': {
    archiveWideBadge: '全部封存統計',
    archiveWideBody:
      '目前顯示的是整份封存的 Core Intelligence 結果。如果切到單一瀏覽器，分析內容也會跟著縮小範圍。',
    externalOutputsReviewBody:
      '如果你要查看或建立可供外部使用的摘要，請到設定頁面。這裡會繼續只顯示 Core Intelligence 本身的分析結果。',
    category_community: '社群',
  },
}

export function intelligenceText(
  language: ResolvedLanguage,
  _t: Translator,
  key: 'archiveWideBadge' | 'archiveWideBody' | 'externalOutputsReviewBody',
) {
  return intelligenceFallbackCopy[language][key]
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

  return intelligenceFallbackCopy[language][key] ?? category
}

export function formatDomainPagePath(path: string) {
  if (!path) return '/'

  try {
    return decodeURIComponent(path)
  } catch {
    return path
  }
}
