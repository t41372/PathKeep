/**
 * Sticky Settings section navigation.
 *
 * ## 職責
 * - 渲染 sticky section nav，讓 route 內的 jump links 和 panel anchor 維持一致。
 * - 顯示 compact icon-only chrome，同時保留可存取的 section label。
 * - 消費外部傳入的 descriptor list，而不是自行維護第二套 ids 或標籤。
 *
 * ## 不負責
 * - 不定義 Settings section 的 descriptor source-of-truth。
 * - 不持有任何 route state、draft state、backend mutation 或 polling。
 * - 不渲染各 section 的具體 panel 內容。
 *
 * ## 依賴關係
 * - 依賴 `src/components/ui.tsx` 的 shared `Glyph` primitive。
 * - 消費 `section-nav-items.ts` 提供的 typed descriptor list。
 *
 * ## 性能備注
 * - 本模組只渲染少量 anchors，不做資料查詢或重計算。
 */

import { Glyph } from '../../components/ui'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Props for the sticky Settings section nav.
 */
export interface SettingsSectionNavProps {
  items: SettingsSectionNavItem[]
  label: string
}

/**
 * Renders the sticky icon-only Settings section nav with translated labels.
 *
 * Each link keeps its accessible name while the visual chrome stays compact,
 * which lets us preserve the in-flight nav redesign without hiding meaning
 * from keyboard or assistive-technology users.
 */
export function SettingsSectionNav({ items, label }: SettingsSectionNavProps) {
  return (
    <nav className="settings-nav" aria-label={label}>
      {items.map((item) => (
        <a
          key={item.key}
          aria-label={item.label}
          className="settings-nav__link"
          href={`#${item.id}`}
          title={item.label}
        >
          <Glyph icon={item.icon} filled />
        </a>
      ))}
    </nav>
  )
}
