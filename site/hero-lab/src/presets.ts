/**
 * The paintings. Each preset is a recipe: a support, the layers a painter
 * would lay in order, and how the finished surface is lit and graded.
 * Composition space: y down, height 1, x up to the aspect ratio (1.6 at 16:10).
 */

import {
  fill,
  sweep,
  type BrushSettings,
  type Region,
  type SweepSettings,
} from './brushwork'
import type {
  FieldLayer,
  Layer,
  Painting,
  SmearLayer,
  StrokeLayer,
  WashLayer,
} from './painter'
import { P, type Pigment } from './pigments'

export interface Preset {
  id: string
  title: string
  note: string
  build: (aspect: number) => Painting
}

const all = (aspect: number, m = 0.12): Region => ({
  x0: -m,
  y0: -m,
  x1: aspect + m,
  y1: 1 + m,
})
const area = (r: Region) => (r.x1 - r.x0) * (r.y1 - r.y0)

/** A thin, scrubbed ground over the whole canvas. */
function ground(
  pigment: Pigment,
  aspect: number,
  thickness: readonly [number, number] = [0.1, 0.17],
): StrokeLayer {
  return {
    name: 'ground',
    kind: 'strokes',
    pigment,
    impasto: 0.02,
    level: 0.4,
    strokes: (r) =>
      fill(r, all(aspect), {
        count: 110,
        angle: [0, 25],
        length: [0.2, 0.45],
        width: [0.12, 0.24],
        decay: [0.3, 0.9],
        taper: [0.15, 0.3],
        bristle: [0.25, 0.45],
        thickness,
        soft: [0.7, 1],
      }),
  }
}

interface FieldPaints {
  body: Pigment
  bodyThickness?: readonly [number, number]
  glaze?: Pigment
  glazeThickness?: readonly [number, number]
  scumble?: Pigment
  scumbleThickness?: readonly [number, number]
  /** Softness across x, softness across y, edge wander. */
  soft?: readonly [number, number, number]
  density?: number
  /** Where the scumble gathers: −1 top, +1 bottom. */
  scumbleBias?: number
  wear?: { amount: number; size: number; height: number }
  losses?: number
}

/** A colour field built the slow way: a body of soft strokes, a glaze, a broken scumble. */
function field(name: string, region: Region, f: FieldPaints): StrokeLayer[] {
  const soft = f.soft ?? [0.035, 0.03, 0.035]
  const n = (k: number) =>
    Math.max(12, Math.round(k * area(region) * (f.density ?? 1)))
  const brush = (o: Partial<BrushSettings>): BrushSettings => ({
    count: 100,
    angle: [0, 5],
    scrub: 0.18,
    scrubAngle: 28,
    length: [0.14, 0.38],
    width: [0.06, 0.13],
    taper: [0.18, 0.32],
    soft: [0.75, 1],
    bristle: [0.15, 0.35],
    decay: [0.3, 1],
    ...o,
  })
  const layers: StrokeLayer[] = [
    {
      name: `${name} body`,
      kind: 'strokes',
      pigment: f.body,
      region,
      regionSoft: soft,
      impasto: 0.05,
      strokes: (r) =>
        fill(
          r,
          region,
          brush({ count: n(330), thickness: f.bodyThickness ?? [0.07, 0.12] }),
        ),
    },
  ]
  if (f.glaze) {
    layers.push({
      name: `${name} glaze`,
      kind: 'strokes',
      pigment: f.glaze,
      region,
      regionSoft: soft,
      impasto: 0.01,
      level: 0.2,
      strokes: (r) =>
        fill(
          r,
          region,
          brush({
            count: n(150),
            width: [0.08, 0.16],
            thickness: f.glazeThickness ?? [0.03, 0.07],
          }),
        ),
    })
  }
  if (f.scumble) {
    layers.push({
      name: `${name} scumble`,
      kind: 'strokes',
      pigment: f.scumble,
      region,
      regionSoft: soft,
      impasto: 0.08,
      strokes: (r) =>
        fill(
          r,
          region,
          brush({
            count: n(110),
            scrub: 0.3,
            dryness: [0.35, 0.65],
            bristle: [0.45, 0.7],
            soft: [0.5, 0.8],
            taper: [0.12, 0.25],
            thickness: f.scumbleThickness ?? [0.05, 0.11],
            bias: f.scumbleBias,
          }),
        ),
    })
  }
  for (const layer of layers) {
    layer.wear = f.wear
    layer.losses = f.losses
  }
  return layers
}

/** Dry brush dragged along a line: loaded start, starved tail, skipping the valleys. */
function dryBrush(
  name: string,
  pigment: Pigment,
  s: Partial<SweepSettings> & Pick<SweepSettings, 'y' | 'x'>,
): StrokeLayer {
  return {
    name,
    kind: 'strokes',
    pigment,
    impasto: 0.22,
    level: 0.3,
    strokes: (r, a) =>
      sweep(r, a, {
        count: 7,
        spread: 0.02,
        length: [0.16, 0.34],
        width: [0.022, 0.05],
        load: [0.8, 1],
        decay: [1.6, 3],
        dryness: [0.6, 0.85],
        bristle: [0.85, 1],
        thickness: [0.5, 0.8],
        soft: 0.25,
        taper: [0.04, 0.07],
        puddle: [0.1, 0.3],
        wobble: [0.04, 0.12],
        ...s,
      }),
  }
}

function wash(
  name: string,
  pigment: Pigment,
  o: Omit<WashLayer, 'kind' | 'name' | 'pigment'>,
): WashLayer {
  return { name, kind: 'wash', pigment, impasto: 0, level: 0, ...o }
}

interface PoolShape {
  x: number
  y: number
  rx: number
  ry: number
  rotate?: number
  lobes?: number
}

/** An organic pool of wash; `soft` 0 dries hard with a tide line, 1 bleeds wet into wet. */
function pool(
  name: string,
  pigment: Pigment,
  at: PoolShape,
  o: Omit<WashLayer, 'kind' | 'name' | 'pigment' | 'pool' | 'edge'> & {
    soft: number
  },
): WashLayer {
  const { soft, ...rest } = o
  return wash(name, pigment, {
    pool: at,
    edge: { wander: 0, freq: 1, softTop: soft, softBottom: soft },
    ...rest,
  })
}

