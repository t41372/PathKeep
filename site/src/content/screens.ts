/**
 * The app pictures shown inside the floating windows until the live demo
 * replaces them, one set per language, each in the language of its page
 * (`hero-lab/capture-screens.ts` takes them).
 */

import { langs, type Feature, type Lang } from './copy'

export interface Screen {
  id: 'browsing' | 'agent' | 'home'
  /** The picture's name inside each language's folder under `/screens/`. */
  file: string
  alt: Record<Lang, string>
}

/** Where a screen's picture for a language lives. */
export const screenSrc = (screen: { file: string }, lang: Lang) =>
  `/screens/${langs.find((l) => l.lang === lang)!.slug}/${screen.file}`

/**
 * The opening card shows what people keep PathKeep for, each space's
 * records; the demo's window opens on the home page, as the app does.
 */
export const screens: Record<Screen['id'], Screen> = {
  browsing: {
    id: 'browsing',
    file: 'browsing-history.webp',
    alt: {
      en: 'PathKeep’s browsing history: every visit, by day',
      'zh-TW': 'PathKeep 的瀏覽紀錄：每一次造訪依日期排好',
      'zh-CN': 'PathKeep 的浏览记录：每一次访问按日期排好',
    },
  },
  agent: {
    id: 'agent',
    file: 'agent-chats.webp',
    alt: {
      en: 'The list of agent conversations',
      'zh-TW': 'Agent 對話的紀錄清單',
      'zh-CN': 'Agent 对话的记录列表',
    },
  },
  home: {
    id: 'home',
    file: 'browsing-home.webp',
    alt: {
      en: 'PathKeep’s browsing history home page',
      'zh-TW': 'PathKeep 瀏覽紀錄的首頁',
      'zh-CN': 'PathKeep 浏览记录的首页',
    },
  },
}

/** Part of a screen, cut around one feature. */
export interface FeatureShot {
  file: string
  /** The file's size in pixels (twice the screen's). */
  width: number
  height: number
  alt: Record<Lang, string>
}

/**
 * The pictures on the feature wall (`Features.astro`), one per language:
 * `hero-lab/capture-features.ts` cuts the browsing ones from the demo, so
 * their words are the app's own, and the Agent one from the screens above.
 */
export const featureShots: Partial<Record<Feature['id'], FeatureShot>> = {
  search: {
    file: 'feature-search.webp',
    width: 2480,
    height: 900,
    alt: {
      en: 'Searching the history with the pattern /tokio|smol/',
      'zh-TW': '用正規表示式 /tokio|smol/ 搜尋紀錄',
      'zh-CN': '用正则表达式 /tokio|smol/ 搜索记录',
    },
  },
  history: {
    file: 'feature-history.webp',
    width: 1520,
    height: 1100,
    alt: {
      en: 'Visits by day, grouped by each stretch of browsing',
      'zh-TW': '造訪依日期、依同一段瀏覽排好',
      'zh-CN': '访问按日期、按同一段浏览排好',
    },
  },
  insights: {
    file: 'feature-insights.webp',
    width: 2040,
    height: 1100,
    alt: {
      en: 'Insights: the most visited sites and the months they fill',
      'zh-TW': '洞察：最常去的網站，以及每個月瀏覽了多少',
      'zh-CN': '洞察：最常去的网站，以及每个月浏览了多少',
    },
  },
  agent: {
    file: 'feature-agent.webp',
    width: 2440,
    height: 1070,
    alt: {
      en: 'Agent insights: hours worked, tokens, cost and every day of the year',
      'zh-TW': 'Agent 洞察：工作時間、token、費用，和一年裡的每一天',
      'zh-CN': 'Agent 洞察：工作时间、token、费用，和一年里的每一天',
    },
  },
}
