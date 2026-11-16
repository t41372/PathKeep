/**
 * Page entry for the renderer: `window.heroPaint(request)` paints a preset
 * and hands back the encoded files (base64) for the driver to write to disk.
 */

import { paint } from './painter'
import { presets } from './presets'

export interface PaintRequest {
  preset: string
  seed: number
  width: number
  height: number
  png?: boolean
  webpQuality?: number
  previews?: number[]
  /** Site sizes: one WebP per width, named `w<width>.webp`, no JPEG. */
  sizes?: number[]
  sizeQuality?: number
  crops?: {
    name: string
    x: number
    y: number
    w: number
    h: number
    outW?: number
    type?: 'png' | 'webp'
    quality?: number
  }[]
  /** How many dominant colours to pull out of the finished picture. */
  palette?: number
}

export interface PaintResult {
  ms: number
  log: string[]
  files: { name: string; base64: string }[]
  palette: { hex: string; share: number }[]
}

function toBlob(
  canvas: HTMLCanvasElement,
  type: string,
  quality?: number,
): Promise<Blob> {
  return new Promise((resolve, reject) =>
    canvas.toBlob(
      (b) => (b ? resolve(b) : reject(new Error(`could not encode ${type}`))),
      type,
      quality,
    ),
  )
}

function toBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result).split(',')[1] ?? '')
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })
}

async function heroPaint(req: PaintRequest): Promise<PaintResult> {
  const preset = presets[req.preset]
  if (!preset) throw new Error(`no preset named ${req.preset}`)
  const log: string[] = []
  const painted = document.createElement('canvas')
  const t0 = performance.now()
  const painting = preset.build(req.width / req.height)
  await paint(painted, painting, {
    width: req.width,
    height: req.height,
    seed: req.seed,
    overscan: painting.overscan,
    log: (line) => log.push(line),
  })
  const ms = performance.now() - t0
  // Crop the overscan away so the file is exactly the requested size.
  let canvas = painted
  if (painted.width !== req.width || painted.height !== req.height) {
    canvas = document.createElement('canvas')
    canvas.width = req.width
    canvas.height = req.height
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('no 2d context')
    const ox = Math.round((painted.width - req.width) / 2)
    const oy = Math.round((painted.height - req.height) / 2)
    ctx.drawImage(
      painted,
      ox,
      oy,
      req.width,
      req.height,
      0,
      0,
      req.width,
      req.height,
    )
  }
  const files: PaintResult['files'] = []
  if (req.png)
    files.push({
      name: 'full.png',
      base64: await toBase64(await toBlob(canvas, 'image/png')),
    })
  if (req.webpQuality) {
    files.push({
      name: 'full.webp',
      base64: await toBase64(
        await toBlob(canvas, 'image/webp', req.webpQuality),
      ),
    })
  }
  for (const width of req.previews ?? []) {
    const c = document.createElement('canvas')
    c.width = width
    c.height = Math.round((width * req.height) / req.width)
    const ctx = c.getContext('2d')
    if (!ctx) throw new Error('no 2d context')
    ctx.imageSmoothingEnabled = true
    ctx.imageSmoothingQuality = 'high'
    ctx.drawImage(canvas, 0, 0, c.width, c.height)
    files.push({
      name: `w${width}.jpg`,
      base64: await toBase64(await toBlob(c, 'image/jpeg', 0.92)),
    })
    files.push({
      name: `w${width}.webp`,
      base64: await toBase64(await toBlob(c, 'image/webp', 0.9)),
    })
  }
  for (const width of req.sizes ?? []) {
    const c = document.createElement('canvas')
    c.width = width
    c.height = Math.round((width * req.height) / req.width)
    const ctx = c.getContext('2d')
    if (!ctx) throw new Error('no 2d context')
    ctx.imageSmoothingQuality = 'high'
    ctx.drawImage(canvas, 0, 0, c.width, c.height)
    files.push({
      name: `w${width}.webp`,
      base64: await toBase64(
        await toBlob(c, 'image/webp', req.sizeQuality ?? 0.86),
      ),
    })
  }
  for (const crop of req.crops ?? []) {
    const c = document.createElement('canvas')
    c.width = crop.outW ?? crop.w
    c.height = Math.round((c.width * crop.h) / crop.w)
    const ctx = c.getContext('2d')
    if (!ctx) throw new Error('no 2d context')
    ctx.imageSmoothingQuality = 'high'
    ctx.drawImage(
      canvas,
      crop.x,
      crop.y,
      crop.w,
      crop.h,
      0,
      0,
      c.width,
      c.height,
    )
    const type = crop.type ?? 'png'
    const blob = await toBlob(
      c,
      `image/${type}`,
      type === 'webp' ? (crop.quality ?? 0.92) : undefined,
    )
    files.push({
      name: `crop-${crop.name}.${type}`,
      base64: await toBase64(blob),
    })
  }
  return {
    ms,
    log,
    files,
    palette: req.palette ? dominantColours(canvas, req.palette) : [],
  }
}

