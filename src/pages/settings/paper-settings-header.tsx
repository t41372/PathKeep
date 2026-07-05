/**
 * Paper-redesign header rendered above the Settings sections when the route
 * has `?layout=paper`. It replaces the v0.2 `.settings-overview` intro card
 * with a literary serif title, mono eyebrow, and a paper-styled jump-to nav.
 *
 * ## Responsibilities
 * - Render the eyebrow / title / subtitle block in paper typography.
 * - Re-render the section nav items as inline anchor pills so the visual
 *   language is consistent with PaperImportPanel and PaperAuditPanel.
 *
 * ## Not responsible for
 * - Owning section scroll behaviour — anchor clicks defer to the browser
 *   hash routing already wired into Settings.
 * - Reskinning the individual Settings section panels (still v0.2 in this
 *   pass; a deeper sweep follows).
 */

import { useCallback, useEffect, useMemo, useRef } from 'react'
import { useLocation } from 'react-router-dom'
import type { SettingsSectionNavItem } from './section-nav-items'

export interface PaperSettingsHeaderProps {
  eyebrow: string
  title: string
  subtitle: string
  jumpLabel: string
  items: readonly SettingsSectionNavItem[]
  testId?: string
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

function scrollSectionIntoView(sectionId: string) {
  const target = document.getElementById(sectionId)
  if (!(target instanceof HTMLElement)) return
  target.scrollIntoView({ block: 'start' })
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

export function PaperSettingsHeader({
  eyebrow,
  title,
  subtitle,
  jumpLabel,
  items,
  testId,
}: PaperSettingsHeaderProps) {
  const location = useLocation()
  const sectionIds = useMemo(() => items.map((item) => item.id), [items])
  // Mirror section-id membership into a ref so the auto-scroll effect can
  // depend ONLY on the hash. A parent that mints a fresh `items` array each
  // render would otherwise change `sectionIds` identity and re-fire the
  // deep-link scroll on every render, yanking the viewport back up. Declared
  // before the scroll effect so the ref is fresh on the same commit.
  const sectionIdsRef = useRef(sectionIds)
  useEffect(() => {
    sectionIdsRef.current = sectionIds
  }, [sectionIds])
  const handleSectionClick = useCallback((sectionId: string) => {
    scheduleSectionScroll(sectionId)
  }, [])

  // Auto-scroll fires ONCE per actual hash arrival; explicit jump-pill clicks
  // still scroll via scheduleSectionScroll, but a re-render at the same hash
  // must never re-scroll.
  const autoScrolledHashRef = useRef<string | null>(null)
  useEffect(() => {
    if (autoScrolledHashRef.current === location.hash) return
    autoScrolledHashRef.current = location.hash

    const sectionId = sectionIdFromLocationHash(
      location.hash,
      sectionIdsRef.current,
    )
    if (!sectionId) return
    return scheduleSectionScroll(sectionId)
  }, [location.hash])

  return (
    <header
      data-testid={testId ?? 'paper-settings-header'}
      className="border-border-light mb-6 border-b pb-6"
    >
      <span className="text-ink-faint mb-2 block font-mono text-[10px] tracking-[0.08em] uppercase">
        {eyebrow}
      </span>
      <h2 className="text-ink m-0 font-serif text-[28px] leading-[1.2] tracking-[-0.01em]">
        {title}
      </h2>
      <p className="text-ink-muted mt-3 max-w-[640px] font-serif text-[15px] leading-[1.55] italic">
        {subtitle}
      </p>

      <nav
        aria-label={jumpLabel}
        className="mt-5 flex flex-wrap items-center gap-2"
      >
        <span className="text-ink-faint mr-1 font-mono text-[10px] tracking-[0.08em] uppercase">
          {jumpLabel}
        </span>
        {items.map((item) => (
          <a
            key={item.key}
            href={`#${location.pathname}${location.search}#${item.id}`}
            onClick={() => handleSectionClick(item.id)}
            className="rounded-paper border-border-light hover:bg-hover text-ink border px-3 py-1 font-mono text-[11px] tracking-[0.02em] transition-colors"
          >
            {item.label}
          </a>
        ))}
      </nav>
    </header>
  )
}
