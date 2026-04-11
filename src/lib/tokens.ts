/**
 * This module mirrors the design-token contract into TypeScript-friendly helpers.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `themes`
 * - `tokens`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

export const themes = ['dark', 'light'] as const

export const tokens = {
  color: {
    bg: 'var(--bg)',
    bgElevated: 'var(--bg-elevated)',
    bgSurface: 'var(--bg-surface)',
    bgHover: 'var(--bg-hover)',
    border: 'var(--border)',
    borderActive: 'var(--border-active)',
    text: 'var(--text)',
    textMuted: 'var(--text-muted)',
    textBright: 'var(--text-bright)',
    accent: 'var(--accent)',
    accentDim: 'var(--accent-dim)',
    success: 'var(--success)',
    warning: 'var(--warning)',
    error: 'var(--error)',
    info: 'var(--info)',
  },
  font: {
    ui: 'var(--font-ui)',
    body: 'var(--font-body)',
    code: 'var(--font-code)',
  },
  space: {
    1: 'var(--space-1)',
    2: 'var(--space-2)',
    3: 'var(--space-3)',
    4: 'var(--space-4)',
    5: 'var(--space-5)',
    6: 'var(--space-6)',
    8: 'var(--space-8)',
    10: 'var(--space-10)',
    12: 'var(--space-12)',
  },
  density: {
    sidebar: 'var(--layout-sidebar-width)',
    topbar: 'var(--layout-topbar-height)',
    panelPadding: 'var(--density-panel-padding)',
    contentGap: 'var(--density-content-gap)',
  },
  radius: {
    base: 'var(--radius)',
  },
  transition: {
    default: 'var(--transition)',
  },
} as const
