/**
 * Contact sheet: lays rendered previews out in a labelled grid and screenshots it.
 *
 *   bun site/hero-lab/sheet.ts <out.jpg> <columns> <image> [image…]
 */

import { chromium } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { basename } from 'node:path'

const [out, colsArg, ...images] = process.argv.slice(2)
const cols = Number(colsArg)
const cellW = Math.floor(2400 / cols)
const cells = await Promise.all(
  images.map(async (path) => {
    const data = (await readFile(path)).toString('base64')
    return `<figure><img src="data:image/jpeg;base64,${data}"><figcaption>${basename(path)}</figcaption></figure>`
  }),
)
const html = `<!doctype html><html><head><style>
  body { margin: 0; background: #1b1a19; font: 13px ui-monospace, monospace; color: #ddd; }
  main { display: grid; grid-template-columns: repeat(${cols}, ${cellW}px); gap: 0; }
  figure { margin: 0; position: relative; }
  img { width: 100%; display: block; }
  figcaption { position: absolute; left: 6px; top: 6px; background: rgb(0 0 0 / 0.55); padding: 2px 6px; }
</style></head><body><main>${cells.join('')}</main></body></html>`
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({
  viewport: { width: cellW * cols, height: 400 },
})
await page.setContent(html)
await page.screenshot({ path: out, fullPage: true, type: 'jpeg', quality: 88 })
await browser.close()
console.log(out)