// sRGB <-> OKLab, for clustering colours the way they look.
const toLinear = (c: number) =>
  c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
const toSrgb = (c: number) =>
  c <= 0.0031308 ? 12.92 * c : 1.055 * Math.max(c, 0) ** (1 / 2.4) - 0.055

function oklab(r: number, g: number, b: number): [number, number, number] {
  const [lr, lg, lb] = [toLinear(r), toLinear(g), toLinear(b)]
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb)
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb)
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb)
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ]
}

function fromOklab([L, A, B]: [number, number, number]): string {
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3
  const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3
  const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3
  const rgb = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ]
  return `#${rgb
    .map((c) =>
      Math.round(Math.min(1, Math.max(0, toSrgb(c))) * 255)
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`
}

function dominantColours(
  source: HTMLCanvasElement,
  k: number,
): { hex: string; share: number }[] {
  const c = document.createElement('canvas')
  c.width = 240
  c.height = Math.round((240 * source.height) / source.width)
  const ctx = c.getContext('2d')
  if (!ctx) return []
  ctx.imageSmoothingQuality = 'high'
  ctx.drawImage(source, 0, 0, c.width, c.height)
  const data = ctx.getImageData(0, 0, c.width, c.height).data
  const points: [number, number, number][] = []
  for (let i = 0; i < data.length; i += 4)
    points.push(oklab(data[i] / 255, data[i + 1] / 255, data[i + 2] / 255))
  // k-means++ seeding, deterministic.
  const centres: [number, number, number][] = [
    points[Math.floor(points.length / 2)],
  ]
  const dist = (a: number[], b: number[]) =>
    (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2
  while (centres.length < k) {
    let best = 0
    let bestD = -1
    for (let i = 0; i < points.length; i += 7) {
      const d = Math.min(...centres.map((cc) => dist(points[i], cc)))
      if (d > bestD) {
        bestD = d
        best = i
      }
    }
    centres.push(points[best])
  }
  const assign = new Int32Array(points.length)
  for (let iter = 0; iter < 24; iter++) {
    const sums = centres.map(() => [0, 0, 0, 0])
    for (let i = 0; i < points.length; i++) {
      let bi = 0
      let bd = Infinity
      for (let j = 0; j < centres.length; j++) {
        const d = dist(points[i], centres[j])
        if (d < bd) {
          bd = d
          bi = j
        }
      }
      assign[i] = bi
      const sm = sums[bi]
      sm[0] += points[i][0]
      sm[1] += points[i][1]
      sm[2] += points[i][2]
      sm[3]++
    }
    for (let j = 0; j < centres.length; j++) {
      const sm = sums[j]
      if (sm[3] > 0) centres[j] = [sm[0] / sm[3], sm[1] / sm[3], sm[2] / sm[3]]
    }
  }
  const counts = centres.map((_, j) =>
    assign.reduce((n, a) => n + (a === j ? 1 : 0), 0),
  )
  return centres
    .map((cc, j) => ({
      hex: fromOklab(cc),
      share: counts[j] / points.length,
      L: cc[0],
    }))
    .filter((c) => c.share > 0.004)
    .sort((a, b) => b.share - a.share)
    .map(({ hex, share }) => ({ hex, share }))
}

declare global {
  interface Window {
    heroPaint: typeof heroPaint
    heroPresets: string[]
    heroToWebp: typeof toWebp
  }
}

/** Re-encodes a PNG (base64) as WebP, keeping alpha. */
async function toWebp(pngBase64: string, quality: number): Promise<string> {
  const img = new Image()
  img.src = `data:image/png;base64,${pngBase64}`
  await img.decode()
  const c = document.createElement('canvas')
  c.width = img.naturalWidth
  c.height = img.naturalHeight
  const ctx = c.getContext('2d')
  if (!ctx) throw new Error('no 2d context')
  ctx.drawImage(img, 0, 0)
  return toBase64(await toBlob(c, 'image/webp', quality))
}

window.heroPaint = heroPaint
window.heroToWebp = toWebp
window.heroPresets = Object.keys(presets)
