/**
 * Converts the app screenshots that `capture-screens.ts` writes, one folder
 * per language in `site/hero-lab/screens-in/<slug>/*.png`, to WebP (alpha
 * kept) under `site/public/screens/<slug>/`, for the windows on the page.
 *
 *   bun site/hero-lab/export-screens.ts
 */

import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { langs } from '../src/content/copy'
import { openRenderer } from './driver'

const inDir = join(import.meta.dir, 'screens-in')
const outDir = join(import.meta.dir, '..', 'public', 'screens')

const work: { slug: string; name: string }[] = []
for (const { slug } of langs) {
  const pngs = (await readdir(join(inDir, slug)).catch(() => [])).filter((f) =>
    f.endsWith('.png'),
  )
  for (const name of pngs) work.push({ slug, name })
}
if (!work.length) {
  console.log(`no PNG screenshots under ${inDir}`)
  process.exit(0)
}

const renderer = await openRenderer()
try {
  for (const { slug, name } of work) {
    await mkdir(join(outDir, slug), { recursive: true })
    const png = (await readFile(join(inDir, slug, name))).toString('base64')
    const webp = Buffer.from(await renderer.toWebp(png, 0.88), 'base64')
    const file = name.replace(/\.png$/, '.webp')
    await writeFile(join(outDir, slug, file), webp)
    console.log(`screens/${slug}/${file} ${(webp.length / 1024).toFixed(0)} KB`)
  }
} finally {
  await renderer.close()
}