interface ColourField {
  body: Pigment
  bodyB?: Pigment
  bodyMix?: FieldLayer['mix']
  base?: number
  glaze?: Pigment
  glazeBase?: number
  /** Pull the glaze in from the body's edge (positive) so the body shows as a rim. */
  glazeInset?: number
  scumble?: Pigment
  scumbleBase?: number
  scumbleDry?: number
  patchiness?: number
  streaks?: number
  rim?: number
  edge?: Partial<NonNullable<FieldLayer['edge']>>
  brush?: Partial<NonNullable<FieldLayer['brush']>>
  wear?: { amount: number; size: number; height: number }
  losses?: number
}

const inset = (r: Region, d: number): Region => ({
  x0: r.x0 + d,
  y0: r.y0 + d,
  x1: r.x1 - d,
  y1: r.y1 - d,
})

/** A Rothko-style field: an even body, a thin glaze over it, a dry scumble that breaks over the weave. */
function colourField(
  name: string,
  region: Region,
  f: ColourField,
): FieldLayer[] {
  const edge = {
    softX: 0.03,
    softY: 0.026,
    wander: 0.022,
    fray: 0.02,
    ...f.edge,
  }
  const brush = { angle: 0, width: 26, length: 900, cross: 0.3, ...f.brush }
  const layers: FieldLayer[] = [
    {
      name: `${name} body`,
      kind: 'field',
      pigment: f.body,
      pigmentB: f.bodyB,
      mix: f.bodyMix,
      region,
      edge,
      brush,
      rim: { amount: f.rim ?? 0.12, width: 0.03 },
      paint: {
        base: f.base ?? 0.9,
        patchiness: f.patchiness ?? 0.16,
        streaks: f.streaks ?? 0.22,
        dryness: 0,
      },
      impasto: 0.05,
    },
  ]
  if (f.glaze) {
    layers.push({
      name: `${name} glaze`,
      kind: 'field',
      pigment: f.glaze,
      region: inset(region, f.glazeInset ?? 0.012),
      edge: { ...edge, softX: edge.softX * 1.4, softY: edge.softY * 1.4 },
      brush: { ...brush, angle: brush.angle + 2, length: brush.length * 1.3 },
      paint: {
        base: f.glazeBase ?? 0.22,
        patchiness: 0.45,
        streaks: 0.35,
        dryness: 0,
      },
      impasto: 0.01,
      level: 0.2,
    })
  }
  if (f.scumble) {
    layers.push({
      name: `${name} scumble`,
      kind: 'field',
      pigment: f.scumble,
      region: inset(region, 0.02),
      edge: {
        ...edge,
        softX: edge.softX * 1.8,
        softY: edge.softY * 1.8,
        fray: edge.fray * 1.5,
      },
      brush: { ...brush, angle: brush.angle - 3, cross: 0.6 },
      paint: {
        base: f.scumbleBase ?? 0.3,
        patchiness: 0.6,
        streaks: 0.55,
        dryness: f.scumbleDry ?? 0.5,
      },
      impasto: 0.08,
    })
  }
  for (const layer of layers) {
    layer.wear = f.wear
    layer.losses = f.losses
  }
  return layers
}

/** The ground: one thin colour over the whole canvas, brushed every which way. */
function groundField(
  pigment: Pigment,
  aspect: number,
  base: number,
): FieldLayer {
  return {
    name: 'ground',
    kind: 'field',
    pigment,
    region: { x0: -0.3, y0: -0.3, x1: aspect + 0.3, y1: 1.3 },
    edge: { softX: 0.01, softY: 0.01, wander: 0, fray: 0 },
    brush: { angle: 0, width: 34, length: 700, cross: 0.8 },
    paint: { base, patchiness: 0.35, streaks: 0.3, dryness: 0 },
    impasto: 0.02,
    level: 0.4,
  }
}

const cfDawn: Preset = {
  id: 'cf-dawn',
  title: 'Color field · Dawn',
  note: 'Raw linen, a rose-red ground; a large orange field on top, a pale slab to its right, a thin rose band, a cream field below.',
  build: (aspect) => {
    const top: Region = { x0: -0.06, y0: -0.06, x1: 1.32, y1: 0.58 }
    const side: Region = { x0: 1.4, y0: -0.06, x1: aspect + 0.06, y1: 0.58 }
    const band: Region = { x0: 0.08, y0: 0.645, x1: aspect + 0.06, y1: 0.705 }
    const low: Region = { x0: -0.06, y0: 0.77, x1: aspect + 0.06, y1: 1.06 }
    return {
      support: { kind: 'linen', base: '#c9b99c', fleck: '#5b4733' },
      layers: [
        groundField(P.lightRed, aspect, 0.3),
        ...colourField('orange', top, {
          body: P.cadOrange,
          base: 0.95,
          glaze: P.pyrrole,
          glazeBase: 0.25,
          scumble: P.apricot,
          scumbleBase: 0.32,
          scumbleDry: 0.45,
        }),
        ...colourField('pale slab', side, {
          body: P.paleRose,
          base: 0.85,
          scumble: P.white,
          scumbleBase: 0.3,
        }),
        ...colourField('rose band', band, {
          body: P.lightRed,
          bodyB: P.alizarin,
          bodyMix: { base: 0.3, vary: 0.5, freq: 2, gradient: 0 },
          base: 0.8,
          edge: { softY: 0.012, fray: 0.012, wander: 0.01 },
        }),
        ...colourField('cream', low, {
          body: P.naples,
          base: 0.85,
          glaze: P.ochre,
          glazeBase: 0.1,
          scumble: P.white,
          scumbleBase: 0.3,
        }),
        dryBrush('orange drag', P.emberGouache, {
          y: 0.605,
          x: [0, 1.2],
          width: [0.04, 0.075],
          length: [0.2, 0.42],
        }),
        dryBrush('cream drag', P.buff, {
          y: 0.75,
          x: [0.2, 1.4],
          count: 5,
          width: [0.035, 0.065],
          length: [0.18, 0.38],
        }),
      ],
      finish: {
        relief: 0.3,
        gloss: 0.025,
        vignette: 0.1,
        saturation: 0.97,
        tint: '#fff8f0',
        lift: '#2b2320',
        liftAmount: 0.03,
        grain: 0.013,
      },
    }
  },
}

