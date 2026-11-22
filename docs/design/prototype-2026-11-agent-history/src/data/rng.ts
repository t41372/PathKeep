/** Seeded randomness, so every reload draws the same invented archive. */

export type Rng = () => number

export function mulberry32(seed: number): Rng {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

export function pick<T>(rng: Rng, items: readonly T[]): T {
  return items[Math.floor(rng() * items.length)]
}

export function weighted<T>(rng: Rng, items: readonly (readonly [T, number])[]): T {
  const total = items.reduce((sum, [, w]) => sum + w, 0)
  let roll = rng() * total
  for (const [item, w] of items) {
    roll -= w
    if (roll <= 0) return item
  }
  return items[items.length - 1][0]
}

/** Log-normal draw around a median; sigma ~1 gives a long right tail. */
export function lognormal(rng: Rng, median: number, sigma: number): number {
  const u = Math.max(rng(), 1e-9)
  const v = rng()
  const z = Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v)
  return median * Math.exp(sigma * z)
}

export function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

export function hex(rng: Rng, length: number) {
  let out = ''
  for (let i = 0; i < length; i++) out += Math.floor(rng() * 16).toString(16)
  return out
}
