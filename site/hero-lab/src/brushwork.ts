/**
 * Where the brush goes: seeded generators that place strokes the way a
 * painter would fill a field or drag a dry brush across a boundary, and the
 * ribbon mesh the GPU draws for them.
 *
 * Coordinates are composition space: y down, canvas height = 1, x from 0 to
 * the aspect ratio. Widths and lengths are in the same units.
 */

export interface Stroke {
  x: number
  y: number
  angle: number
  length: number
  width: number
  /** Sideways bow at the middle, as a share of the length. */
  bend: number
  /** Hand tremor, as a share of the width. */
  wobble: number
  load: number
  decay: number
  dryness: number
  bristle: number
  thickness: number
  soft: number
  /** Share of the length spent landing and lifting the brush. */
  taper: number
  /** Extra paint dropped where the brush lands. */
  puddle: number
  seed: number
}

export interface Region {
  x0: number
  y0: number
  x1: number
  y1: number
}

export type Range = readonly [number, number]

export function rng(seed: number): () => number {
  let a = seed >>> 0
  let b = (seed * 2654435761) >>> 0
  let c = (seed ^ 0x9e3779b9) >>> 0
  let d = 1
  return () => {
    a >>>= 0
    b >>>= 0
    c >>>= 0
    d >>>= 0
    const t = (a + b + d) | 0
    d = (d + 1) | 0
    a = b ^ (b >>> 9)
    b = (c + (c << 3)) | 0
    c = (c << 21) | (c >>> 11)
    c = (c + t) | 0
    return (t >>> 0) / 4294967296
  }
}

export type Rand = () => number

function pick(r: Rand, range: Range | number): number {
  if (typeof range === 'number') return range
  return range[0] + (range[1] - range[0]) * r()
}

function gauss(r: Rand): number {
  const u = Math.max(r(), 1e-9)
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * r())
}

export interface BrushSettings {
  count: number
  /** Mean angle in degrees and its spread (standard deviation). */
  angle?: readonly [number, number]
  /** Share of strokes laid at a steeper, scrubbing angle. */
  scrub?: number
  scrubAngle?: number
  /** Stroke length as a share of the region's width. */
  length: Range
  width: Range
  load?: Range | number
  decay?: Range | number
  dryness?: Range | number
  bristle?: Range | number
  thickness?: Range | number
  soft?: Range | number
  taper?: Range | number
  puddle?: Range | number
  bend?: Range | number
  wobble?: Range | number
  /** How far stroke centres may stray outside the region (composition units). */
  margin?: number
  /** Concentrate stroke centres toward the region's top (−1) or bottom (+1) edge. */
  bias?: number
}

function make(
  r: Rand,
  s: BrushSettings,
  cx: number,
  cy: number,
  regionWidth: number,
  angleDeg: number,
): Stroke {
  const length = pick(r, s.length) * regionWidth
  const forward = r() < 0.5 ? 0 : Math.PI
  const angle = (angleDeg * Math.PI) / 180 + forward
  return {
    x: cx - (Math.cos(angle) * length) / 2,
    y: cy - (Math.sin(angle) * length) / 2,
    angle,
    length,
    width: pick(r, s.width),
    bend: pick(r, s.bend ?? [-0.04, 0.04]),
    wobble: pick(r, s.wobble ?? [0.02, 0.08]),
    load: pick(r, s.load ?? [0.8, 1]),
    decay: pick(r, s.decay ?? [0.4, 1.2]),
    dryness: pick(r, s.dryness ?? 0),
    bristle: pick(r, s.bristle ?? [0.35, 0.55]),
    thickness: pick(r, s.thickness ?? [0.3, 0.45]),
    soft: pick(r, s.soft ?? 0.5),
    taper: pick(r, s.taper ?? 0.12),
    puddle: pick(r, s.puddle ?? 0),
    seed: Math.floor(r() * 100000),
  }
}

/** Strokes scattered over a region, mostly along one direction. */
export function fill(r: Rand, region: Region, s: BrushSettings): Stroke[] {
  const out: Stroke[] = []
  const margin = s.margin ?? 0.02
  const [meanAngle, spread] = s.angle ?? [0, 6]
  const width = region.x1 - region.x0
  for (let i = 0; i < s.count; i++) {
    const cx = region.x0 - margin + (width + 2 * margin) * r()
    let v = r()
    if (s.bias)
      v = s.bias > 0 ? 1 - (1 - v) ** (1 + s.bias * 2) : v ** (1 - s.bias * 2)
    const cy = region.y0 - margin + (region.y1 - region.y0 + 2 * margin) * v
    const scrub = r() < (s.scrub ?? 0)
    const angle = scrub
      ? (r() < 0.5 ? -1 : 1) * (s.scrubAngle ?? 35) + gauss(r) * 10
      : meanAngle + gauss(r) * spread
    out.push(make(r, s, cx, cy, width, angle))
  }
  return out
}

