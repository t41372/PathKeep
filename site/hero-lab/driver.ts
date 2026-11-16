/**
 * Opens the painter in headless Chrome so scripts can render presets.
 * The installed Chrome renders on the GPU; set HERO_CHANNEL=chromium to use
 * Playwright's own build instead (software GL, same picture, much slower).
 */

import { chromium, type Browser, type Page } from '@playwright/test'
import { join } from 'node:path'
import type { PaintRequest, PaintResult } from './src/main'

export interface Renderer {
  presets: string[]
  render: (req: PaintRequest) => Promise<PaintResult>
  toWebp: (pngBase64: string, quality: number) => Promise<string>
  close: () => Promise<void>
}

export async function openRenderer(): Promise<Renderer> {
  const bundle = await Bun.build({
    entrypoints: [join(import.meta.dir, 'src/main.ts')],
    target: 'browser',
    format: 'esm',
    loader: { '.glsl': 'text', '.frag': 'text', '.vert': 'text' },
  })
  if (!bundle.success) {
    for (const m of bundle.logs) console.error(m)
    throw new Error('bundle failed')
  }
  const code = await bundle.outputs[0].text()
  const browser: Browser = await chromium.launch({
    channel: process.env.HERO_CHANNEL ?? 'chrome',
    headless: true,
  })
  const page: Page = await browser.newPage()
  page.on('console', (m) => console.log(`[page] ${m.text()}`))
  page.on('pageerror', (e) => console.error(`[page error] ${e.message}`))
  await page.setContent('<!doctype html><html><body></body></html>')
  await page.addScriptTag({ content: code, type: 'module' })
  await page.waitForFunction(() => 'heroPaint' in window)
  const presets = await page.evaluate(() => window.heroPresets)
  return {
    presets,
    render: (req) =>
      page.evaluate((r) => window.heroPaint(r), req) as Promise<PaintResult>,
    toWebp: (png, quality) =>
      page.evaluate(([p, q]) => window.heroToWebp(p as string, q as number), [
        png,
        quality,
      ] as const),
    close: () => browser.close(),
  }
}
