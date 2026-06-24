/**
 * Settings section anchor descriptors.
 *
 * ## 職責
 * - 定義 Settings 各 section 的單一 source-of-truth：anchor id、glyph、與可翻譯標籤。
 * - 提供 route 與 extracted section renderers 共用的 typed descriptor。
 * - 保持 sticky nav 與 panel anchor ids 維持同一組 stable contract。
 *
 * ## 不負責
 * - 不渲染 sticky nav 本身。
 * - 不持有任何 route state、draft state、backend mutation 或 polling。
 * - 不決定 section 內的 UI 或 workflow 行為。
 *
 * ## 依賴關係
 * - 依賴 `src/components/ui.tsx` 的 `GlyphIconName` 型別來對齊 icon contract。
 * - 由 Settings route 傳入翻譯函數，映射當前 locale 的 section label。
 *
 * ## 性能備注
 * - descriptor list 很小，建立成本固定且可忽略；本模組不做資料查詢或重計算。
 */

import type { GlyphIconName } from '../../components/ui'

/**
 * Names the stable Settings section keys used by the nav descriptor list.
 *
 * The key is route-internal only, but typing it prevents later slices from
 * rewiring anchors and labels through ad-hoc string literals.
 */
export type SettingsSectionKey =
  | 'appearance'
  | 'general'
  | 'updater'
  | 'retention'
  | 'applock'
  | 'profiles'
  | 'ai'
  | 'contentFetch'
  | 'derived'
  | 'migration'
  | 'linkPreviews'
  | 'platform'

interface SettingsSectionSpec {
  id: string
  icon: GlyphIconName
  labelKey: string
}

const settingsSectionSpecs: Record<SettingsSectionKey, SettingsSectionSpec> = {
  appearance: {
    // Anchor id MUST match the AppearanceSection container's default `anchorId`
    // ('appearance') so the sticky-nav / deep-link scroll lands on the card.
    id: 'appearance',
    icon: 'auto_stories',
    labelKey: 'settings.appearanceTitle',
  },
  general: {
    id: 'settings-general',
    icon: 'settings',
    labelKey: 'settings.general',
  },
  updater: {
    id: 'settings-updater',
    icon: 'system_update',
    labelKey: 'settings.updateTitle',
  },
  retention: {
    id: 'settings-retention',
    icon: 'delete_sweep',
    labelKey: 'settings.retentionTitle',
  },
  applock: {
    id: 'settings-applock',
    icon: 'shield',
    labelKey: 'settings.appLock',
  },
  profiles: {
    id: 'settings-profiles',
    icon: 'language',
    labelKey: 'settings.browserProfiles',
  },
  ai: {
    id: 'settings-ai',
    icon: 'smart_toy',
    labelKey: 'settings.aiProvider',
  },
  contentFetch: {
    // Anchor id MUST match the ContentFetchSection container's default
    // `anchorId` ('content-fetch') so the sticky-nav scroll lands on the
    // consent card, not a phantom id.
    id: 'content-fetch',
    icon: 'public',
    labelKey: 'settings.contentFetchNavLabel',
  },
  derived: {
    id: 'settings-derived',
    icon: 'memory',
    labelKey: 'settings.enrichmentDerivedState',
  },
  migration: {
    id: 'settings-migration',
    icon: 'download',
    labelKey: 'settings.migrationTitle',
  },
  linkPreviews: {
    id: 'settings-link-previews',
    icon: 'preview',
    labelKey: 'settings.linkPreviewsTitle',
  },
  platform: {
    id: 'settings-platform',
    icon: 'build',
    labelKey: 'settings.platformTroubleshooting',
  },
}

const defaultSettingsSectionOrder: SettingsSectionKey[] = [
  'appearance',
  'general',
  'profiles',
  'applock',
  'ai',
  'contentFetch',
  'migration',
  'linkPreviews',
  'updater',
  'retention',
  'derived',
  'platform',
]

/**
 * Describes one Settings section anchor as rendered in the sticky nav.
 *
 * The route and extracted section components both consume this shape so they
 * can share one stable id/icon/label contract without duplicating literals.
 */
export interface SettingsSectionNavItem {
  id: string
  icon: GlyphIconName
  key: SettingsSectionKey
  label: string
}

/**
 * Builds the translated Settings section descriptors from the stable spec map.
 *
 * The Settings route owns locale changes, so this helper only maps fixed ids
 * and icons onto the current translated labels.
 */
export function createSettingsSectionNavItems(
  translate: (key: string) => string,
  keys: SettingsSectionKey[] = defaultSettingsSectionOrder,
): SettingsSectionNavItem[] {
  return keys.map((key) => {
    const spec = settingsSectionSpecs[key]
    return {
      key,
      id: spec.id,
      icon: spec.icon,
      label: translate(spec.labelKey),
    }
  })
}

/**
 * Resolves one descriptor from the nav item list.
 *
 * Extracted section renderers use this lookup so their panel ids stay tied to
 * the same descriptor list that drives the sticky nav.
 */
export function getSettingsSectionNavItem(
  items: SettingsSectionNavItem[],
  key: SettingsSectionKey,
): SettingsSectionNavItem {
  const item = items.find((candidate) => candidate.key === key)

  if (!item) {
    throw new Error(`Missing settings section nav item: ${key}`)
  }

  return item
}