export interface SweepSettings extends Omit<BrushSettings, 'margin' | 'bias'> {
  /** Where the sweeps run: a centre line and how far strokes stray from it. */
  y: number
  spread: number
  /** Horizontal span the strokes start in. */
  x: Range
  /** Slope of the centre line, dy per dx. */
  tilt?: number
}

/** Long, mostly horizontal strokes along a line: dry brush over a boundary. */
export function sweep(r: Rand, aspect: number, s: SweepSettings): Stroke[] {
  const out: Stroke[] = []
  const [meanAngle, spread] = s.angle ?? [0, 1.2]
  for (let i = 0; i < s.count; i++) {
    const stroke = make(r, s, 0, 0, aspect, meanAngle + gauss(r) * spread)
    const startX = pick(r, s.x)
    const leftToRight = Math.cos(stroke.angle) > 0
    stroke.x = leftToRight ? startX : startX + stroke.length
    stroke.y = s.y + gauss(r) * s.spread + (s.tilt ?? 0) * (startX - aspect / 2)
    out.push(stroke)
  }
  return out
}

function noise1(seed: number, x: number): number {
  const h = (n: number) => {
    let v = (Math.imul(n ^ seed, 0x27d4eb2d) ^ (n >>> 15)) >>> 0
    v = Math.imul(v ^ (v >>> 13), 0x165667b1) >>> 0
    return (v >>> 0) / 4294967296
  }
  const i = Math.floor(x)
  const f = x - i
  const u = f * f * (3 - 2 * f)
  return (h(i) * (1 - u) + h(i + 1) * u) * 2 - 1
}

export const FLOATS_PER_VERTEX = 18
const SEGMENTS = 72
const OVERSCAN = 1.2

/** Ribbon triangles for every stroke; `refHeight` turns lengths into reference pixels. */
export function strokeMesh(
  strokes: readonly Stroke[],
  refHeight: number,
): Float32Array {
  const verts = new Float32Array(
    strokes.length * SEGMENTS * 6 * FLOATS_PER_VERTEX,
  )
  let o = 0
  const centre = (st: Stroke, s: number): [number, number] => {
    const dx = Math.cos(st.angle)
    const dy = Math.sin(st.angle)
    const along = s * st.length
    const side =
      st.bend * st.length * 4 * s * (1 - s) +
      st.wobble * st.width * noise1(st.seed, s * 3.2)
    return [st.x + dx * along - dy * side, st.y + dy * along + dx * side]
  }
  for (const st of strokes) {
    const lenPx = st.length * refHeight
    const widPx = st.width * refHeight
    const hairs = Math.max(8, widPx / 4.5)
    const rows: number[][] = []
    for (let i = 0; i <= SEGMENTS; i++) {
      const s = -0.03 + (i / SEGMENTS) * 1.06
      const [cx, cy] = centre(st, s)
      const [ax, ay] = centre(st, s - 0.002)
      const [bx, by] = centre(st, s + 0.002)
      let tx = bx - ax
      let ty = by - ay
      const tl = Math.hypot(tx, ty) || 1
      tx /= tl
      ty /= tl
      const press = 0.82 + 0.18 * Math.min(1, Math.max(0, s / 0.07))
      const half =
        (st.width / 2) *
        press *
        (0.94 + 0.06 * noise1(st.seed + 7, s * 5)) *
        OVERSCAN
      rows.push([
        cx - ty * half,
        cy + tx * half,
        s,
        -OVERSCAN,
        cx + ty * half,
        cy - tx * half,
        s,
        OVERSCAN,
      ])
    }
    const attrs = [
      st.seed % 9973,
      st.load,
      st.decay,
      st.dryness,
      st.bristle,
      hairs,
      st.thickness,
      st.soft,
      st.taper,
      st.puddle,
      0,
      0,
      lenPx,
      widPx,
    ]
    const put = (x: number, y: number, s: number, t: number) => {
      verts[o++] = x
      verts[o++] = y
      verts[o++] = s
      verts[o++] = t
      for (const v of attrs) verts[o++] = v
    }
    for (let i = 0; i < SEGMENTS; i++) {
      const a = rows[i]
      const b = rows[i + 1]
      put(a[0], a[1], a[2], a[3])
      put(a[4], a[5], a[6], a[7])
      put(b[0], b[1], b[2], b[3])
      put(a[4], a[5], a[6], a[7])
      put(b[4], b[5], b[6], b[7])
      put(b[0], b[1], b[2], b[3])
    }
  }
  return verts
}
