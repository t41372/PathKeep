/**
 * Sticky Settings section navigation.
 *
 * ## 職責
 * - 渲染 sticky section nav，讓 route 內的 jump links 和 panel anchor 維持一致。
 * - 顯示 compact text-labeled chrome，同時保留可存取的 section label。
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

import { useCallback, useEffect, useMemo } from 'react'
import { useLocation } from 'react-router-dom'
import { Glyph } from '../../components/ui'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Props for the sticky Settings section nav.
 */
export interface SettingsSectionNavProps {
  items: SettingsSectionNavItem[]
  label: string
}

function focusSection(element: HTMLElement) {
  if (!element.hasAttribute('tabindex')) {
    element.setAttribute('tabindex', '-1')
  }

  try {
    element.focus({ preventScroll: true })
  } catch {
    element.focus()
  }
}

function prefersReducedMotion() {
  return (
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  )
}

function scrollSectionIntoView(sectionId: string) {
  const target = document.getElementById(sectionId)
  if (!(target instanceof HTMLElement)) {
    return
  }

  // Smooth-scroll the deep-linked / clicked section into view, but fall back to
  // an instant jump when the user has asked for reduced motion (a11y).
  target.scrollIntoView({
    behavior: prefersReducedMotion() ? 'auto' : 'smooth',
    block: 'start',
  })
  focusSection(target)
}

function scheduleSectionScroll(sectionId: string) {
  if (typeof window.requestAnimationFrame === 'function') {
    const frame = window.requestAnimationFrame(() =>
      scrollSectionIntoView(sectionId),
    )
    return () => window.cancelAnimationFrame?.(frame)
  }

  const timeout = window.setTimeout(() => scrollSectionIntoView(sectionId), 0)
  return () => window.clearTimeout(timeout)
}

function sectionIdFromLocationHash(hash: string, sectionIds: string[]) {
  const sectionId = hash.replace(/^#/, '')
  return sectionIds.includes(sectionId) ? sectionId : null
}

/**
 * Renders the sticky Settings section nav with translated visible labels.
 *
 * Each link keeps its icon as a quick anchor, but the visible label is the
 * primary affordance so users do not need to memorize symbols before editing preferences.
 */
export function SettingsSectionNav({ items, label }: SettingsSectionNavProps) {
  const location = useLocation()
  const sectionIds = useMemo(() => items.map((item) => item.id), [items])
  const handleSectionClick = useCallback((sectionId: string) => {
    scheduleSectionScroll(sectionId)
  }, [])

  useEffect(() => {
    const sectionId = sectionIdFromLocationHash(location.hash, sectionIds)
    if (!sectionId) {
      return
    }

    return scheduleSectionScroll(sectionId)
  }, [location.hash, sectionIds])

  return (
    <nav className="settings-nav" aria-label={label}>
      {items.map((item) => (
        <a
          key={item.key}
          aria-label={item.label}
          className="settings-nav__link"
          href={`#${location.pathname}${location.search}#${item.id}`}
          onClick={() => handleSectionClick(item.id)}
          title={item.label}
        >
          <Glyph icon={item.icon} filled />
          <span className="settings-nav__label">{item.label}</span>
        </a>
      ))}
    </nav>
  )
}
