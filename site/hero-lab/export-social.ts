/**
 * Draws the picture a link to the page shows when it is shared (1200×630,
 * one per language) into `site/public/social/<slug>.jpg`: the top of the
 * opening card's painting, the name, the promise and the version line, set
 * in the page's own fonts. Needs the preview server on 4391; the page's
 * `<head>` points at the pictures once `site` is set.
 *
 *   bun site/hero-lab/export-social.ts
 */
import { chromium } from '@playwright/test'
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { copy, langs } from '../src/content/copy'

const out = join(import.meta.dir, '..', 'public', 'social')
await mkdir(out, { recursive: true })

const browser = await chromium.launch({ channel: 'chrome', headless: true })
for (const l of langs) {
  const c = copy[l.lang]
  const page = await browser.newPage({
    viewport: { width: 1200, height: 630 },
    deviceScaleFactor: 1,
  })
  await page.goto(`http://127.0.0.1:4391${l.path}`, {
    waitUntil: 'networkidle',
  })
  await page.evaluate(
    ({ title, note }) => {
      // One line per phrase, as the opening card sets the promise.
      const phrases = title
        .replace(/\{\w+:([^}]+)\}/g, '$1')
        .split('|')
        .map((p) => `<span style="display:block">${p.trim()}</span>`)
        .join('')
      document.body.innerHTML = `
        <div class="share">
          <img class="share__paint" src="/paintings/intro-1920.webp" alt="">
          <div class="share__brand"><img src="/brand/mark.svg" alt="" width="34" height="34">PathKeep</div>
          <p class="share__title">${phrases}</p>
          <p class="share__note">${note}</p>
        </div>
        <style>
          html, body { margin: 0; background: #131211; overflow: hidden; }
          .share { position: relative; width: 1200px; height: 630px; overflow: hidden; color: var(--ink); }
          .share__paint { position: absolute; inset: 0; width: 100%; height: 1300px; object-fit: cover; object-position: 50% 0; }
          .share__brand { position: absolute; left: 72px; top: 60px; display: flex; align-items: center; gap: 12px; font: 600 24px/1 var(--sans); letter-spacing: -0.01em; }
          .share__title { position: absolute; left: 72px; right: 72px; top: 150px; margin: 0; font: 300 112px / var(--lh-hero) var(--serif); letter-spacing: var(--track); }
          .share__note { position: absolute; left: 72px; bottom: 60px; margin: 0; font: 18px/1.6 var(--mono); }
        </style>`
    },
    { title: c.hero.title, note: c.hero.note },
  )
  await page.waitForLoadState('networkidle')
  await page.evaluate(async () => {
    await document.fonts.ready
    await Promise.all(
      Array.from(document.images).map((i) => i.decode().catch(() => undefined)),
    )
  })
  await page.waitForTimeout(300)
  await page.screenshot({
    path: join(out, `${l.slug}.jpg`),
    type: 'jpeg',
    quality: 86,
  })
  console.log(`social/${l.slug}.jpg`)
  await page.close()
}
await browser.close()
