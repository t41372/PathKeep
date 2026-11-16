/**
 * Renders the paintings the landing page sets its content on and cuts each
 * panel's picture out of them, at the widths the page's `srcset` lists.
 *
 *   bun site/hero-lab/export-panels.ts [cut,cut…]
 *
 * Writes `site/public/paintings/<cut>-<width>.webp`.
 *
 * Each cut is the part of a painting its panel shows: the opening card
 * (hero and app window) is one tall painting; the spaces, privacy and
 * download panels show the band their text sits on. Texture scales with a
 * painting's width, so rendering larger only adds pixels, never changes
 * the picture.
 */

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { openRenderer } from './driver'

interface Cut {
  name: string
  /** The region, in pixels of the render. */
  x: number
  y: number
  w: number
  h: number
  /** File widths; heights keep the region's shape. */
  widths: number[]
}

const SOURCES: {
  preset: string
  width: number
  height: number
  cuts: Cut[]
}[] = [
  {
    preset: 'pk-ember',
    width: 3310,
    height: 3800,
    cuts: [
      // The opening card, hero and app window together: the whole painting,
      // light apricot at the top under the headline, the window lower down.
      {
        name: 'intro',
        x: 0,
        y: 0,
        w: 3310,
        h: 3800,
        widths: [1920, 2560, 3310],
      },
      // A phone's opening card is tall and narrow: more of the apricot, centred.
      { name: 'hero-phone', x: 1313, y: 0, w: 684, h: 1368, widths: [720] },
    ],
  },
  {
    preset: 'pk-ember',
    width: 3840,
    height: 2400,
    // The retention chart's PathKeep row: burnt sienna with a pale streak.
    cuts: [{ name: 'path', x: 0, y: 1990, w: 3840, h: 48, widths: [1920] }],
  },
  // The two spaces: the dark foot of each swatch carries the text.
  {
    preset: 'sw-browsing',
    width: 1400,
    height: 1480,
    cuts: [
      {
        name: 'browsing',
        x: 39,
        y: 351,
        w: 1322,
        h: 1129,
        widths: [680, 1320],
      },
    ],
  },
  {
    preset: 'sw-agent',
    width: 1400,
    height: 1480,
    cuts: [
      { name: 'agent', x: 39, y: 351, w: 1322, h: 1129, widths: [680, 1320] },
    ],
  },
  // Indigo above for the statement, the ember streak at the panel's foot.
  {
    preset: 'pk-dusk',
    width: 2752,
    height: 1960,
    cuts: [
      {
        name: 'privacy',
        x: 0,
        y: 0,
        w: 2752,
        h: 1600,
        widths: [1376, 2064, 2752],
      },
    ],
  },
  {
    preset: 'pk-spaces',
    width: 2752,
    height: 1560,
    cuts: [
      {
        name: 'demo',
        x: 0,
        y: 0,
        w: 2752,
        h: 1560,
        widths: [1376, 2064, 2752],
      },
    ],
  },
  // Bone with the grey drag behind the words and the orange line in the gap above the platforms.
  {
    preset: 'pk-paper',
    width: 4096,
    height: 1846,
    cuts: [
      {
        name: 'download',
        x: 667,
        y: 562,
        w: 2762,
        h: 1284,
        widths: [1376, 2064, 2752],
      },
    ],
  },
]

const only = (process.argv[2] ?? '').split(',').filter(Boolean)
const out = join(import.meta.dir, '..', 'public', 'paintings')
await mkdir(out, { recursive: true })

const renderer = await openRenderer()
try {
  for (const source of SOURCES) {
    const cuts = source.cuts.filter(
      (c) => !only.length || only.includes(c.name),
    )
    if (!cuts.length) continue
    const res = await renderer.render({
      preset: source.preset,
      seed: 1,
      width: source.width,
      height: source.height,
      crops: cuts.flatMap((c) =>
        c.widths.map((w) => ({
          name: `${c.name}-${w}`,
          x: c.x,
          y: c.y,
          w: c.w,
          h: c.h,
          outW: w,
          type: 'webp' as const,
          quality: 0.82,
        })),
      ),
    })
    for (const f of res.files) {
      const bytes = Buffer.from(f.base64, 'base64')
      const name = f.name.replace(/^crop-/, '')
      await writeFile(join(out, name), bytes)
      console.log(`paintings/${name} ${(bytes.length / 1024).toFixed(0)} KB`)
    }
  }
} finally {
  await renderer.close()
}
