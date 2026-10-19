/**
 * The README's "how it works" diagram, drawn as SVG in each README language
 * and in GitHub's light and dark themes (the README picks one with
 * <picture>). One layout, so the four files can't drift apart.
 *
 *   node scripts/readme-diagram.mjs   (writes docs/screenshots/<language>/how-it-works-<theme>.svg)
 *
 * Only system fonts: GitHub shows an SVG as an image, which loads none.
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
)

const WORDS = {
  en: {
    browsers: 'Your browsers',
    takeout: 'Google Takeout',
    takeoutNote: 'import',
    copy: 'read-only copy',
    archive: 'Your archive',
    where: 'On this computer',
    facts: [
      'Adds only what is new',
      'Keeps each visit once',
      'Encrypted if you choose',
      'Every backup and import on record',
    ],
    uses: [
      ['History', 'Every visit, searched in full'],
      ['Insights', 'Worked out on your computer'],
      ['Ask · semantic search', 'Optional, with a model you choose'],
    ],
  },
  'zh-CN': {
    browsers: '你的浏览器',
    takeout: 'Google Takeout',
    takeoutNote: '导入',
    copy: '只读副本',
    archive: '你的存档',
    where: '在这台电脑上',
    facts: [
      '每次只加新的',
      '同一次访问只存一次',
      '可以选择加密',
      '每次备份、导入都有记录',
    ],
    uses: [
      ['历史', '每一次访问，全文可搜'],
      ['洞察', '在这台电脑上算出来'],
      ['助理 · 语义搜索', '可选，用你选的模型'],
    ],
  },
}

const BROWSERS = [
  ['Google Chrome', '#4285f4'],
  ['Microsoft Edge', '#0f9d8f'],
  ['Firefox', '#ff7139'],
  ['Safari', '#1e90ff'],
  ['ChatGPT Atlas · Comet', '#8b8b93'],
]

// The app's tokens (src/index.css), as hex.
const THEMES = {
  light: {
    card: '#ffffff',
    border: '#e4e4ea',
    text: '#18181b',
    muted: '#6b6b74',
    brand: '#ec7a32',
    brandSoft: '#fdf0e6',
    arrow: '#b4b4bc',
  },
  dark: {
    card: '#1f1f2b',
    border: '#34343f',
    text: '#f4f4f5',
    muted: '#a1a1aa',
    brand: '#f4964c',
    brandSoft: '#2d2420',
    arrow: '#5c5c66',
  },
}

const FONT =
  "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', 'Noto Sans CJK SC', sans-serif"

function diagram(words, colors) {
  const c = colors
  const text = (
    x,
    y,
    value,
    { size = 14, weight = 400, fill = c.text, anchor = 'start' } = {},
  ) =>
    `<text x="${x}" y="${y}" font-size="${size}" font-weight="${weight}" fill="${fill}" text-anchor="${anchor}">${value}</text>`
  const box = (
    x,
    y,
    w,
    h,
    { fill = c.card, stroke = c.border, dash = '' } = {},
  ) =>
    `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="12" fill="${fill}" stroke="${stroke}"${dash ? ` stroke-dasharray="${dash}"` : ''}/>`
  const arrow = (x1, x2, y) =>
    `<line x1="${x1}" y1="${y}" x2="${x2 - 8}" y2="${y}" stroke="${c.arrow}" stroke-width="2"/>` +
    `<path d="M${x2 - 10} ${y - 6} L${x2} ${y} L${x2 - 10} ${y + 6}" fill="none" stroke="${c.arrow}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>`

  const parts = []
  // Browsers.
  parts.push(
    text(20, 34, words.browsers, { size: 13, weight: 600, fill: c.muted }),
  )
  BROWSERS.forEach(([name, dot], index) => {
    const y = 52 + index * 50
    parts.push(box(20, y, 250, 40))
    parts.push(`<circle cx="42" cy="${y + 20}" r="6" fill="${dot}"/>`)
    parts.push(text(58, y + 25, name))
  })
  const takeoutY = 52 + BROWSERS.length * 50
  parts.push(box(20, takeoutY, 250, 40, { dash: '5 4' }))
  parts.push(`<circle cx="42" cy="${takeoutY + 20}" r="6" fill="#ea4335"/>`)
  parts.push(text(58, takeoutY + 25, words.takeout))
  parts.push(
    text(254, takeoutY + 25, words.takeoutNote, {
      size: 12,
      fill: c.muted,
      anchor: 'end',
    }),
  )

  // Archive.
  const mid = 52 + (takeoutY + 40 - 52) / 2
  parts.push(arrow(282, 380, mid))
  parts.push(
    text(331, mid + 24, words.copy, {
      size: 12,
      fill: c.muted,
      anchor: 'middle',
    }),
  )
  parts.push(box(392, 92, 296, 222, { fill: c.brandSoft, stroke: c.brand }))
  parts.push(text(416, 132, words.archive, { size: 20, weight: 700 }))
  parts.push(text(416, 156, words.where, { size: 13, fill: c.muted }))
  words.facts.forEach((fact, index) => {
    const y = 196 + index * 30
    parts.push(`<circle cx="422" cy="${y - 5}" r="3.5" fill="${c.brand}"/>`)
    parts.push(text(436, y, fact))
  })

  // What it's used for.
  parts.push(arrow(700, 760, mid))
  words.uses.forEach(([title, note], index) => {
    const y = 72 + index * 92
    parts.push(box(772, y, 256, 76, index === 2 ? { dash: '5 4' } : {}))
    parts.push(text(794, y + 32, title, { size: 16, weight: 600 }))
    parts.push(text(794, y + 56, note, { size: 13, fill: c.muted }))
  })

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1040 ${takeoutY + 60}" width="1040" height="${takeoutY + 60}" font-family="${FONT}">
${parts.join('\n')}
</svg>
`
}

for (const [language, words] of Object.entries(WORDS)) {
  const dir = path.join(repoRoot, 'docs', 'screenshots', language)
  mkdirSync(dir, { recursive: true })
  for (const [theme, colors] of Object.entries(THEMES)) {
    const file = path.join(dir, `how-it-works-${theme}.svg`)
    writeFileSync(file, diagram(words, colors))
    console.log(path.relative(repoRoot, file))
  }
}