const cfEmber: Preset = {
  id: 'cf-ember',
  title: 'Color field · Ember',
  note: 'A maroon ground; a deep plum field and a maroon field on the left, an ember field glowing down the right side.',
  build: (aspect) => {
    const dark: Region = { x0: -0.06, y0: -0.06, x1: 1.15, y1: 0.63 }
    const glow: Region = { x0: 1.24, y0: 0.07, x1: aspect + 0.06, y1: 0.93 }
    const low: Region = { x0: -0.06, y0: 0.71, x1: 1.15, y1: 1.06 }
    return {
      support: { kind: 'linen', base: '#c4b394', fleck: '#4f3d2b' },
      layers: [
        groundField(P.maroon, aspect, 0.9),
        ...colourField('plum', dark, {
          body: P.plum,
          bodyB: P.maroon,
          bodyMix: { base: 0.35, vary: 0.7, freq: 1.6, gradient: 0.5 },
          base: 0.62,
          patchiness: 0.3,
          glaze: P.indigo,
          glazeBase: 0.12,
          scumble: P.lightRed,
          scumbleBase: 0.22,
          scumbleDry: 0.5,
        }),
        ...colourField('ember', glow, {
          body: P.cadOrange,
          base: 1,
          glaze: P.pyrrole,
          glazeBase: 0.4,
          scumble: P.cadYellow,
          scumbleBase: 0.14,
          scumbleDry: 0.55,
          rim: 0.25,
        }),
        ...colourField('maroon', low, {
          body: P.maroon,
          base: 0.9,
          glaze: P.alizarin,
          glazeBase: 0.25,
          scumble: P.lightRed,
          scumbleBase: 0.25,
          scumbleDry: 0.6,
        }),
        dryBrush('ember drag', P.emberGouache, {
          y: 0.67,
          x: [0.3, 1.2],
          count: 6,
          width: [0.04, 0.075],
          length: [0.2, 0.4],
        }),
      ],
      finish: {
        relief: 0.3,
        gloss: 0.04,
        vignette: 0.2,
        tint: '#fff4ec',
        lift: '#140c10',
        liftAmount: 0.04,
        grain: 0.016,
      },
    }
  },
}

const cfSpaces: Preset = {
  id: 'cf-spaces',
  title: 'Color field · Two spaces',
  note: 'Raw linen barely sized; a terracotta field on the left, a deep teal field on the right, bare linen between them.',
  build: (aspect) => {
    const left: Region = { x0: -0.06, y0: -0.06, x1: 0.96, y1: 1.06 }
    const right: Region = { x0: 1.1, y0: -0.06, x1: aspect + 0.06, y1: 0.76 }
    const corner: Region = { x0: 1.1, y0: 0.84, x1: aspect + 0.06, y1: 1.06 }
    return {
      support: { kind: 'linen', base: '#cbbb9e', fleck: '#5b4733' },
      layers: [
        groundField(P.ochre, aspect, 0.12),
        ...colourField('terracotta', left, {
          body: P.cadOrange,
          base: 0.95,
          glaze: P.burntSienna,
          glazeBase: 0.2,
          scumble: P.apricot,
          scumbleBase: 0.3,
        }),
        ...colourField('teal', right, {
          body: P.deepTeal,
          base: 1,
          glaze: P.teal,
          glazeBase: 0.25,
          scumble: P.tealGouache,
          scumbleBase: 0.3,
          scumbleDry: 0.55,
        }),
        ...colourField('linen wash', corner, {
          body: P.buff,
          base: 0.3,
          patchiness: 0.4,
        }),
        dryBrush('teal drag', P.tealGouache, {
          y: 0.8,
          spread: 0.02,
          x: [1.05, 1.6],
          count: 6,
          width: [0.04, 0.075],
          length: [0.2, 0.4],
        }),
      ],
      finish: {
        relief: 0.34,
        gloss: 0.03,
        vignette: 0.12,
        tint: '#fffaf4',
        lift: '#22201c',
        liftAmount: 0.03,
        grain: 0.013,
      },
    }
  },
}

const cfFresco: Preset = {
  id: 'cf-fresco',
  title: 'Color field · Fresco',
  note: 'Lime plaster; faded terracotta, ochre and verdigris fields, rubbed through on the high points, flaked and crazed.',
  build: (aspect) => {
    const upper: Region = { x0: -0.06, y0: -0.06, x1: 1.3, y1: 0.54 }
    const upRight: Region = { x0: 1.38, y0: -0.06, x1: aspect + 0.06, y1: 0.54 }
    const lowLeft: Region = { x0: -0.06, y0: 0.61, x1: 0.84, y1: 1.06 }
    const lowRight: Region = { x0: 0.92, y0: 0.62, x1: aspect + 0.06, y1: 1.06 }
    const wear = { amount: 0.45, size: 520, height: 0.56 }
    return {
      support: { kind: 'plaster', base: '#e8e0cf', fleck: '#8c7b62' },
      layers: [
        groundField(P.ochre, aspect, 0.15),
        ...colourField('terracotta', upper, {
          body: P.lightRed,
          base: 0.8,
          glaze: P.burntSienna,
          glazeBase: 0.12,
          scumble: P.paleRose,
          wear,
          losses: 0.03,
        }),
        ...colourField('pale', upRight, {
          body: P.buff,
          base: 0.7,
          scumble: P.white,
          wear,
          losses: 0.03,
        }),
        ...colourField('ochre', lowLeft, {
          body: P.ochre,
          base: 0.8,
          glaze: P.quinGold,
          glazeBase: 0.1,
          scumble: P.naples,
          wear,
          losses: 0.03,
        }),
        ...colourField('verdigris', lowRight, {
          body: P.tealGouache,
          base: 0.75,
          glaze: P.deepTeal,
          glazeBase: 0.1,
          scumble: P.white,
          wear,
          losses: 0.03,
        }),
      ],
      finish: {
        relief: 0.45,
        crack: 0.14,
        crackSize: 64,
        vignette: 0.12,
        saturation: 0.86,
        tint: '#fbf4e8',
        lift: '#3b3128',
        liftAmount: 0.05,
        grain: 0.016,
      },
    }
  },
}

const paper = {
  kind: 'paper',
  base: '#f4efe5',
  fleck: '#b5a68a',
  scale: 1.35,
} as const

