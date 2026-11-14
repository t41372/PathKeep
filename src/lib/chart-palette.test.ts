// @vitest-environment node
/**
 * The fixed chart colours (`--chart-1` … `--chart-6`, `--chart-other` in
 * `src/index.css`) colour series that are parts of one thing: by tool, by
 * browser, by category. They are picked by number, so a wrong value does not
 * show up as an error anywhere; it shows up as two series nobody can tell
 * apart. The ways that can happen, each checked below in both themes:
 *
 * - a colour is missing, or is not written as `oklch(L C H)`;
 * - a colour is outside sRGB, so the screen shows a different one;
 * - a colour is too faint against a card (below 3:1, WCAG 1.4.11);
 * - two colours are too close for normal vision, or for someone with
 *   protanopia, deuteranopia or tritanopia;
 * - a slot is one hue in light and another in dark, so "the blue one" in a
 *   legend stops being true when the theme changes;
 * - a colour follows the space (`var(--brand)`), so the same chart changes
 *   from one space to the next.
 */
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const css = readFileSync(
  fileURLToPath(new URL('../index.css', import.meta.url)),
  'utf8',
)
const slots = ['1', '2', '3', '4', '5', '6', 'other'] as const

type Rgb = [number, number, number]
type Lch = [number, number, number]

/** The declarations of the first rule whose selector is exactly `selector`. */
function block(selector: string) {
  const start = css.search(
    new RegExp(`(^|\\n)${selector.replace('.', '\\.')}\\s*\\{`),
  )
  if (start < 0) throw new Error(`no ${selector} rule in index.css`)
  return css.slice(start, css.indexOf('\n}', start))
}

function declared(theme: string, slot: string): string | null {
  const match = new RegExp(`--chart-${slot}:\\s*([^;]+);`).exec(block(theme))
  return match ? match[1].trim() : null
}

function parse(value: string | null): Lch | null {
  const match = /^oklch\(([\d.]+) ([\d.]+) ([\d.]+)\)$/.exec(value ?? '')
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null
}

function toLinear([l, c, h]: Lch): Rgb {
  const a = c * Math.cos((h * Math.PI) / 180)
  const b = c * Math.sin((h * Math.PI) / 180)
  const L = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const M = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const S = (l - 0.0894841775 * a - 1.291485548 * b) ** 3
  return [
    4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S,
    -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S,
    -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S,
  ]
}

const clamp = (rgb: Rgb) => rgb.map((x) => Math.min(1, Math.max(0, x))) as Rgb

function toOklab(rgb: Rgb) {
  const [r, g, b] = clamp(rgb)
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ]
}

const distance = (a: Rgb, b: Rgb) =>
  Math.hypot(...toOklab(a).map((x, i) => x - toOklab(b)[i]))

function luminance(rgb: Rgb) {
  const [r, g, b] = clamp(rgb)
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

function contrast(a: Rgb, b: Rgb) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

const fromHex = (hex: string) =>
  [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
  }) as Rgb

/** Full colour-vision deficiency, in linear RGB (Machado, Oliveira and Fernandes 2009). */
const deficiencies: Record<string, number[][]> = {
  protanopia: [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
  ],
  deuteranopia: [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.01182, 0.04294, 0.968881],
  ],
  tritanopia: [
    [1.255528, -0.076749, -0.178779],
    [-0.078411, 0.930809, 0.147602],
    [0.004733, 0.691367, 0.3039],
  ],
}

const seenWith = (matrix: number[][], rgb: Rgb) =>
  matrix.map(
    (row) => row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2],
  ) as Rgb

/**
 * A card as it lands on screen: the card layer over the panel over the
 * window, flattened. Light is all but white; dark is the panel's blue-black
 * lifted a little by the card.
 */
const themes = [
  { name: 'light', selector: ':root', card: fromHex('#fdfdfe') },
  { name: 'dark', selector: '.dark', card: fromHex('#24242f') },
]

describe.each(themes)('fixed chart colours, $name', ({ selector, card }) => {
  const values = slots.map((slot) => ({
    slot,
    raw: declared(selector, slot),
    lch: parse(declared(selector, slot)),
  }))

  it('declares every slot as a plain oklch colour, never the space accent', () => {
    for (const { slot, raw, lch } of values)
      expect(lch, `--chart-${slot}: ${raw}`).not.toBeNull()
  })

  it('stays inside sRGB', () => {
    for (const { slot, lch } of values) {
      const rgb = toLinear(lch!)
      expect(
        rgb.every((x) => x >= -0.0005 && x <= 1.0005),
        `--chart-${slot} is outside sRGB`,
      ).toBe(true)
    }
  })

  it('reads against a card at 3:1 or better', () => {
    for (const { slot, lch } of values)
      expect(
        contrast(toLinear(lch!), card),
        `--chart-${slot} against the card`,
      ).toBeGreaterThanOrEqual(3)
  })

  it('keeps every pair apart, with and without colour-vision deficiency', () => {
    for (const [i, a] of values.entries())
      for (const b of values.slice(i + 1)) {
        const [x, y] = [toLinear(a.lch!), toLinear(b.lch!)]
        expect(
          distance(x, y),
          `--chart-${a.slot} and --chart-${b.slot}`,
        ).toBeGreaterThanOrEqual(0.1)
        for (const [name, matrix] of Object.entries(deficiencies))
          expect(
            distance(seenWith(matrix, x), seenWith(matrix, y)),
            `--chart-${a.slot} and --chart-${b.slot} with ${name}`,
          ).toBeGreaterThanOrEqual(0.1)
      }
  })
})

it('keeps each slot the same hue in light and dark', () => {
  for (const slot of slots) {
    const light = parse(declared(':root', slot))
    const dark = parse(declared('.dark', slot))
    if (!light || !dark) throw new Error(`--chart-${slot} is not declared`)
    // A grey has no hue to compare.
    if (light[1] < 0.04 && dark[1] < 0.04) continue
    const apart = Math.abs(light[2] - dark[2])
    expect(Math.min(apart, 360 - apart), `--chart-${slot}`).toBeLessThan(15)
  }
})
