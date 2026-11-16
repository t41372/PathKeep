/**
 * Splits a tall full-page screenshot into N stacked parts for review.
 *
 *   bun site/hero-lab/split.ts <image.jpg> <parts>
 */
import { chromium } from '@playwright/test'
import { readFile } from 'node:fs/promises'

const [file, partsArg] = process.argv.slice(2)
const parts = Number(partsArg ?? 3)
const src = (await readFile(file)).toString('base64')
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
await page.setContent(
  `<body style="margin:0;overflow:hidden"><img id="i" src="data:image/jpeg;base64,${src}"></body>`,
)
const { w, h } = await page.evaluate(() => {
  const i = document.getElementById('i') as HTMLImageElement
  return { w: i.naturalWidth, h: i.naturalHeight }
})
const step = Math.ceil(h / parts)
for (let k = 0; k < parts; k++) {
  await page.setViewportSize({ width: w, height: Math.min(step, h - k * step) })
  await page.evaluate((y) => {
    ;(document.getElementById('i') as HTMLImageElement).style.transform =
      `translateY(-${y}px)`
  }, k * step)
  await page.screenshot({
    path: file.replace(/\.jpg$/, `-part${k + 1}.jpg`),
    type: 'jpeg',
    quality: 80,
  })
}
await browser.close()
console.log(`${h}px in ${parts} parts`)