const wcDawn: Preset = {
  id: 'wc-dawn',
  title: 'Watercolour · Dawn',
  note: 'Cold-pressed paper; gold, rose and cerulean pools bleeding into each other on a diagonal, an orange pool dried hard at the lower left, gouache dragged through the lower half.',
  build: () => ({
    support: { ...paper, age: 0.3 },
    layers: [
      pool(
        'cerulean',
        P.cerulean,
        { x: 1.32, y: 0.22, rx: 0.62, ry: 0.5, rotate: 18, lobes: 0.5 },
        {
          soft: 0.9,
          pigmentB: P.violet,
          mix: { base: 0.25, vary: 0.5, freq: 1.3, gradient: 0.3 },
          density: { base: 0.2, mottle: 0.55, pool: 0.25, streaks: 0.04 },
          texture: { granulation: 0.55, flocculation: 0.15, tide: 0.7 },
        },
      ),
      pool(
        'rose',
        P.quinRose,
        { x: 0.88, y: 0.5, rx: 0.62, ry: 0.36, rotate: -24, lobes: 0.5 },
        {
          soft: 0.8,
          pigmentB: P.quinGold,
          mix: { base: 0.3, vary: 0.6, freq: 1.2, gradient: 0.6 },
          wetVary: 0.6,
          density: { base: 0.2, mottle: 0.5, pool: 0.3, streaks: 0.05 },
          texture: { granulation: 0.12, flocculation: 0.1, tide: 0.8 },
          bloom: { count: 1, radius: 0.1, strength: 0.6 },
        },
      ),
      pool(
        'orange',
        P.pyrrole,
        { x: 0.36, y: 0.84, rx: 0.62, ry: 0.3, rotate: -14, lobes: 0.4 },
        {
          soft: 0.3,
          pigmentB: P.quinGold,
          mix: { base: 0.4, vary: 0.6, freq: 1.4, gradient: -0.5 },
          wetVary: 0.6,
          density: { base: 0.4, mottle: 0.45, pool: 0.3, streaks: 0.08 },
          texture: { granulation: 0.3, flocculation: 0.12, tide: 1 },
          bloom: { count: 1, radius: 0.08, strength: 0.7 },
        },
      ),
      pool(
        'gold',
        P.quinGold,
        { x: 0.05, y: 0.55, rx: 0.3, ry: 0.22, rotate: 10 },
        {
          soft: 0.75,
          density: { base: 0.22, mottle: 0.5, pool: 0.2, streaks: 0.04 },
          texture: { granulation: 0.1, flocculation: 0.1, tide: 0.6 },
        },
      ),
      dryBrush('buff gouache', P.buff, {
        y: 0.8,
        spread: 0.06,
        x: [-0.1, 1.1],
        count: 6,
        length: [0.25, 0.5],
        width: [0.03, 0.065],
        tilt: -0.1,
      }),
      dryBrush('ember gouache', P.emberGouache, {
        y: 0.9,
        spread: 0.03,
        x: [0.1, 1.1],
        count: 3,
        length: [0.18, 0.36],
        width: [0.018, 0.035],
        tilt: -0.1,
      }),
      dryBrush('blue gouache', P.paleBlue, {
        y: 0.56,
        spread: 0.03,
        x: [1, 1.5],
        count: 3,
        length: [0.12, 0.25],
        width: [0.02, 0.035],
      }),
    ],
    finish: {
      relief: 0.28,
      vignette: 0.06,
      saturation: 0.96,
      tint: '#fffaf3',
      lift: '#3a3230',
      liftAmount: 0.02,
      grain: 0.011,
    },
  }),
}

const wcDusk: Preset = {
  id: 'wc-dusk',
  title: 'Watercolour · Dusk',
  note: 'Indigo and violet washed over the whole sheet, a sienna warmth left of centre, Payne’s grey settling at the bottom, ember gouache dragged across in the dark.',
  build: (aspect) => ({
    support: { ...paper, age: 0.2 },
    layers: [
      wash('indigo night', P.indigo, {
        pigmentB: P.violet,
        band: [-0.4, 1.4, -0.4, aspect + 0.4],
        density: { base: 0.58, mottle: 0.5, pool: 0.1, streaks: 0.07 },
        texture: { granulation: 0.45, flocculation: 0.15, tide: 0.6 },
        mix: { base: 0.4, vary: 0.75, freq: 0.9, gradient: -0.3 },
        bloom: { count: 2, radius: 0.13, strength: 0.5 },
      }),
      pool(
        'sienna warmth',
        P.burntSienna,
        { x: 0.42, y: 0.6, rx: 0.6, ry: 0.28, rotate: -8, lobes: 0.5 },
        {
          soft: 0.85,
          pigmentB: P.alizarin,
          mix: { base: 0.3, vary: 0.6, freq: 1.3, gradient: 0 },
          density: { base: 0.3, mottle: 0.5, pool: 0.2, streaks: 0.05 },
          texture: { granulation: 0.4, flocculation: 0.1, tide: 0.5 },
        },
      ),
      pool(
        'rose veil',
        P.quinRose,
        { x: 1.32, y: 0.22, rx: 0.5, ry: 0.36, rotate: 15, lobes: 0.5 },
        {
          soft: 0.9,
          pigmentB: P.violet,
          mix: { base: 0.4, vary: 0.6, freq: 1.2, gradient: 0 },
          density: { base: 0.26, mottle: 0.5, pool: 0.2, streaks: 0.05 },
          texture: { granulation: 0.2, flocculation: 0.1, tide: 0.5 },
        },
      ),
      wash('paynes low', P.paynes, {
        band: [0.8, 1.4, -0.4, aspect + 0.4],
        edge: { wander: 0.03, freq: 1.5, softTop: 0.85, softBottom: 1 },
        density: { base: 0.38, mottle: 0.4, pool: 0, streaks: 0.08 },
        texture: { granulation: 0.35, flocculation: 0.15, tide: 0.5 },
      }),
      dryBrush('ember gouache', P.emberGouache, {
        y: 0.72,
        spread: 0.05,
        x: [0, 1.15],
        count: 7,
        length: [0.2, 0.45],
        width: [0.025, 0.055],
        tilt: -0.05,
      }),
      dryBrush('buff gouache', P.buff, {
        y: 0.86,
        spread: 0.04,
        x: [0.2, 1.3],
        count: 3,
        thickness: [0.35, 0.55],
      }),
    ],
    finish: {
      relief: 0.28,
      vignette: 0.16,
      tint: '#fff6ee',
      lift: '#14121c',
      liftAmount: 0.04,
      grain: 0.014,
    },
  }),
}

