/**
 * The shape of the landing page's words, and the languages they ship in.
 *
 * Responsibilities: what each section says, as types every language must
 * fill, and where each language lives. The words themselves are in
 * `copy-en.ts`, `copy-zh-tw.ts` and `copy-zh-cn.ts`. `|` in a string marks
 * where it may wrap (see `Phrases.astro`); strip it for plain-text uses
 * such as `<title>`.
 * Not responsible for: layout.
 */

import { en } from './copy-en'
import { zhCN } from './copy-zh-cn'
import { zhTW } from './copy-zh-tw'

export type Lang = 'en' | 'zh-TW' | 'zh-CN'

export interface RetentionRow {
  source: string
  keeps: string
  /** Days kept; null for "as long as you want". */
  days: number | null
}

export interface Space {
  id: 'browsing' | 'agent'
  name: string
  covers: string
  pages: string
}

/** Something PathKeep does with what it keeps; `Features.astro` pictures some. */
export interface Feature extends Item {
  id: 'search' | 'history' | 'insights' | 'agent' | 'ask' | 'notes'
}

export interface Item {
  title: string
  text: string
}

export interface Platform {
  name: string
  detail: string
  action: string
  /** Not released yet: the action follows the project instead of downloading. */
  soon?: boolean
}

export interface Copy {
  htmlLang: string
  description: string
  skip: string
  nav: {
    label: string
    why: string
    keeps: string
    privacy: string
    faq: string
    github: string
    download: string
  }
  hero: {
    title: string
    lede: string
    primary: string
    secondary: string
    demo: string
    note: string
  }
  shot: { tablist: string; tabs: Record<Space['id'], string>; caption: string }
  why: {
    title: string
    body: string
    /** The axis: 90 days, a year, and its far end past the break. */
    axis: [string, string, string]
    rows: RetentionRow[]
    source: string
    closing: string
  }
  keeps: { title: string; body: string; spaces: Space[]; more: string }
  find: { title: string; body: string; items: Feature[] }
  privacy: { title: string; body: string; points: string[]; network: string }
  demo: {
    title: string
    body: string
    /** The link that opens the demo full-size in a tab of its own. */
    newTab: string
    /** Said instead of the demo on screens too small for the app. */
    narrow: string
    /** The demo frame's name, for screen readers. */
    frame: string
    /** Over the loaded demo until it is clicked, so scrolling passes it by. */
    start: string
  }
  faq: { title: string; items: Item[] }
  download: {
    title: string
    body: string
    platforms: Platform[]
    note: string
    source: string
  }
  footer: {
    license: string
    links: { label: string; github: string; releases: string; readme: string }
  }
  /** The page for an address that leads nowhere. */
  notFound: { title: string; body: string; home: string }
}

export const copy: Record<Lang, Copy> = { en, 'zh-TW': zhTW, 'zh-CN': zhCN }

/**
 * Where each language lives, how its link reads in the other two, and the
 * name its files go by (the share picture, `public/social/<slug>.jpg`).
 */
export const langs: {
  lang: Lang
  path: string
  label: string
  slug: string
}[] = [
  { lang: 'en', path: '/', label: 'EN', slug: 'en' },
  { lang: 'zh-TW', path: '/zh-tw/', label: '繁中', slug: 'zh-tw' },
  { lang: 'zh-CN', path: '/zh-cn/', label: '简中', slug: 'zh-cn' },
]

/** A product named with its mark, `{key:Name}` (see `brands.ts`). */
export const markToken = /\{(\w+):([^}]+)\}/g

/** Copy as plain text: no phrase breaks, products by name alone. */
export const plain = (text: string) =>
  text.replaceAll('|', '').replace(markToken, '$2')
