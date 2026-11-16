/**
 * Renders hero paintings to files.
 *
 *   bun site/hero-lab/render.ts [preset[,preset…]|all] [--seed N] [--size WxH]
 *       [--preview 2000,1440] [--crop name:x,y,w,h] [--png] [--webp 0.9]
 *
 * Bundles `src/main.ts` with Bun, opens it in headless Chrome (the installed
 * Chrome renders on the GPU; Playwright's bundled shell falls back to
 * software GL, which works but is slow), and writes each file to
 * `site/hero-lab/out/<preset>/<preset>-s<seed>-<W>x<H>-<file>`.
 */

import { chromium } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import type { PaintRequest, PaintResult } from './src/main'

const here = import.meta.dir
const argv = process.argv.slice(2)
const flag = (name: string) => {
  const i = argv.indexOf(`--${name}`)
  return i >= 0 ? argv[i + 1] : undefined
}
const flags = (name: string) =>
  argv.flatMap((a, i) => (a === `--${name}` ? [argv[i + 1]] : []))

const [width, height] = (flag('size') ?? '3840x2400').split('x').map(Number)
const seeds = (flag('seed') ?? '1').split(',').map(Number)
const previews = (flag('preview') ?? '2000')
  .split(',')
  .filter(Boolean)
  .map(Number)
const crops = flags('crop').map((spec) => {
  const [name, rect] = spec.split(':')
  const [x, y, w, h] = rect.split(',').map(Number)
  return { name, x, y, w, h }
})
const webp = flag('webp')

const bundle = await Bun.build({
  entrypoints: [join(here, 'src/main.ts')],
  target: 'browser',
  format: 'esm',
  loader: { '.glsl': 'text', '.frag': 'text', '.vert': 'text' },
})
if (!bundle.success) {
  for (const m of bundle.logs) console.error(m)
  process.exit(1)
}
const code = await bundle.outputs[0].text()

const browser = await chromium.launch({
  channel: process.env.HERO_CHANNEL ?? 'chrome',
  headless: true,
})
const page = await browser.newPage()
page.on('console', (m) => console.log(`[page] ${m.text()}`))
page.on('pageerror', (e) => console.error(`[page error] ${e.message}`))
await page.setContent('<!doctype html><html><body></body></html>')
await page.addScriptTag({ content: code, type: 'module' })
await page.waitForFunction(() => 'heroPaint' in window)
const names = await page.evaluate(
  () => (window as unknown as { heroPresets?: string[] }).heroPresets ?? [],
)

const wanted =
  argv[0] && !argv[0].startsWith('--') ? argv[0].split(',') : ['all']
const ids = wanted.includes('all') ? names : wanted

try {
  for (const preset of ids) {
    for (const seed of seeds) {
      const req: PaintRequest = {
        preset,
        seed,
        width,
        height,
        previews,
        crops,
        png: argv.includes('--png'),
      }
      if (webp) req.webpQuality = Number(webp)
      const res = (await page.evaluate(
        (r) => window.heroPaint(r),
        req,
      )) as PaintResult
      const dir = join(here, 'out', preset)
      await mkdir(dir, { recursive: true })
      const stem = `${preset}-s${seed}-${width}x${height}`
      for (const f of res.files) {
        const bytes = Buffer.from(f.base64, 'base64')
        await writeFile(join(dir, `${stem}-${f.name}`), bytes)
        console.log(
          `  ${stem}-${f.name}  ${(bytes.length / 1024).toFixed(0)} KB`,
        )
      }
      console.log(`${stem}: ${res.ms.toFixed(0)} ms  (${res.log.join(', ')})`)
    }
  }
} finally {
  await browser.close()
}