const wcTide: Preset = {
  id: 'wc-tide',
  title: 'Watercolour · Tide',
  note: 'Turquoise and cerulean sweeping in from the right, burnt sienna granulating where it meets them on a diagonal, white gouache dragged across.',
  build: () => ({
    support: { ...paper, age: 0.25 },
    layers: [
      pool(
        'turquoise',
        P.turquoise,
        { x: 1.12, y: 0.42, rx: 0.78, ry: 0.62, rotate: 28, lobes: 0.45 },
        {
          soft: 0.75,
          pigmentB: P.cerulean,
          mix: { base: 0.35, vary: 0.7, freq: 1.2, gradient: 0.5 },
          wetVary: 0.6,
          density: { base: 0.28, mottle: 0.55, pool: 0.3, streaks: 0.05 },
          texture: { granulation: 0.45, flocculation: 0.12, tide: 0.8 },
          bloom: { count: 1, radius: 0.11, strength: 0.6 },
        },
      ),
      pool(
        'sienna',
        P.burntSienna,
        { x: 0.28, y: 0.72, rx: 0.48, ry: 0.42, rotate: -24, lobes: 0.45 },
        {
          soft: 0.45,
          pigmentB: P.pyrrole,
          mix: { base: 0.45, vary: 0.7, freq: 1.3, gradient: 0 },
          density: { base: 0.38, mottle: 0.45, pool: 0.3, streaks: 0.06 },
          texture: { granulation: 0.55, flocculation: 0.12, tide: 0.9 },
          bloom: { count: 1, radius: 0.09, strength: 0.7 },
        },
      ),
      pool(
        'gold',
        P.quinGold,
        { x: 0.12, y: 0.24, rx: 0.36, ry: 0.22, rotate: -8 },
        {
          soft: 0.7,
          density: { base: 0.22, mottle: 0.45, pool: 0.2, streaks: 0.05 },
          texture: { granulation: 0.12, flocculation: 0.1, tide: 0.7 },
        },
      ),
      dryBrush('white gouache', P.white, {
        y: 0.78,
        spread: 0.07,
        x: [-0.1, 1.3],
        count: 6,
        length: [0.25, 0.5],
        width: [0.03, 0.065],
        tilt: -0.06,
      }),
      dryBrush('teal gouache', P.tealGouache, {
        y: 0.9,
        spread: 0.035,
        x: [0.4, 1.3],
        count: 3,
        length: [0.15, 0.32],
        width: [0.02, 0.04],
      }),
    ],
    finish: {
      relief: 0.28,
      vignette: 0.07,
      saturation: 0.97,
      tint: '#fffaf4',
      lift: '#2c3232',
      liftAmount: 0.02,
      grain: 0.011,
    },
  }),
}

const wcMist: Preset = {
  id: 'wc-mist',
  title: 'Watercolour · Mist',
  note: 'Mostly bare paper: a grey-blue pool and a peach pool, barely there, and a few strokes of gouache, two of them ember.',
  build: () => ({
    support: { ...paper, age: 0.45 },
    layers: [
      pool(
        'grey-blue',
        P.paynes,
        { x: 1.2, y: 0.26, rx: 0.74, ry: 0.42, rotate: 10, lobes: 0.5 },
        {
          soft: 0.9,
          pigmentB: P.cerulean,
          mix: { base: 0.5, vary: 0.6, freq: 1.2, gradient: 0 },
          density: { base: 0.12, mottle: 0.6, pool: 0.25, streaks: 0.03 },
          texture: { granulation: 0.45, flocculation: 0.12, tide: 0.7 },
        },
      ),
      pool(
        'peach',
        P.quinRose,
        { x: 0.3, y: 0.66, rx: 0.58, ry: 0.3, rotate: -8, lobes: 0.5 },
        {
          soft: 0.75,
          pigmentB: P.quinGold,
          mix: { base: 0.55, vary: 0.5, freq: 1.2, gradient: 0 },
          wetVary: 0.6,
          density: { base: 0.12, mottle: 0.55, pool: 0.3, streaks: 0.03 },
          texture: { granulation: 0.12, flocculation: 0.1, tide: 0.8 },
        },
      ),
      dryBrush('buff gouache', P.buff, {
        y: 0.83,
        spread: 0.04,
        x: [0.15, 1.1],
        count: 3,
        length: [0.22, 0.45],
        width: [0.025, 0.05],
      }),
      dryBrush('ember line', P.emberGouache, {
        y: 0.87,
        spread: 0.02,
        x: [0.4, 0.9],
        count: 2,
        length: [0.18, 0.32],
        width: [0.014, 0.026],
      }),
    ],
    finish: {
      relief: 0.3,
      vignette: 0.05,
      saturation: 0.95,
      tint: '#fffaf2',
      lift: '#3a3230',
      liftAmount: 0.01,
      grain: 0.01,
    },
  }),
}

const wcStain: Preset = {
  id: 'wc-stain',
  title: 'Soak-stain · Raw linen',
  note: 'Thinned paint poured into unprimed linen: orange and gold pools on the left, a teal pool on the right, rose below, stained halos, gouache dragged across the lower half.',
  build: () => ({
    support: { kind: 'linen', base: '#cdbda1', fleck: '#5b4733' },
    layers: [
      pool(
        'orange stain',
        P.pyrrole,
        { x: 0.52, y: 0.4, rx: 0.66, ry: 0.4, rotate: -10, lobes: 0.55 },
        {
          soft: 0.35,
          pigmentB: P.quinGold,
          mix: { base: 0.3, vary: 0.6, freq: 1.2, gradient: 0.3 },
          wetVary: 0.6,
          density: { base: 0.48, mottle: 0.45, pool: 0.2, streaks: 0.04 },
          texture: { granulation: 0.1, flocculation: 0.05, tide: 0.7 },
          fringe: 0.25,
        },
      ),
      pool(
        'gold stain',
        P.quinGold,
        { x: 0.18, y: 0.18, rx: 0.34, ry: 0.24, rotate: 12, lobes: 0.5 },
        {
          soft: 0.5,
          density: { base: 0.32, mottle: 0.45, pool: 0.2, streaks: 0.04 },
          texture: { granulation: 0.08, flocculation: 0.05, tide: 0.6 },
          fringe: 0.2,
        },
      ),
      pool(
        'teal stain',
        P.teal,
        { x: 1.36, y: 0.6, rx: 0.42, ry: 0.4, rotate: 20, lobes: 0.6 },
        {
          soft: 0.3,
          pigmentB: P.turquoise,
          mix: { base: 0.35, vary: 0.6, freq: 1.2, gradient: 0 },
          density: { base: 0.4, mottle: 0.45, pool: 0.2, streaks: 0.04 },
          texture: { granulation: 0.1, flocculation: 0.05, tide: 0.7 },
          fringe: 0.25,
        },
      ),
      pool(
        'rose stain',
        P.quinRose,
        { x: 0.26, y: 0.9, rx: 0.4, ry: 0.2, rotate: 5 },
        {
          soft: 0.5,
          density: { base: 0.32, mottle: 0.45, pool: 0.2, streaks: 0.04 },
          texture: { granulation: 0.1, flocculation: 0.05, tide: 0.6 },
          fringe: 0.2,
        },
      ),
      dryBrush('buff gouache', P.buff, {
        y: 0.8,
        spread: 0.05,
        x: [-0.1, 1.2],
        count: 6,
      }),
      dryBrush('ember gouache', P.emberGouache, {
        y: 0.9,
        spread: 0.03,
        x: [0.2, 1.2],
        count: 3,
      }),
    ],
    finish: {
      relief: 0.32,
      vignette: 0.1,
      saturation: 0.97,
      tint: '#fffaf2',
      lift: '#2b2320',
      liftAmount: 0.02,
      grain: 0.012,
    },
  }),
}

