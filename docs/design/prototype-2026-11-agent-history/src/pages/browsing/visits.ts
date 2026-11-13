/** A few invented visits so the browsing pages look lived-in. */
import { NOW, DAY } from '~/data/fixture'
import { mulberry32, pick } from '~/data/rng'

export interface Visit {
  id: number
  at: number
  title: string
  domain: string
  path: string
  browser: 'Chrome' | 'Safari' | 'Arc'
  starred: boolean
}

const pages: [string, string, string][] = [
  ['github.com', '/kestrel-labs/orbit-notes/pull/214', 'feat: stream search results · Pull Request #214'],
  ['docs.rs', '/tantivy/latest/tantivy/query/struct.PhraseQuery.html', 'PhraseQuery in tantivy::query - Rust'],
  ['developer.mozilla.org', '/zh-TW/docs/Web/API/CSS_Custom_Highlight_API', 'CSS Custom Highlight API - Web API | MDN'],
  ['news.ycombinator.com', '/item?id=41822903', 'Show HN: A local-first archive for your agent transcripts'],
  ['www.youtube.com', '/watch?v=7Pq-S557XQU', '城市散步：台南老街一日'],
  ['zh.wikipedia.org', '/wiki/中文分詞', '中文分詞 - 維基百科，自由的百科全書'],
  ['tailwindcss.com', '/docs/detecting-classes-in-source-files', 'Detecting classes in source files - Tailwind CSS'],
  ['react.dev', '/reference/react/useDeferredValue', 'useDeferredValue – React'],
  ['stackoverflow.com', '/questions/7451716', 'How to make SQLite FTS match adjacent CJK characters'],
  ['tauri.app', '/develop/calling-rust/', 'Calling Rust from the Frontend | Tauri'],
  ['www.google.com', '/search?q=sqlcipher+rekey+progress', 'sqlcipher rekey progress - Google 搜尋'],
  ['mail.google.com', '/mail/u/0/#inbox', '收件匣 (12)'],
  ['www.notion.so', '/Reading-list-2026', '2026 閱讀清單'],
  ['www.thsrc.com.tw', '/ArticleContent/a3b630bb', '時刻表與票價 | 台灣高鐵'],
  ['arxiv.org', '/abs/2510.01234', 'Long-Context Retrieval for Agent Memory'],
]

export const visits: Visit[] = (() => {
  const rng = mulberry32(42)
  const out: Visit[] = []
  let t = NOW - 4 * 60_000
  let id = 0
  while (t > NOW - 3 * DAY) {
    const [domain, path, title] = pick(rng, pages)
    out.push({ id: ++id, at: t, title, domain, path, browser: pick(rng, ['Chrome', 'Chrome', 'Safari', 'Arc'] as const), starred: rng() < 0.06 })
    t -= rng() < 0.15 ? 3 * 3600_000 * rng() : 60_000 + rng() * 25 * 60_000
  }
  return out
})()
