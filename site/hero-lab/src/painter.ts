/**
 * Paints one picture: lays the support, then each layer in order, then the
 * finishing pass, into a WebGL2 canvas.
 *
 * Responsibilities: running the passes and holding the surface between them
 * (reflectance + height, ping-ponged). Not responsible for: what the picture
 * is (`presets.ts`), where strokes go (`brushwork.ts`) or exporting files
 * (`main.ts`).
 */

import {
  FLOATS_PER_VERTEX,
  rng,
  strokeMesh,
  type Rand,
  type Region,
  type Stroke,
} from './brushwork'
import {
  FULLSCREEN_VS,
  HEADER,
  createProgram,
  createTarget,
  fullscreen,
  gpuSync,
  type GL,
  type Target,
} from './gl'
import common from './glsl/common.glsl'
import compositeFs from './glsl/composite.frag'
import fieldFs from './glsl/field.frag'
import smearFs from './glsl/smear.frag'
import finishFs from './glsl/finish.frag'
import strokeFs from './glsl/stroke.frag'
import strokeVs from './glsl/stroke.vert'
import supportFs from './glsl/support.frag'
import washFs from './glsl/wash.frag'
import { linear, type Pigment } from './pigments'

export const REF_WIDTH = 3840

export interface Support {
  kind: 'linen' | 'paper' | 'plaster'
  base: string
  fleck: string
  scale?: number
  age?: number
  /** 0..1: horizontal threads heavier than vertical ones (a ribbed weave). */
  rib?: number
}

interface LayerBase {
  name: string
  pigment: Pigment
  /** Second pigment for wet mixing; the layer says where it takes over. */
  pigmentB?: Pigment
  thickness?: number
  impasto?: number
  wear?: { amount: number; size: number; height: number }
  losses?: number
  /** How much this layer fills the weave's valleys (thick paint evens the surface). */
  level?: number
}

export interface StrokeLayer extends LayerBase {
  kind: 'strokes'
  strokes: (r: Rand, aspect: number) => Stroke[]
  region?: Region
  /** Softness across x, softness across y, edge noise. */
  regionSoft?: readonly [number, number, number]
}

export interface WashLayer extends LayerBase {
  kind: 'wash'
  /** Top, bottom, left, right in composition space (ignored for a pool). */
  band?: readonly [number, number, number, number]
  tilt?: number
  edge?: { wander: number; freq: number; softTop: number; softBottom: number }
  side?: readonly [number, number]
  wetVary?: number
  density?: { base: number; mottle: number; pool: number; streaks: number }
  texture?: { granulation: number; flocculation: number; tide: number }
  bloom?: { count: number; radius: number; strength: number }
  mix?: { base: number; vary: number; freq: number; gradient: number }
  /** An organic pool instead of a band: centre, radii, rotation in degrees, how lobed. */
  pool?: {
    x: number
    y: number
    rx: number
    ry: number
    rotate?: number
    lobes?: number
  }
  /** Faint stain just outside dried edges, as on raw canvas. */
  fringe?: number
  /** An area the wash goes around, left for a later colour. */
  hole?: {
    x: number
    y: number
    rx: number
    ry: number
    soft?: number
    lobes?: number
  }
}

export interface FieldLayer extends LayerBase {
  kind: 'field'
  /** Composition space; push the edges past the canvas for a full-bleed field. */
  region: Region
  edge?: { softX: number; softY: number; wander: number; fray: number }
  paint?: { base: number; patchiness: number; streaks: number; dryness: number }
  /** Angle in degrees; streak width and length in reference px; share of cross strokes. */
  brush?: { angle: number; width: number; length: number; cross: number }
  rim?: { amount: number; width: number }
  mix?: { base: number; vary: number; freq: number; gradient: number }
}

/** Drags the wet colour sideways (squeegee, starved wide brush); moves colour only. */
export interface SmearLayer {
  kind: 'smear'
  name: string
  /** Degrees; 0 drags to the right, 180 to the left. */
  angle?: number
  /** How far colour is carried, in reference px. */
  length: number
  strength: number
  /** 0..1: how much of the canvas the blade touches. */
  coverage: number
  rows?: { width: number; contrast: number; span: number; wobble: number }
  /** Limit to a horizontal zone: centre y and half-height in composition space. */
  zone?: { y: number; half: number }
}