interface BandSpec {
  name: string
  y0: number
  y1: number
  body: Pigment
  bodyB?: Pigment
  mix?: FieldLayer['mix']
  base?: number
  /** Vertical feather of the top and bottom edges (composition units). */
  soft?: number
  /** Horizontal extent and its feather; full width by default. */
  x?: readonly [number, number]
  softX?: number
  glaze?: Pigment
  scumble?: Pigment
  scumbleDry?: number
}

/** A full-width band laid with long horizontal strokes, feathered wide into its neighbours. */
function band(aspect: number, b: BandSpec): FieldLayer[] {
  const region: Region = {
    x0: b.x?.[0] ?? -0.5,
    y0: b.y0,
    x1: b.x?.[1] ?? aspect + 0.5,
    y1: b.y1,
  }
  const edge = {
    softX: b.softX ?? 0.2,
    softY: b.soft ?? 0.07,
    wander: 0.035,
    fray: 0.05,
  }
  const brush = { angle: 0, width: 20, length: 1500, cross: 0.06 }
  const layers: FieldLayer[] = [
    {
      name: `${b.name} body`,
      kind: 'field',
      pigment: b.body,
      pigmentB: b.bodyB,
      mix: b.mix,
      region,
      edge,
      brush,
      paint: {
        base: b.base ?? 0.95,
        patchiness: 0.24,
        streaks: 0.68,
        dryness: 0,
      },
      impasto: 0.04,
      level: 0.3,
    },
  ]
  if (b.glaze) {
    layers.push({
      name: `${b.name} glaze`,
      kind: 'field',
      pigment: b.glaze,
      region,
      edge: { ...edge, softY: edge.softY * 1.3 },
      brush: { ...brush, length: 1900 },
      paint: { base: 0.18, patchiness: 0.5, streaks: 0.45, dryness: 0 },
      impasto: 0.01,
      level: 0.1,
    })
  }
  if (b.scumble) {
    layers.push({
      name: `${b.name} scumble`,
      kind: 'field',
      pigment: b.scumble,
      region,
      edge: { ...edge, softY: edge.softY * 1.4, fray: edge.fray * 1.4 },
      brush: { ...brush, width: 14, length: 1200 },
      paint: {
        base: 0.3,
        patchiness: 0.6,
        streaks: 0.6,
        dryness: b.scumbleDry ?? 0.5,
      },
      impasto: 0.06,
    })
  }
  return layers
}

/** Drags across the whole canvas: a very long one, a long one, one back the other way, a fine one. */
function drags(strength = 1): SmearLayer[] {
  return [
    {
      kind: 'smear',
      name: 'sweep',
      angle: 0,
      length: 2200,
      strength: 0.7 * strength,
      coverage: 0.7,
      rows: { width: 18, contrast: 1.1, span: 2600, wobble: 30 },
    },
    {
      kind: 'smear',
      name: 'long drag',
      angle: 0,
      length: 1100,
      strength: 0.8 * strength,
      coverage: 0.62,
      rows: { width: 12, contrast: 1.3, span: 1600, wobble: 26 },
    },
    {
      kind: 'smear',
      name: 'back drag',
      angle: 180,
      length: 600,
      strength: 0.6 * strength,
      coverage: 0.5,
      rows: { width: 7, contrast: 1.4, span: 1100, wobble: 18 },
    },
    {
      kind: 'smear',
      name: 'fine drag',
      angle: 0,
      length: 260,
      strength: 0.55 * strength,
      coverage: 0.55,
      rows: { width: 3.5, contrast: 2.2, span: 700, wobble: 8 },
    },
  ]
}

const lightDrag: SmearLayer = {
  kind: 'smear',
  name: 'last drag',
  angle: 0,
  length: 420,
  strength: 0.35,
  coverage: 0.45,
  rows: { width: 5, contrast: 1.4, span: 900, wobble: 10 },
}

/** Wide, starved strokes along a boundary between two bands. */
function seam(
  name: string,
  pigment: Pigment,
  y: number,
  count = 7,
): StrokeLayer {
  return dryBrush(name, pigment, {
    y,
    spread: 0.018,
    x: [-0.3, 1.6],
    count,
    length: [0.4, 0.9],
    width: [0.05, 0.09],
    dryness: [0.55, 0.8],
    decay: [1.2, 2.4],
    bristle: [0.85, 1],
    thickness: [0.22, 0.4],
  })
}

const ribbedLinen = {
  kind: 'linen',
  base: '#c5b49a',
  fleck: '#5b4733',
  scale: 1.45,
  rib: 0.7,
} as const
const bandFinish = {
  relief: 0.62,
  vignette: 0.03,
  saturation: 0.95,
  tint: '#fffaf4',
  lift: '#241c18',
  liftAmount: 0.02,
  grain: 0.046,
}

const pkFinish = {
  relief: 0.62,
  vignette: 0.03,
  saturation: 0.96,
  tint: '#fffaf4',
  lift: '#2a2220',
  liftAmount: 0.015,
  grain: 0.044,
}

