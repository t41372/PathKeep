/**
 * The paintings the page sets its content on, as `hero-lab/export-panels.ts`
 * writes them to `public/paintings/<name>-<width>.webp`.
 * Not responsible for: where each sits, which is the stylesheet's.
 */

export interface Painting {
  name: string
  /** File widths, smallest first. */
  widths: number[]
  /** Height of the widest file, so the browser can reserve the space. */
  height: number
}

export const paintings = {
  /** The opening card, hero and app window on one tall painting. */
  intro: { name: 'intro', widths: [1920, 2560, 3310], height: 3800 },
  /** A phone's opening card: taller, with more of the light band behind the words. */
  heroPhone: { name: 'hero-phone', widths: [720], height: 1440 },
  browsing: { name: 'browsing', widths: [680, 1320], height: 1127 },
  agent: { name: 'agent', widths: [680, 1320], height: 1127 },
  privacy: { name: 'privacy', widths: [1376, 2064, 2752], height: 1600 },
  demo: { name: 'demo', widths: [1376, 2064, 2752], height: 1560 },
  download: { name: 'download', widths: [1376, 2064, 2752], height: 1279 },
} satisfies Record<string, Painting>

/** The strip of paint the retention chart draws PathKeep's row with. */
export const pathStrip = {
  src: '/paintings/path-1920.webp',
  width: 1920,
  height: 24,
}

/** The wall beside the page's column, as `--wall-width` in site.css. */
const wall = 'clamp(clamp(12px, 2.2vw, 32px), 9vw - 66px, 64px)'

/**
 * A panel spans the page's column (`--content` beside `--wall-width` in
 * site.css): the window less the walls, held at 1376px from 1504px wide,
 * then growing with the screen to 1680px.
 */
export const panelSizes = `(min-width: 2546px) 1680px, (min-width: 2085px) 66vw, (min-width: 1504px) 1376px, calc(100vw - 2 * ${wall})`

/** Two panels side by side, 18px apart; one per row up to 1100px wide. */
export const halfPanelSizes = `(min-width: 2546px) 831px, (min-width: 2085px) calc(33vw - 9px), (min-width: 1504px) 679px, (max-width: 1100px) calc(100vw - 2 * ${wall}), calc(50vw - ${wall} - 9px)`

/** The opening card's painting is set taller than the card (see site.css). */
export const introSizes = 'max(100vw, 1920px)'
