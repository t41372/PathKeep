/**
 * Barrel for paper-redesign shell components.
 *
 * Why this file exists:
 * - Consumers (src/app/shell.tsx, tests) import a coherent set of pieces from
 *   a single path. Keeping the barrel intentional avoids transitive churn when
 *   a single component moves.
 */

export { PKBrandMark } from './pk-brand-mark'
export type { PKBrandMarkProps } from './pk-brand-mark'
export { PKGlyph, PK_GLYPH_NAMES } from './pk-glyph'
export type { GlyphIconName, PKGlyphProps } from './pk-glyph'
export { PKSidebar } from './pk-sidebar'
export type { PKSidebarProps } from './pk-sidebar'
export { PKTopbar } from './pk-topbar'
export type { PKTopbarProps } from './pk-topbar'
export { PKStatusBar } from './pk-status-bar'
export type { PKStatusBarProps, PKStatusBarSource } from './pk-status-bar'
export { PKSearchPalette } from './pk-search-palette'
export type { PKSearchPaletteProps, PaletteResult } from './pk-search-palette'