const pkDawn: Preset = {
  id: 'pk-dawn',
  title: 'Dragged bands · Light wall',
  note: 'The app’s light wall as paint: a pale wall blue on top, a lilac haze, peach and apricot below, one streak of PathKeep orange dragged in from the left.',
  build: (aspect) => ({
    overscan: 0.05,
    support: ribbedLinen,
    layers: [
      groundField(P.paleRose, aspect, 0.4),
      ...band(aspect, {
        name: 'wall blue',
        y0: -0.5,
        y1: 0.33,
        soft: 0.11,
        body: P.wallBlue,
        bodyB: P.haze,
        mix: { base: 0.25, vary: 0.6, freq: 1.3, gradient: 0.4 },
        scumble: P.white,
        scumbleDry: 0.62,
      }),
      ...band(aspect, {
        name: 'lilac',
        y0: 0.28,
        y1: 0.5,
        soft: 0.1,
        body: P.lilac,
        bodyB: P.wallPeach,
        mix: { base: 0.3, vary: 0.6, freq: 1.4, gradient: 0.8 },
      }),
      ...band(aspect, {
        name: 'peach',
        y0: 0.46,
        y1: 0.8,
        soft: 0.1,
        body: P.wallPeach,
        bodyB: P.wallApricot,
        mix: { base: 0.4, vary: 0.6, freq: 1.3, gradient: 0.6 },
        scumble: P.white,
        scumbleDry: 0.7,
      }),
      ...band(aspect, {
        name: 'ember streak',
        y0: 0.6,
        y1: 0.645,
        soft: 0.02,
        x: [-0.5, 1.15],
        softX: 0.45,
        body: P.emberGouache,
        base: 0.7,
      }),
      ...band(aspect, {
        name: 'apricot',
        y0: 0.78,
        y1: 1.5,
        soft: 0.08,
        body: P.wallApricot,
        bodyB: P.heat,
        mix: { base: 0.2, vary: 0.6, freq: 1.4, gradient: 0.5 },
        scumble: P.naples,
        scumbleDry: 0.68,
      }),
      seam('ember drag', P.emberGouache, 0.62, 5),
      seam('blue drag', P.wallBlue, 0.31, 5),
      ...drags(),
      lightDrag,
    ],
    finish: { ...pkFinish, lift: '#3a3230', liftAmount: 0.01 },
  }),
}

const pkDusk: Preset = {
  id: 'pk-dusk',
  title: 'Dragged bands · Dark wall',
  note: 'The app’s dark wall as paint: deep indigo above, plum and maroon sinking below, one ember streak low down.',
  build: (aspect) => ({
    overscan: 0.05,
    support: ribbedLinen,
    layers: [
      groundField(P.indigoDeep, aspect, 0.8),
      ...band(aspect, {
        name: 'indigo',
        y0: -0.5,
        y1: 0.52,
        soft: 0.11,
        body: P.nightBlue,
        bodyB: P.indigoDeep,
        mix: { base: 0.4, vary: 0.7, freq: 1.2, gradient: 0.5 },
        glaze: P.violet,
      }),
      ...band(aspect, {
        name: 'violet haze',
        y0: 0.34,
        y1: 0.5,
        soft: 0.08,
        body: P.greyViolet,
        bodyB: P.lilac,
        mix: { base: 0.3, vary: 0.6, freq: 1.4, gradient: 0 },
        base: 0.45,
      }),
      ...band(aspect, {
        name: 'plum',
        y0: 0.48,
        y1: 0.78,
        soft: 0.1,
        body: P.plumDeep,
        bodyB: P.lightRed,
        mix: { base: 0.15, vary: 0.6, freq: 1.3, gradient: 0.6 },
      }),
      ...band(aspect, {
        name: 'ember streak',
        y0: 0.69,
        y1: 0.76,
        soft: 0.024,
        x: [-0.5, aspect + 0.5],
        softX: 0.3,
        body: P.cadOrange,
        bodyB: P.emberRust,
        mix: { base: 0.3, vary: 0.7, freq: 1.6, gradient: 0 },
      }),
      ...band(aspect, {
        name: 'night low',
        y0: 0.77,
        y1: 1.5,
        soft: 0.06,
        body: P.plumBlack,
        bodyB: P.maroon,
        mix: { base: 0.25, vary: 0.6, freq: 1.3, gradient: -0.4 },
      }),
      seam('ember drag', P.emberGouache, 0.73, 6),
      ...drags(),
      lightDrag,
    ],
    finish: { ...pkFinish, vignette: 0.06, lift: '#120d10', liftAmount: 0.03 },
  }),
}

const pkEmber: Preset = {
  id: 'pk-ember',
  title: 'Dragged bands · Ember',
  note: 'PathKeep orange through the whole heat scale: apricot above, a broad orange band, burnt sienna, peach at the foot.',
  build: (aspect) => ({
    overscan: 0.05,
    support: ribbedLinen,
    layers: [
      groundField(P.lightRed, aspect, 0.4),
      ...band(aspect, {
        name: 'apricot',
        y0: -0.5,
        y1: 0.3,
        soft: 0.1,
        body: P.wallApricot,
        bodyB: P.heat,
        mix: { base: 0.35, vary: 0.6, freq: 1.3, gradient: 0.6 },
        scumble: P.naples,
        scumbleDry: 0.66,
      }),
      ...band(aspect, {
        name: 'orange',
        y0: 0.26,
        y1: 0.66,
        soft: 0.09,
        body: P.cadOrange,
        bodyB: P.heat,
        mix: { base: 0.3, vary: 0.6, freq: 1.4, gradient: -0.6 },
      }),
      ...band(aspect, {
        name: 'sienna',
        y0: 0.63,
        y1: 0.86,
        soft: 0.07,
        body: P.emberRust,
        bodyB: P.cadOrange,
        mix: { base: 0.3, vary: 0.6, freq: 1.4, gradient: -0.4 },
      }),
      ...band(aspect, {
        name: 'peach foot',
        y0: 0.86,
        y1: 1.5,
        soft: 0.05,
        body: P.wallPeach,
        bodyB: P.heat,
        mix: { base: 0.3, vary: 0.6, freq: 1.4, gradient: 0 },
      }),
      seam('apricot drag', P.apricot, 0.29, 5),
      seam('peach drag', P.wallPeach, 0.86, 5),
      ...drags(),
      lightDrag,
    ],
    finish: pkFinish,
  }),
}

