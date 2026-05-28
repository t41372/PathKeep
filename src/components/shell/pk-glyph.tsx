/**
 * PathKeep glyph renderer for the paper redesign.
 *
 * Why this file exists:
 * - Sidebar nav items, topbar buttons, and detail panel actions all consume
 *   the same icon vocabulary. Centralizing it keeps the visual identity
 *   coherent (1.8 stroke, 24x24 viewBox, currentColor stroke).
 * - The legacy v0.2 ui.tsx glyph wrapper is being phased out alongside the
 *   shell rebuild; this is its replacement.
 *
 * Adding a new glyph:
 * - Add an entry to GLYPH_PATHS with the SVG path commands. Names use
 *   snake_case to stay consistent with the design package.
 */

import { cn } from '@/lib/cn'

type GlyphRenderer = () => React.ReactElement

const GLYPH_PATHS: Record<string, GlyphRenderer> = {
  bar_chart: () => (
    <>
      <path d="M4.5 19.5h15" />
      <path d="M7 18v-5" />
      <path d="M12 18V7" />
      <path d="M17 18v-9" />
    </>
  ),
  auto_stories: () => (
    <>
      <path d="M6.5 5.5h4.5A3 3 0 0 1 14 8.5v10H9.5A3 3 0 0 0 6.5 21z" />
      <path d="M17.5 5.5H13A3 3 0 0 0 10 8.5v10h4.5A3 3 0 0 1 17.5 21z" />
      <path d="M10 9.5h4" />
    </>
  ),
  search: () => (
    <>
      <circle cx="10.5" cy="10.5" r="4.5" />
      <path d="m14 14 5 5" />
    </>
  ),
  memory: () => (
    <>
      <rect height="8" rx="1.5" width="10" x="7" y="8" />
      <path d="M9.5 8V6" />
      <path d="M12 8V6" />
      <path d="M14.5 8V6" />
      <path d="M9.5 18v-2" />
      <path d="M12 18v-2" />
      <path d="M14.5 18v-2" />
      <path d="M7 10H5" />
      <path d="M7 14H5" />
      <path d="M19 10h-2" />
      <path d="M19 14h-2" />
    </>
  ),
  smart_toy: () => (
    <>
      <rect height="8" rx="2" width="10" x="7" y="8" />
      <path d="M12 8V5.5" />
      <path d="m10 17 1.2 1.5" />
      <path d="m14 17-1.2 1.5" />
      <path d="M8 11H6.5" />
      <path d="M17.5 11H16" />
      <circle cx="10" cy="11.5" fill="currentColor" stroke="none" r="0.8" />
      <circle cx="14" cy="11.5" fill="currentColor" stroke="none" r="0.8" />
      <path d="M10 14h4" />
    </>
  ),
  download: () => (
    <>
      <path d="M12 4v9.5" />
      <path d="m8 13 4 4 4-4" />
      <path d="M4 20h16" />
    </>
  ),
  history: () => (
    <>
      <path d="M3.5 5.5V10H8" />
      <path d="M4.3 10A8 8 0 1 0 8 5.3" />
      <path d="M12 8v4.2l2.8 1.8" />
    </>
  ),
  sync: () => (
    <>
      <path d="M20 7h-6a5 5 0 0 0-5 5v1" />
      <path d="m17 4 3 3-3 3" />
      <path d="M4 17h6a5 5 0 0 0 5-5v-1" />
      <path d="m7 20-3-3 3-3" />
    </>
  ),
  settings: () => (
    <>
      <path d="M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.2a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.8a2 2 0 0 1-1 1.7l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.2a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.2a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.7v-.8a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.2a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </>
  ),
  preview: () => (
    <>
      <path d="M2.5 12s3.5-5.5 9.5-5.5S21.5 12 21.5 12s-3.5 5.5-9.5 5.5S2.5 12 2.5 12Z" />
      <circle cx="12" cy="12" r="2.5" />
    </>
  ),
  shield: () => (
    <>
      <path d="M12 3.5 19 6v5.5c0 4.2-2.7 8-7 9.7-4.3-1.7-7-5.5-7-9.7V6z" />
    </>
  ),
  database: () => (
    <>
      <ellipse cx="12" cy="6" rx="7" ry="3" />
      <path d="M5 6v6c0 2 3 3 7 3s7-1 7-3V6" />
      <path d="M5 12v6c0 2 3 3 7 3s7-1 7-3v-6" />
    </>
  ),
  cloud_upload: () => (
    <>
      <path d="M7 18a4 4 0 1 1 .8-7.9A5.2 5.2 0 0 1 18 11a3.5 3.5 0 1 1 0 7H7z" />
      <path d="M12 15V9.5" />
      <path d="m9.5 11.8 2.5-2.5 2.5 2.5" />
    </>
  ),
  folder_open: () => (
    <>
      <path d="M3.5 9.5a2 2 0 0 1 2-2H10l2 2h6.5a2 2 0 0 1 2 2l-1 6.5a2 2 0 0 1-2 1.5H6a2 2 0 0 1-2-1.5z" />
      <path d="M3.5 9.5V7A2 2 0 0 1 5.5 5H10l2 2h4" />
    </>
  ),
  warning: () => (
    <>
      <path d="M12 4.5 20 19H4z" />
      <path d="M12 9v4.5" />
      <circle cx="12" cy="16.5" fill="currentColor" stroke="none" r="0.8" />
    </>
  ),
  check: () => (
    <>
      <path d="m5 12.5 4.2 4.2L19 7.5" />
    </>
  ),
  content_copy: () => (
    <>
      <rect height="11" rx="1.5" width="10" x="9" y="7" />
      <path d="M15 5H6a1 1 0 0 0-1 1v9" />
    </>
  ),
  arrow_forward: () => (
    <>
      <path d="M5 12h14M13 6l6 6-6 6" />
    </>
  ),
  arrow_back: () => (
    <>
      <path d="M19 12H5M11 18l-6-6 6-6" />
    </>
  ),
  arrow_right: () => (
    <>
      <path d="M9 6l6 6-6 6" />
    </>
  ),
  arrow_left: () => (
    <>
      <path d="M15 6l-6 6 6 6" />
    </>
  ),
  arrow_down: () => (
    <>
      <path d="M6 9l6 6 6-6" />
    </>
  ),
  arrow_up: () => (
    <>
      <path d="M6 15l6-6 6 6" />
    </>
  ),
  public: () => (
    <>
      <circle cx="12" cy="12" r="8" />
      <path d="M4 12h16" />
      <path d="M12 4c2.4 2.1 3.6 4.8 3.6 8s-1.2 5.9-3.6 8c-2.4-2.1-3.6-4.8-3.6-8S9.6 6.1 12 4Z" />
    </>
  ),
  link: () => (
    <>
      <path d="M10.5 13.5l3-3M13.5 8.5l1.2-1.2a3 3 0 014.2 4.2L17.5 13M10.5 11l-1.2 1.2a3 3 0 11-4.2-4.2L6.5 6.8" />
    </>
  ),
  branch: () => (
    <>
      <circle cx="7" cy="5" r="2" />
      <circle cx="7" cy="19" r="2" />
      <circle cx="16" cy="12" r="2" />
      <path d="M7 7v10" />
      <path d="M7 11c0 2 1 3 3 3h4" />
    </>
  ),
  bookmark: () => (
    <>
      <path d="M7 4h10v17l-5-4-5 4z" />
    </>
  ),
  build: () => (
    <>
      <path d="M14.7 6.3a3.7 3.7 0 0 0 5 5L10.2 20.8a2 2 0 0 1-2.8 0L5.2 18.6a2 2 0 0 1 0-2.8z" />
    </>
  ),
  delete_sweep: () => (
    <>
      <path d="M5 7h14" />
      <path d="M9.5 7V5.5A1.5 1.5 0 0 1 11 4h2a1.5 1.5 0 0 1 1.5 1.5V7" />
      <path d="m8 7 1 12h6l1-12" />
      <path d="M11 10.5v5" />
      <path d="M13 10.5v5" />
    </>
  ),
  language: () => (
    <>
      <circle cx="12" cy="12" r="8" />
      <path d="M4 12h16" />
      <path d="M12 4c2.4 2.1 3.6 4.8 3.6 8s-1.2 5.9-3.6 8c-2.4-2.1-3.6-4.8-3.6-8S9.6 6.1 12 4Z" />
    </>
  ),
  notifications: () => (
    <>
      <path d="M18 10.5A6 6 0 0 0 6 10.5c0 4-1.8 5.2-2.5 6h17c-.7-.8-2.5-2-2.5-6Z" />
      <path d="M9.8 19a2.3 2.3 0 0 0 4.4 0" />
    </>
  ),
  system_update: () => (
    <>
      <path d="M12 5v9" />
      <path d="m8.5 10.5 3.5 3.5 3.5-3.5" />
      <path d="M5 18h14v2H5z" />
    </>
  ),
  play: () => (
    <>
      <path d="M8 5l11 7-11 7z" />
    </>
  ),
  pause: () => (
    <>
      <rect x="7" y="5" width="3" height="14" />
      <rect x="14" y="5" width="3" height="14" />
    </>
  ),
  close: () => (
    <>
      <path d="M6 6l12 12M18 6l-6 6-6 6" />
    </>
  ),
  plus: () => (
    <>
      <path d="M12 5v14M5 12h14" />
    </>
  ),
  minus: () => (
    <>
      <path d="M5 12h14" />
    </>
  ),
  sun: () => (
    <>
      <circle cx="12" cy="12" r="3.5" />
      <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4l1.4-1.4M17 7l1.4-1.4" />
    </>
  ),
  moon: () => (
    <>
      <path d="M19 14.5A8.5 8.5 0 1 1 9.5 5a6.5 6.5 0 0 0 9.5 9.5z" />
    </>
  ),
  lock: () => (
    <>
      <rect x="5" y="11" width="14" height="9" rx="1" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    </>
  ),
  unlock: () => (
    <>
      <rect x="5" y="11" width="14" height="9" rx="1" />
      <path d="M8 11V8a4 4 0 0 1 7.5-2" />
    </>
  ),
  refresh: () => (
    <>
      <path d="M3 12a9 9 0 0 1 15-6.7l3-3" />
      <path d="M21 3v6h-6" />
      <path d="M21 12a9 9 0 0 1-15 6.7l-3 3" />
      <path d="M3 21v-6h6" />
    </>
  ),
}

export type GlyphIconName = keyof typeof GLYPH_PATHS

export interface PKGlyphProps {
  icon: GlyphIconName
  size?: number
  strokeWidth?: number
  className?: string
}

export function PKGlyph({
  icon,
  size = 18,
  strokeWidth = 1.8,
  className,
}: PKGlyphProps) {
  const render = GLYPH_PATHS[icon]
  if (!render) {
    return null
  }
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      width={size}
      height={size}
      className={cn('block shrink-0', className)}
      aria-hidden="true"
    >
      {render()}
    </svg>
  )
}

const PK_GLYPH_NAMES_INTERNAL = Object.keys(GLYPH_PATHS) satisfies string[]
export const PK_GLYPH_NAMES: readonly GlyphIconName[] =
  PK_GLYPH_NAMES_INTERNAL as readonly GlyphIconName[]
