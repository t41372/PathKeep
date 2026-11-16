/**
 * Photographs the app's screens for the windows on the page, once per
 * language. The screens are drawn in Traditional Chinese; for English and
 * Simplified Chinese every piece of text in the picture is swapped for its
 * entry in `screens-text.ts` first. A screen with a piece of Chinese that
 * has no entry, or an English screen still showing any Chinese afterwards,
 * is not written, and the run ends in failure.
 *
 *   bun site/hero-lab/capture-screens.ts <url> [--hide <selector>]
 *
 * <url> serves the screens, routed by hash (#/browsing/home and so on), and
 * may carry a query. --hide names elements to leave out of the pictures.
 * Writes `hero-lab/screens-in/<slug>/<screen>.png`: 1360×860 at twice the
 * pixels, the window's own background left transparent. Then run
 * `export-screens.ts` for the WebP files the page loads.
 */
import { chromium } from '@playwright/test'
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { langs } from '../src/content/copy'
import { inPhrase, words } from './screens-text'

const argv = process.argv.slice(2)
const base = argv[0]
const hide = argv.includes('--hide') ? argv[argv.indexOf('--hide') + 1] : ''
if (!base || base.startsWith('--')) {
  console.log('usage: capture-screens.ts <url> [--hide <selector>]')
  process.exit(1)
}

const screens = [
  ['browsing-home', '#/browsing/home'],
  ['browsing-history', '#/browsing/history'],
  ['agent-insights', '#/agent/insights'],
  ['agent-chats', '#/agent/chats'],
] as const

/**
 * Swaps the Chinese in the visible picture for `lang`, or, when a piece has
 * no entry, changes nothing and returns what is missing. Runs in the page.
 */
const swap = ({
  lang,
  words,
  inPhrase,
}: {
  lang: 'en' | 'zh-CN'
  words: Record<string, Record<string, string>>
  inPhrase: Record<string, Record<string, Record<string, string>>>
}) => {
  const cjk = /[　-〿㐀-鿿豈-﫿＀-￯]/
  const shown = (el: Element) => {
    const box = el.getBoundingClientRect()
    const style = getComputedStyle(el)
    return (
      style.display !== 'none' &&
      style.visibility !== 'hidden' &&
      box.width > 0 &&
      box.height > 0 &&
      box.bottom > 0 &&
      box.top < innerHeight &&
      box.right > 0 &&
      box.left < innerWidth
    )
  }
  // The phrase a piece belongs to: its nearest ancestor that is not inline.
  const phraseOf = (el: Element) => {
    let block = el
    while (
      block.parentElement &&
      ['inline', 'inline-block', 'inline-flex'].includes(
        getComputedStyle(block).display,
      ) &&
      (block.textContent ?? '').length < 60
    )
      block = block.parentElement
    return (block.textContent ?? '').replace(/\s+/g, ' ').trim()
  }
  const pieces = () => {
    const found: { node: Text; text: string; el: Element }[] = []
    const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT)
    for (let n = walk.nextNode(); n; n = walk.nextNode()) {
      const text = n.textContent ?? ''
      const el = n.parentElement
      if (el && cjk.test(text) && shown(el))
        found.push({ node: n as Text, text, el })
    }
    return found
  }

  const missing: string[] = []
  const edits: (() => void)[] = []
  for (const { node, text, el } of pieces()) {
    const key = text.trim()
    const entry = inPhrase[phraseOf(el)]?.[key] ?? words[key]
    if (!entry) missing.push(key)
    else {
      const [lead, trail] = [text.match(/^\s*/)![0], text.match(/\s*$/)![0]]
      edits.push(() => (node.textContent = lead + entry[lang] + trail))
    }
  }
  for (const input of document.querySelectorAll<HTMLInputElement>(
    'input[placeholder], textarea[placeholder]',
  )) {
    const key = input.placeholder.trim()
    if (!cjk.test(key) || !shown(input)) continue
    const entry = words[key]
    if (!entry) missing.push(`placeholder ${key}`)
    else edits.push(() => (input.placeholder = entry[lang]))
  }
  if (missing.length) return missing
  for (const edit of edits) edit()
  document.documentElement.lang = lang
  if (lang === 'en') return pieces().map((p) => `left in Chinese: ${p.text}`)
  return []
}

const out = join(import.meta.dir, 'screens-in')
const browser = await chromium.launch({ channel: 'chrome', headless: true })
let failed = false
for (const l of langs) {
  await mkdir(join(out, l.slug), { recursive: true })
  for (const [name, hash] of screens) {
    // A fresh page each time: moving between hash routes keeps the shell's
    // text from the last picture, already swapped.
    const page = await browser.newPage({
      viewport: { width: 1360, height: 860 },
      deviceScaleFactor: 2,
    })
    await page.goto(base + hash, { waitUntil: 'networkidle' })
    await page.addStyleTag({
      content: `${hide ? `${hide} { display: none !important }` : ''} html, body { background: transparent !important }`,
    })
    await page.waitForTimeout(1200)
    if (l.lang !== 'zh-TW') {
      const problems = await page.evaluate(swap, {
        lang: l.lang,
        words: words as unknown as Record<string, Record<string, string>>,
        inPhrase: inPhrase as unknown as Record<
          string,
          Record<string, Record<string, string>>
        >,
      })
      if (problems.length) {
        failed = true
        console.log(`${l.slug}/${name}: not written`)
        for (const p of new Set(problems)) console.log(`  ${p}`)
        await page.close()
        continue
      }
      await page.waitForTimeout(300)
    }
    await page.screenshot({
      path: join(out, l.slug, `${name}.png`),
      omitBackground: true,
    })
    console.log(`screens-in/${l.slug}/${name}.png`)
    await page.close()
  }
}
await browser.close()
process.exit(failed ? 1 : 0)
