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

import { useCallback, useEffect, useMemo, useRef } from 'react'
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
  // Mirror the (possibly re-derived) section-id membership into a ref so the
  // auto-scroll effect can depend ONLY on the hash. Without this, a parent that
  // hands us a fresh `items` array each render (e.g. an unrelated state change)
  // would change `sectionIds` identity and re-fire the deep-link scroll on
  // every render, yanking the viewport back to the hashed section. This sync
  // effect is declared BEFORE the scroll effect so the ref is fresh by the time
  // the scroll effect reads it on the same commit.
  const sectionIdsRef = useRef(sectionIds)
  useEffect(() => {
    sectionIdsRef.current = sectionIds
  }, [sectionIds])
  const handleSectionClick = useCallback((sectionId: string) => {
    scheduleSectionScroll(sectionId)
  }, [])

  // Guard the hash-driven auto-scroll so it fires ONCE per actual hash arrival.
  // An explicit nav-pill click still scrolls (it calls scheduleSectionScroll
  // directly), but a re-render at the same hash must never re-scroll.
  const autoScrolledHashRef = useRef<string | null>(null)
  useEffect(() => {
    if (autoScrolledHashRef.current === location.hash) {
      return
    }
    autoScrolledHashRef.current = location.hash

    const sectionId = sectionIdFromLocationHash(
      location.hash,
      sectionIdsRef.current,
    )
    if (!sectionId) {
      return
    }

    return scheduleSectionScroll(sectionId)
  }, [location.hash])

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