export type Layer = StrokeLayer | WashLayer | FieldLayer | SmearLayer

export interface Finish {
  relief: number
  gloss?: number
  crack?: number
  crackSize?: number
  vignette?: number
  saturation?: number
  tint?: string
  lift?: string
  liftAmount?: number
  grain?: number
}

export interface Painting {
  support: Support
  layers: Layer[]
  finish: Finish
  /** Extra canvas painted on every side and cropped away (share of the size). */
  overscan?: number
}

export interface PaintOptions {
  width: number
  height: number
  seed: number
  /** Paint this much extra on every side (share of the size) and let the caller crop it, so drags never smear a clamped edge. */
  overscan?: number
  log?: (line: string) => void
}

const SUPPORT_KIND = { linen: 0, paper: 1, plaster: 2 } as const
const STROKES_PER_DRAW = 24

export async function paint(
  canvas: HTMLCanvasElement,
  painting: Painting,
  o: PaintOptions,
): Promise<void> {
  const { seed } = o
  const m = o.overscan ?? 0
  const w = Math.round(o.width * (1 + 2 * m))
  const h = Math.round(o.height * (1 + 2 * m))
  const log = o.log ?? (() => {})
  canvas.width = w
  canvas.height = h
  const gl = canvas.getContext('webgl2', {
    alpha: false,
    antialias: false,
    depth: false,
    premultipliedAlpha: false,
    preserveDrawingBuffer: true,
    powerPreference: 'high-performance',
  }) as GL | null
  if (!gl) throw new Error('WebGL2 is not available')
  if (!gl.getExtension('EXT_color_buffer_float'))
    throw new Error('float render targets are not available')

  const ref = REF_WIDTH / o.width
  const aspect = w / h
  const tiles = Math.max(1, Math.ceil(Math.max(w, h) / 1400))
  const base = { uRes: [w, h], uRef: ref }

  const surface: [Target, Target] = [
    createTarget(gl, w, h, gl.RGBA32F),
    createTarget(gl, w, h, gl.RGBA32F),
  ]
  const layerTarget = createTarget(gl, w, h, gl.RGBA16F)
  let cur = 0

  const fs = (body: string) => `${HEADER}${common}\n${body}`
  const programs = {
    support: createProgram(gl, FULLSCREEN_VS, fs(supportFs)),
    wash: createProgram(gl, FULLSCREEN_VS, fs(washFs)),
    field: createProgram(gl, FULLSCREEN_VS, fs(fieldFs)),
    composite: createProgram(gl, FULLSCREEN_VS, fs(compositeFs)),
    finish: createProgram(gl, FULLSCREEN_VS, fs(finishFs)),
    smear: createProgram(gl, FULLSCREEN_VS, fs(smearFs)),
    stroke: createProgram(gl, `${HEADER}${strokeVs}`, fs(strokeFs)),
  }
  const emptyVao = gl.createVertexArray()
  const strokeVao = gl.createVertexArray()
  const strokeBuffer = gl.createBuffer()
  gl.bindVertexArray(strokeVao)
  gl.bindBuffer(gl.ARRAY_BUFFER, strokeBuffer)
  const stride = FLOATS_PER_VERTEX * 4
  const attributes: [string, number, number][] = [
    ['aPos', 2, 0],
    ['aST', 2, 8],
    ['aA', 4, 16],
    ['aB', 4, 32],
    ['aC', 4, 48],
    ['aDim', 2, 64],
  ]
  for (const [name, size, offset] of attributes) {
    const loc = programs.stroke.attrib(name)
    if (loc < 0) continue
    gl.enableVertexAttribArray(loc)
    gl.vertexAttribPointer(loc, size, gl.FLOAT, false, stride, offset)
  }

  const t0 = performance.now()
  const s = painting.support
  gl.bindVertexArray(emptyVao)
  programs.support.set({
    ...base,
    uSeed: seed,
    uKind: SUPPORT_KIND[s.kind],
    uBase: linear(s.base),
    uFleck: linear(s.fleck),
    uScale: s.scale ?? 1,
    uAge: s.age ?? 0,
    uRib: s.rib ?? 0,
  })
  await fullscreen(gl, surface[cur], w, h, tiles)
  log(`support ${(performance.now() - t0).toFixed(0)} ms`)

  const r = rng(seed * 7919 + 13)
  for (const [index, layer] of painting.layers.entries()) {
    const t = performance.now()
    const layerSeed = seed * 131 + index * 17 + 5
    if (layer.kind === 'smear') {
      gl.bindVertexArray(emptyVao)
      const rows = layer.rows ?? {
        width: 14,
        contrast: 1.2,
        span: 1400,
        wobble: 24,
      }
      programs.smear.set({
        ...base,
        uSeed: layerSeed,
        uDrag: [
          ((layer.angle ?? 0) * Math.PI) / 180,
          layer.length,
          layer.strength,
          layer.coverage,
        ],
        uRows: [rows.width, rows.contrast, rows.span, rows.wobble],
        uSpan: layer.zone ? [layer.zone.y, layer.zone.half] : [0, 0],
      })
      programs.smear.textures({ uSurface: surface[cur].tex })
      await fullscreen(gl, surface[1 - cur], w, h, tiles)
      cur = 1 - cur
      log(`${layer.name} ${(performance.now() - t).toFixed(0)} ms`)
      continue
    }
    if (layer.kind === 'field') {
      gl.bindVertexArray(emptyVao)
      const e = layer.edge ?? {
        softX: 0.03,
        softY: 0.03,
        wander: 0.025,
        fray: 0.02,
      }
      const pt = layer.paint ?? {
        base: 0.8,
        patchiness: 0.18,
        streaks: 0.25,
        dryness: 0,
      }
      const b = layer.brush ?? { angle: 0, width: 26, length: 900, cross: 0.25 }
      const rim = layer.rim ?? { amount: 0, width: 0.02 }
      const m = layer.mix ?? { base: 0, vary: 0, freq: 1, gradient: 0 }
      const rg = layer.region
      programs.field.set({
        ...base,
        uSeed: layerSeed,
        uRegion: [rg.x0, rg.y0, rg.x1, rg.y1],
        uEdge: [e.softX, e.softY, e.wander, e.fray],
        uPaint: [pt.base, pt.patchiness, pt.streaks, pt.dryness],
        uBrush: [(b.angle * Math.PI) / 180, b.width, b.length, b.cross],
        uRim: [rim.amount, rim.width],
        uMix: [m.base, m.vary, m.freq, m.gradient],
      })
      programs.field.textures({ uSurface: surface[cur].tex })
      await fullscreen(gl, layerTarget, w, h, tiles)
    } else if (layer.kind === 'wash') {
      gl.bindVertexArray(emptyVao)
      const e = layer.edge ?? {
        wander: 0.04,
        freq: 2,
        softTop: 0.5,
        softBottom: 0.5,
      }
      const d = layer.density ?? {
        base: 0.4,
        mottle: 0.4,
        pool: 0.2,
        streaks: 0.08,
      }
      const tx = layer.texture ?? {
        granulation: 0.3,
        flocculation: 0.3,
        tide: 0.7,
      }
      const b = layer.bloom ?? { count: 0, radius: 0.08, strength: 0.7 }
      const m = layer.mix ?? { base: 0, vary: 0, freq: 1, gradient: 0 }
      programs.wash.set({
        ...base,
        uSeed: layerSeed,
        uBand: layer.band ?? [-1, 2, -1, aspect + 1],
        uTilt: layer.tilt ?? 0,
        uEdge: [e.wander, e.freq, e.softTop, e.softBottom],
        uSide: layer.side ?? [0.6, 0.6],
        uWetVary: layer.wetVary ?? 0.4,
        uDensity: [d.base, d.mottle, d.pool, d.streaks],
        uTexture: [tx.granulation, tx.flocculation, tx.tide, 0],
        uBloom: [b.count, b.radius, b.strength, layerSeed + 0.5],
        uMix: [m.base, m.vary, m.freq, m.gradient],
        uShape: layer.pool ? 1 : 0,
        uPool: layer.pool
          ? [layer.pool.x, layer.pool.y, layer.pool.rx, layer.pool.ry]
          : [0, 0, 1, 1],
        uPoolShape: [
          ((layer.pool?.rotate ?? 0) * Math.PI) / 180,
          layer.pool?.lobes ?? 0.35,
        ],
        uFringe: layer.fringe ?? 0,
        uHole: layer.hole
          ? [layer.hole.x, layer.hole.y, layer.hole.rx, layer.hole.ry]
          : [0, 0, 0, 0],
        uHoleShape: [layer.hole?.soft ?? 0.25, layer.hole?.lobes ?? 0.3],
      })
      programs.wash.textures({ uSurface: surface[cur].tex })
      await fullscreen(gl, layerTarget, w, h, tiles)
    } else {
      const strokes = layer.strokes(r, aspect)
      gl.bindFramebuffer(gl.FRAMEBUFFER, layerTarget.fbo)
      gl.viewport(0, 0, w, h)
      gl.clearColor(0, 0, 0, 0)
      gl.clear(gl.COLOR_BUFFER_BIT)
      gl.enable(gl.BLEND)
      gl.blendEquation(gl.FUNC_ADD)
      gl.blendFunc(gl.ONE, gl.ONE)
      gl.bindVertexArray(strokeVao)
      const region = layer.region
      const soft = layer.regionSoft ?? [0.02, 0.02, 0.02]
      programs.stroke.set({
        ...base,
        uAspect: aspect,
        uRegion: region
          ? [region.x0, region.y0, region.x1, region.y1]
          : [1, 0, 0, 0],
        uRegionSoft: [soft[0], soft[1], soft[2], layerSeed + 0.25],
      })
      programs.stroke.textures({ uSurface: surface[cur].tex })
      for (let i = 0; i < strokes.length; i += STROKES_PER_DRAW) {
        const mesh = strokeMesh(
          strokes.slice(i, i + STROKES_PER_DRAW),
          REF_WIDTH / aspect,
        )
        gl.bindBuffer(gl.ARRAY_BUFFER, strokeBuffer)
        gl.bufferData(gl.ARRAY_BUFFER, mesh, gl.STREAM_DRAW)
        gl.drawArrays(gl.TRIANGLES, 0, mesh.length / FLOATS_PER_VERTEX)
        await gpuSync(gl)
      }
      gl.disable(gl.BLEND)
    }

    gl.bindVertexArray(emptyVao)
    const wear = layer.wear
    programs.composite.set({
      ...base,
      uKa: layer.pigment.K,
      uSa: layer.pigment.S,
      uKb: (layer.pigmentB ?? layer.pigment).K,
      uSb: (layer.pigmentB ?? layer.pigment).S,
      uThickness: layer.thickness ?? 1,
      uImpasto: layer.impasto ?? 0.04,
      uWear: wear
        ? [wear.amount, wear.size, wear.height, layerSeed + 0.75]
        : [0, 1, 1, 0],
      uLosses: layer.losses ?? 0,
      uLevel: layer.level ?? 0.8,
    })
    programs.composite.textures({
      uSurface: surface[cur].tex,
      uLayer: layerTarget.tex,
    })
    await fullscreen(gl, surface[1 - cur], w, h, 1)
    cur = 1 - cur
    log(`${layer.name} ${(performance.now() - t).toFixed(0)} ms`)
  }

  const f = painting.finish
  gl.bindVertexArray(emptyVao)
  programs.finish.set({
    ...base,
    uSeed: seed,
    uRelief: f.relief,
    uGloss: f.gloss ?? 0,
    uCrack: f.crack ?? 0,
    uCrackSize: f.crackSize ?? 80,
    uVignette: f.vignette ?? 0.1,
    uSaturation: f.saturation ?? 1,
    uTint: linear(f.tint ?? '#ffffff'),
    uLift: linear(f.lift ?? '#000000'),
    uLiftAmount: f.liftAmount ?? 0,
    uGrain: f.grain ?? 0.01,
  })
  programs.finish.textures({ uSurface: surface[cur].tex })
  await fullscreen(gl, null, w, h, tiles)
  log(`total ${(performance.now() - t0).toFixed(0)} ms`)
}
