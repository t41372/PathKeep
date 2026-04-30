/**
 * This module holds reusable shell-level UI building blocks that many routes lean on before they need a route-specific component.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `Glyph`
 * - `Surface`
 * - `FieldBlock`
 * - `ToggleRow`
 * - `DataRow`
 * - `InfoStat`
 * - `StatusTag`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import type { ReactNode } from 'react'

// ---------------------------------------------------------------------------
// Glyph — local SVG icon wrapper
// ---------------------------------------------------------------------------

const glyphStrokeProps = {
  fill: 'none',
  stroke: 'currentColor',
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
  strokeWidth: 1.8,
} as const

function renderBarChartGlyph() {
  return (
    <>
      <path d="M4.5 19.5h15" {...glyphStrokeProps} />
      <path d="M7 18v-5" {...glyphStrokeProps} />
      <path d="M12 18V7" {...glyphStrokeProps} />
      <path d="M17 18v-9" {...glyphStrokeProps} />
    </>
  )
}

function renderBookGlyph() {
  return (
    <>
      <path
        d="M6.5 5.5h4.5A3 3 0 0 1 14 8.5v10H9.5A3 3 0 0 0 6.5 21z"
        {...glyphStrokeProps}
      />
      <path
        d="M17.5 5.5H13A3 3 0 0 0 10 8.5v10h4.5A3 3 0 0 1 17.5 21z"
        {...glyphStrokeProps}
      />
      <path d="M10 9.5h4" {...glyphStrokeProps} />
    </>
  )
}

function renderGearGlyph() {
  return (
    <>
      <path
        d="M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.2a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.8a2 2 0 0 1-1 1.7l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.2a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.2a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.7v-.8a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.2a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z"
        {...glyphStrokeProps}
      />
      <circle cx="12" cy="12" r="3" {...glyphStrokeProps} />
    </>
  )
}

function renderWrenchGlyph() {
  return (
    <path
      d="M14.7 6.3a3.7 3.7 0 0 0 5 5L10.2 20.8a2 2 0 0 1-2.8 0L5.2 18.6a2 2 0 0 1 0-2.8z"
      {...glyphStrokeProps}
    />
  )
}

function renderCheckGlyph() {
  return <path d="m5 12.5 4.2 4.2L19 7.5" {...glyphStrokeProps} />
}

function renderCloudUploadGlyph() {
  return (
    <>
      <path
        d="M7 18a4 4 0 1 1 .8-7.9A5.2 5.2 0 0 1 18 11a3.5 3.5 0 1 1 0 7H7z"
        {...glyphStrokeProps}
      />
      <path d="M12 15V9.5" {...glyphStrokeProps} />
      <path d="m9.5 11.8 2.5-2.5 2.5 2.5" {...glyphStrokeProps} />
    </>
  )
}

function renderCopyGlyph() {
  return (
    <>
      <rect height="11" rx="1.5" width="10" x="9" y="7" {...glyphStrokeProps} />
      <path d="M15 5H6a1 1 0 0 0-1 1v9" {...glyphStrokeProps} />
    </>
  )
}

function renderTrashGlyph() {
  return (
    <>
      <path d="M5 7h14" {...glyphStrokeProps} />
      <path
        d="M9.5 7V5.5A1.5 1.5 0 0 1 11 4h2a1.5 1.5 0 0 1 1.5 1.5V7"
        {...glyphStrokeProps}
      />
      <path d="m8 7 1 12h6l1-12" {...glyphStrokeProps} />
      <path d="M11 10.5v5" {...glyphStrokeProps} />
      <path d="M13 10.5v5" {...glyphStrokeProps} />
    </>
  )
}

function renderFolderGlyph() {
  return (
    <>
      <path
        d="M3.5 9.5a2 2 0 0 1 2-2H10l2 2h6.5a2 2 0 0 1 2 2l-1 6.5a2 2 0 0 1-2 1.5H6a2 2 0 0 1-2-1.5z"
        {...glyphStrokeProps}
      />
      <path d="M3.5 9.5V7A2 2 0 0 1 5.5 5H10l2 2h4" {...glyphStrokeProps} />
    </>
  )
}

function renderHistoryGlyph() {
  return (
    <>
      <path d="M3.5 5.5V10H8" {...glyphStrokeProps} />
      <path d="M4.3 10A8 8 0 1 0 8 5.3" {...glyphStrokeProps} />
      <path d="M12 8v4.2l2.8 1.8" {...glyphStrokeProps} />
    </>
  )
}

function renderGlobeGlyph() {
  return (
    <>
      <circle cx="12" cy="12" r="8" {...glyphStrokeProps} />
      <path d="M4 12h16" {...glyphStrokeProps} />
      <path
        d="M12 4c2.4 2.1 3.6 4.8 3.6 8s-1.2 5.9-3.6 8c-2.4-2.1-3.6-4.8-3.6-8S9.6 6.1 12 4Z"
        {...glyphStrokeProps}
      />
    </>
  )
}

function renderMemoryGlyph() {
  return (
    <>
      <rect height="8" rx="1.5" width="10" x="7" y="8" {...glyphStrokeProps} />
      <path d="M9.5 8V6" {...glyphStrokeProps} />
      <path d="M12 8V6" {...glyphStrokeProps} />
      <path d="M14.5 8V6" {...glyphStrokeProps} />
      <path d="M9.5 18v-2" {...glyphStrokeProps} />
      <path d="M12 18v-2" {...glyphStrokeProps} />
      <path d="M14.5 18v-2" {...glyphStrokeProps} />
      <path d="M7 10H5" {...glyphStrokeProps} />
      <path d="M7 14H5" {...glyphStrokeProps} />
      <path d="M19 10h-2" {...glyphStrokeProps} />
      <path d="M19 14h-2" {...glyphStrokeProps} />
    </>
  )
}

function renderPreviewGlyph() {
  return (
    <>
      <path
        d="M2.5 12s3.5-5.5 9.5-5.5S21.5 12 21.5 12s-3.5 5.5-9.5 5.5S2.5 12 2.5 12Z"
        {...glyphStrokeProps}
      />
      <circle cx="12" cy="12" r="2.5" {...glyphStrokeProps} />
    </>
  )
}

function renderSearchGlyph() {
  return (
    <>
      <circle cx="10.5" cy="10.5" r="4.5" {...glyphStrokeProps} />
      <path d="m14 14 5 5" {...glyphStrokeProps} />
    </>
  )
}

function renderShieldGlyph() {
  return (
    <path
      d="M12 3.5 19 6v5.5c0 4.2-2.7 8-7 9.7-4.3-1.7-7-5.5-7-9.7V6z"
      {...glyphStrokeProps}
    />
  )
}

function renderRobotGlyph() {
  return (
    <>
      <rect height="8" rx="2" width="10" x="7" y="8" {...glyphStrokeProps} />
      <path d="M12 8V5.5" {...glyphStrokeProps} />
      <path d="m10 17 1.2 1.5" {...glyphStrokeProps} />
      <path d="m14 17-1.2 1.5" {...glyphStrokeProps} />
      <path d="M8 11H6.5" {...glyphStrokeProps} />
      <path d="M17.5 11H16" {...glyphStrokeProps} />
      <circle cx="10" cy="11.5" fill="currentColor" r="0.8" />
      <circle cx="14" cy="11.5" fill="currentColor" r="0.8" />
      <path d="M10 14h4" {...glyphStrokeProps} />
    </>
  )
}

function renderSyncGlyph() {
  return (
    <>
      <path d="M20 7h-6a5 5 0 0 0-5 5v1" {...glyphStrokeProps} />
      <path d="m17 4 3 3-3 3" {...glyphStrokeProps} />
      <path d="M4 17h6a5 5 0 0 0 5-5v-1" {...glyphStrokeProps} />
      <path d="m7 20-3-3 3-3" {...glyphStrokeProps} />
    </>
  )
}

function renderSystemUpdateGlyph() {
  return (
    <>
      <path d="M12 5v9" {...glyphStrokeProps} />
      <path d="m8.5 10.5 3.5 3.5 3.5-3.5" {...glyphStrokeProps} />
      <path d="M5 18h14v2H5z" {...glyphStrokeProps} />
    </>
  )
}

function renderWarningGlyph() {
  return (
    <>
      <path d="M12 4.5 20 19H4z" {...glyphStrokeProps} />
      <path d="M12 9v4.5" {...glyphStrokeProps} />
      <circle cx="12" cy="16.5" fill="currentColor" r="0.8" />
    </>
  )
}

function renderDownloadGlyph() {
  return (
    <>
      <path d="M12 4v9.5" {...glyphStrokeProps} />
      <path d="m8 13 4 4 4-4" {...glyphStrokeProps} />
      <path d="M4 20h16" {...glyphStrokeProps} />
    </>
  )
}

function renderDatabaseGlyph() {
  return (
    <>
      <ellipse cx="12" cy="6" rx="7" ry="3" {...glyphStrokeProps} />
      <path d="M5 6v6c0 2 3 3 7 3s7-1 7-3V6" {...glyphStrokeProps} />
      <path d="M5 12v6c0 2 3 3 7 3s7-1 7-3v-6" {...glyphStrokeProps} />
    </>
  )
}

function renderArrowForwardGlyph() {
  return <path d="M5 12h14M13 6l6 6-6 6" {...glyphStrokeProps} />
}

function renderArrowBackGlyph() {
  return <path d="M19 12H5M11 18l-6-6 6-6" {...glyphStrokeProps} />
}

function renderNotificationsGlyph() {
  return (
    <>
      <path
        d="M18 10.5A6 6 0 0 0 6 10.5c0 4-1.8 5.2-2.5 6h17c-.7-.8-2.5-2-2.5-6Z"
        {...glyphStrokeProps}
      />
      <path d="M9.8 19a2.3 2.3 0 0 0 4.4 0" {...glyphStrokeProps} />
    </>
  )
}

const glyphVectors = {
  arrow_back: renderArrowBackGlyph,
  arrow_forward: renderArrowForwardGlyph,
  auto_stories: renderBookGlyph,
  bar_chart: renderBarChartGlyph,
  build: renderWrenchGlyph,
  check: renderCheckGlyph,
  cloud_upload: renderCloudUploadGlyph,
  content_copy: renderCopyGlyph,
  database: renderDatabaseGlyph,
  delete_sweep: renderTrashGlyph,
  download: renderDownloadGlyph,
  folder_open: renderFolderGlyph,
  history: renderHistoryGlyph,
  language: renderGlobeGlyph,
  memory: renderMemoryGlyph,
  notifications: renderNotificationsGlyph,
  preview: renderPreviewGlyph,
  public: renderGlobeGlyph,
  search: renderSearchGlyph,
  settings: renderGearGlyph,
  shield: renderShieldGlyph,
  smart_toy: renderRobotGlyph,
  sync: renderSyncGlyph,
  system_update: renderSystemUpdateGlyph,
  warning: renderWarningGlyph,
} as const

export type GlyphIconName = keyof typeof glyphVectors

/**
 * Explains how glyph works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function Glyph({
  icon,
  filled = false,
  label,
}: {
  icon: GlyphIconName
  filled?: boolean
  label?: string
}) {
  const renderVector = glyphVectors[icon]
  const decorative = !label

  return (
    <svg
      aria-hidden={decorative ? 'true' : undefined}
      aria-label={label}
      className={`glyph ${filled ? 'filled' : ''}`}
      focusable="false"
      role={decorative ? undefined : 'img'}
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
    >
      {renderVector()}
    </svg>
  )
}

// ---------------------------------------------------------------------------
// Surface — bordered card with optional header actions
// ---------------------------------------------------------------------------

/**
 * Explains how surface works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function Surface({
  eyebrow,
  title,
  icon,
  actions,
  children,
}: {
  eyebrow: string
  title: string
  icon: GlyphIconName
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="surface">
      <header className="surfaceHeader">
        <div className="surfaceTitle">
          <span className="surfaceIcon">
            <Glyph icon={icon} />
          </span>
          <div>
            <p className="sectionEyebrow">{eyebrow}</p>
            {title ? <h3>{title}</h3> : null}
          </div>
        </div>
        {actions ? <div className="surfaceActions">{actions}</div> : null}
      </header>
      <div className="surfaceBody">{children}</div>
    </section>
  )
}

// ---------------------------------------------------------------------------
// FieldBlock — label + form control wrapper
// ---------------------------------------------------------------------------

/**
 * Explains how field block works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function FieldBlock({
  label,
  control,
  children,
}: {
  label: string
  control?: ReactNode
  children?: ReactNode
}) {
  return (
    <label className="fieldBlock">
      <span className="fieldLabel">{label}</span>
      {control ?? children}
    </label>
  )
}

// ---------------------------------------------------------------------------
// ToggleRow — label + checkbox toggle
// ---------------------------------------------------------------------------

/**
 * Explains how toggle row works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function ToggleRow({
  checked,
  disabled = false,
  label,
  onChange,
}: {
  checked: boolean
  disabled?: boolean
  label: string
  onChange: (checked: boolean) => void
}) {
  return (
    <label className="toggleRow">
      <span>{label}</span>
      <input
        checked={checked}
        disabled={disabled}
        type="checkbox"
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  )
}

// ---------------------------------------------------------------------------
// DataRow — key/value display row
// ---------------------------------------------------------------------------

/**
 * Explains how data row works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function DataRow({
  label,
  value,
  children,
}: {
  label: string
  value?: ReactNode
  children?: ReactNode
}) {
  return (
    <div className="dataRow">
      <dt>{label}</dt>
      <dd>{value ?? children}</dd>
    </div>
  )
}

// ---------------------------------------------------------------------------
// InfoStat — compact stat chip with label + bold value
// ---------------------------------------------------------------------------

/**
 * Explains how info stat works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function InfoStat({
  label,
  value,
}: {
  label: string
  value: ReactNode
}) {
  return (
    <div className="infoStat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

// ---------------------------------------------------------------------------
// StatusTag — tinted inline badge
// ---------------------------------------------------------------------------

/**
 * Explains how status tag works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function StatusTag({
  ariaLabel,
  tone,
  children,
}: {
  ariaLabel?: string
  tone: 'info' | 'success' | 'danger' | 'neutral'
  children: ReactNode
}) {
  return (
    <span aria-label={ariaLabel} className={`statusTag ${tone}`}>
      {children}
    </span>
  )
}