const pkSpaces: Preset = {
  id: 'pk-spaces',
  title: 'Dragged bands · Two spaces',
  note: 'Agent teal and browsing orange: deep teal above, a pale cool band, the orange band, dark teal again at the foot.',
  build: (aspect) => ({
    overscan: 0.05,
    support: ribbedLinen,
    layers: [
      groundField(P.deepTeal, aspect, 0.5),
      ...band(aspect, {
        name: 'teal',
        y0: -0.5,
        y1: 0.48,
        soft: 0.1,
        body: P.deepTeal,
        bodyB: P.tealGouache,
        mix: { base: 0.3, vary: 0.6, freq: 1.3, gradient: 0.8 },
        glaze: P.teal,
      }),
      ...band(aspect, {
        name: 'cool haze',
        y0: 0.44,
        y1: 0.57,
        soft: 0.055,
        body: P.haze,
        bodyB: P.wallBlue,
        mix: { base: 0.4, vary: 0.6, freq: 1.5, gradient: 0 },
        base: 0.8,
      }),
      ...band(aspect, {
        name: 'orange',
        y0: 0.56,
        y1: 0.74,
        soft: 0.075,
        body: P.cadOrange,
        bodyB: P.emberRust,
        mix: { base: 0.35, vary: 0.6, freq: 1.4, gradient: 0.5 },
        glaze: P.pyrrole,
      }),
      ...band(aspect, {
        name: 'teal foot',
        y0: 0.72,
        y1: 1.5,
        soft: 0.09,
        body: P.deepTeal,
        bodyB: P.indigoDeep,
        mix: { base: 0.3, vary: 0.6, freq: 1.3, gradient: 0.5 },
      }),
      seam('haze drag', P.haze, 0.5, 5),
      seam('orange drag', P.emberGouache, 0.66, 5),
      ...drags(),
      lightDrag,
    ],
    finish: pkFinish,
  }),
}

const pkPaper: Preset = {
  id: 'pk-paper',
  title: 'Dragged bands · Paper',
  note: 'Quiet and editorial: cool bone above, a grey drag, one thin orange line, warm bone below.',
  build: (aspect) => ({
    overscan: 0.05,
    support: ribbedLinen,
    layers: [
      groundField(P.buff, aspect, 0.35),
      ...band(aspect, {
        name: 'cool bone',
        y0: -0.5,
        y1: 0.55,
        soft: 0.12,
        body: P.bone,
        bodyB: P.wallBlue,
        mix: { base: 0.3, vary: 0.6, freq: 1.2, gradient: -0.5 },
        scumble: P.white,
        scumbleDry: 0.62,
      }),
      ...band(aspect, {
        name: 'grey',
        y0: 0.5,
        y1: 0.62,
        soft: 0.05,
        body: P.ash,
        base: 0.55,
      }),
      ...band(aspect, {
        name: 'ember line',
        y0: 0.632,
        y1: 0.672,
        soft: 0.014,
        x: [-0.5, 1.05],
        softX: 0.4,
        body: P.cadOrange,
        base: 1.05,
      }),
      ...band(aspect, {
        name: 'warm bone',
        y0: 0.66,
        y1: 1.5,
        soft: 0.1,
        body: P.bone,
        bodyB: P.naples,
        mix: { base: 0.25, vary: 0.6, freq: 1.3, gradient: 0.3 },
        scumble: P.white,
        scumbleDry: 0.66,
      }),
      seam('grey drag', P.ash, 0.56, 4),
      ...drags(),
      seam('ember line drag', P.emberGouache, 0.652, 4),
      lightDrag,
    ],
    finish: { ...pkFinish, lift: '#3a3230', liftAmount: 0.01 },
  }),
}

/** Small painted swatches for the two spaces, in the same dragged manner. */
const swBrowsing: Preset = {
  id: 'sw-browsing',
  title: 'Swatch · Browsing history',
  note: 'Browsing orange: heat scale bands dragged sideways.',
  build: (aspect) => ({
    overscan: 0.05,
    support: ribbedLinen,
    layers: [
      groundField(P.lightRed, aspect, 0.4),
      ...band(aspect, {
        name: 'apricot',
        y0: -0.5,
        y1: 0.36,
        soft: 0.1,
        body: P.wallApricot,
        bodyB: P.heat,
        mix: { base: 0.4, vary: 0.6, freq: 1.6, gradient: 0.6 },
      }),
      ...band(aspect, {
        name: 'orange',
        y0: 0.3,
        y1: 0.74,
        soft: 0.1,
        body: P.cadOrange,
        bodyB: P.heat,
        mix: { base: 0.3, vary: 0.6, freq: 1.6, gradient: -0.5 },
      }),
      ...band(aspect, {
        name: 'ember',
        y0: 0.7,
        y1: 1.5,
        soft: 0.08,
        body: P.emberRust,
        bodyB: P.cadOrange,
        mix: { base: 0.4, vary: 0.6, freq: 1.6, gradient: 0 },
      }),
      ...drags(),
      lightDrag,
    ],
    finish: pkFinish,
  }),
}

const swAgent: Preset = {
  id: 'sw-agent',
  title: 'Swatch · Agent conversations',
  note: 'Agent teal: deep teal, a lighter teal band, dragged sideways.',
  build: (aspect) => ({
    overscan: 0.05,
    support: ribbedLinen,
    layers: [
      groundField(P.deepTeal, aspect, 0.5),
      ...band(aspect, {
        name: 'teal haze',
        y0: -0.5,
        y1: 0.34,
        soft: 0.1,
        body: P.tealGouache,
        bodyB: P.haze,
        mix: { base: 0.3, vary: 0.6, freq: 1.6, gradient: 0.6 },
      }),
      ...band(aspect, {
        name: 'teal',
        y0: 0.3,
        y1: 0.72,
        soft: 0.1,
        body: P.deepTeal,
        bodyB: P.tealGouache,
        mix: { base: 0.35, vary: 0.6, freq: 1.6, gradient: -0.5 },
      }),
      ...band(aspect, {
        name: 'deep',
        y0: 0.68,
        y1: 1.5,
        soft: 0.08,
        body: P.deepTeal,
        bodyB: P.indigoDeep,
        mix: { base: 0.3, vary: 0.6, freq: 1.6, gradient: 0 },
      }),
      ...drags(),
      lightDrag,
    ],
    finish: pkFinish,
  }),
}

export const presets: Record<string, Preset> = Object.fromEntries(
  [
    cfDawn,
    cfEmber,
    cfSpaces,
    cfFresco,
    wcDawn,
    wcDusk,
    wcTide,
    wcMist,
    wcStain,
    pkDawn,
    pkDusk,
    pkEmber,
    pkSpaces,
    pkPaper,
    swBrowsing,
    swAgent,
  ].map((p) => [p.id, p]),
)

export type { Layer }
