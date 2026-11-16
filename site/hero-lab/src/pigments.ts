/**
 * Paints described the way a painter tests them: a unit-thick swatch over
 * white and the same swatch over black. Kubelka–Munk turns that pair into
 * absorption (K) and scattering (S) per channel, so a transparent glaze, a
 * chalky scumble and an opaque gouache each behave like themselves when laid
 * over something else.
 */

export interface Pigment {
  K: [number, number, number]
  S: [number, number, number]
}

function hexToLinear(hex: string): [number, number, number] {
  const v = hex.replace('#', '')
  const c = [0, 2, 4].map((i) => parseInt(v.slice(i, i + 2), 16) / 255)
  return c.map((x) =>
    x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4,
  ) as [number, number, number]
}

function acoth(x: number): number {
  return 0.5 * Math.log((x + 1) / (x - 1))
}

function channel(rw: number, rb: number): [number, number] {
  const w = Math.min(Math.max(rw, 0.004), 0.996)
  const b = Math.min(Math.max(rb, 0.0015), w - 0.003)
  const a = 0.5 * (w + (b - w + 1) / b)
  const bb = Math.sqrt(a * a - 1)
  const arg = (bb * bb - (a - w) * (a - 1)) / (bb * (1 - w))
  const S = acoth(arg) / bb
  const K = S * (a - 1)
  return [K, S]
}

/** A paint from its look over white and over black at unit thickness (sRGB hex). */
export function paint(overWhite: string, overBlack: string): Pigment {
  const w = hexToLinear(overWhite)
  const b = hexToLinear(overBlack)
  const ks = [0, 1, 2].map((i) => channel(w[i], b[i]))
  return {
    K: ks.map((p) => p[0]) as [number, number, number],
    S: ks.map((p) => p[1]) as [number, number, number],
  }
}

export function linear(hex: string): [number, number, number] {
  return hexToLinear(hex)
}

/** The palette the presets draw from. Names say what kind of paint each is. */
export const P = {
  // Transparent watercolours and glazes: dark over black.
  ultramarine: paint('#4a63b4', '#0b0d1e'),
  cerulean: paint('#6aa2c6', '#24404f'),
  turquoise: paint('#16908f', '#031615'),
  teal: paint('#1f8a8c', '#05201f'),
  indigo: paint('#2d3654', '#080a12'),
  paynes: paint('#4b5663', '#0d1013'),
  quinGold: paint('#dc9a2c', '#1e1004'),
  quinRose: paint('#cf5a86', '#1f0813'),
  alizarin: paint('#b3303e', '#190306'),
  burntSienna: paint('#b8562c', '#2a1208'),
  pyrrole: paint('#f07a32', '#3a1606'),
  rawUmber: paint('#6f5d43', '#18120a'),
  sepia: paint('#5a4636', '#120d09'),
  violet: paint('#6b4f8f', '#120c1c'),
  // Semi-opaque: some body over black.
  ochre: paint('#d8aa52', '#6a4e1e'),
  lightRed: paint('#cf7d5c', '#5e2c1c'),
  roseGround: paint('#dcab8f', '#5c3424'),
  cadOrange: paint('#ee7a30', '#9a4416'),
  cadYellow: paint('#f1c13d', '#a88016'),
  maroon: paint('#6c2a30', '#1d0a0c'),
  plum: paint('#4d2e48', '#160c15'),
  deepTeal: paint('#2f5f62', '#0d1f20'),
  // Opaque: nearly the same over black (gouache, scumbles, whites).
  white: paint('#f5f2ea', '#d9dce0'),
  buff: paint('#eee2c6', '#d3c4a3'),
  naples: paint('#efd59a', '#c9ac6e'),
  paleBlue: paint('#ccd6e1', '#9faab8'),
  paleRose: paint('#ecd3c7', '#c4a598'),
  apricot: paint('#f0c19e', '#c18a66'),
  emberGouache: paint('#e9773a', '#c25a24'),
  tealGouache: paint('#3f8a86', '#2a6461'),
  greyViolet: paint('#a9a3b5', '#7d778c'),
  inkGouache: paint('#2b2a33', '#1f1e25'),
  // Earth colours for the dragged band paintings.
  rust: paint('#b8745a', '#8e5039'),
  rustDeep: paint('#90523f', '#5e3022'),
  verdigris: paint('#5f8f7f', '#4f7868'),
  bone: paint('#ece2d0', '#d6c9b2'),
  umber: paint('#64433a', '#452a21'),
  ash: paint('#a59d92', '#6e675f'),
  emberRust: paint('#c0602f', '#5a240e'),
  slate: paint('#7a8894', '#3c4650'),
  haze: paint('#b9c3cc', '#8994a0'),
  plumDeep: paint('#4a3a52', '#1b1420'),
  indigoDeep: paint('#36384f', '#12131c'),
  // PathKeep's own walls and heat scale, as paint.
  wallBlue: paint('#d6e2f2', '#aebdd2'),
  lilac: paint('#d9d2e3', '#b2a9c2'),
  wallPeach: paint('#f5dccd', '#d6b4a0'),
  wallApricot: paint('#f1cdb6', '#cc9f84'),
  heat: paint('#f3ad84', '#cf7f55'),
  nightBlue: paint('#3b3d60', '#20213a'),
  plumBlack: paint('#2b1d28', '#140c13'),
} as const
