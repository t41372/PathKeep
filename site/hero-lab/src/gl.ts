/**
 * WebGL2 plumbing for the hero painter.
 *
 * Responsibilities: float render targets, programs whose uniforms are found
 * by introspection, a fullscreen pass that can be split into tiles, and a GPU
 * fence so a long pass never runs as one multi-second draw (the browser drops
 * the context when a single draw takes too long).
 * Not responsible for: what any pass paints. That lives in `painter.ts`.
 */

export type GL = WebGL2RenderingContext

export const HEADER = `#version 300 es
precision highp float;
precision highp int;
precision highp sampler2D;
`

export interface Target {
  tex: WebGLTexture
  fbo: WebGLFramebuffer
  width: number
  height: number
}

export function createTarget(
  gl: GL,
  width: number,
  height: number,
  internalFormat: number,
): Target {
  const tex = gl.createTexture()
  const fbo = gl.createFramebuffer()
  if (!tex || !fbo) throw new Error('could not allocate a render target')
  gl.bindTexture(gl.TEXTURE_2D, tex)
  gl.texStorage2D(gl.TEXTURE_2D, 1, internalFormat, width, height)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
  gl.bindFramebuffer(gl.FRAMEBUFFER, fbo)
  gl.framebufferTexture2D(
    gl.FRAMEBUFFER,
    gl.COLOR_ATTACHMENT0,
    gl.TEXTURE_2D,
    tex,
    0,
  )
  const status = gl.checkFramebufferStatus(gl.FRAMEBUFFER)
  if (status !== gl.FRAMEBUFFER_COMPLETE)
    throw new Error(`framebuffer incomplete (0x${status.toString(16)})`)
  gl.bindFramebuffer(gl.FRAMEBUFFER, null)
  return { tex, fbo, width, height }
}

export type UniformValue = number | readonly number[]

export interface Program {
  prog: WebGLProgram
  set: (values: Record<string, UniformValue>) => void
  textures: (bindings: Record<string, WebGLTexture>) => void
  attrib: (name: string) => number
}

function compile(gl: GL, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type)
  if (!shader) throw new Error('could not create a shader')
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? ''
    const lines = source.split('\n')
    const wanted = new Set<number>()
    for (const m of log.matchAll(/ERROR: \d+:(\d+)/g)) {
      const n = Number(m[1])
      for (let i = n - 3; i <= n + 2; i++) wanted.add(i)
    }
    const context = [...wanted]
      .filter((n) => n >= 1 && n <= lines.length)
      .sort((a, b) => a - b)
      .map((n) => `${String(n).padStart(4)}| ${lines[n - 1]}`)
      .join('\n')
    throw new Error(`shader compile failed:\n${log}\n${context}`)
  }
  return shader
}

export function createProgram(
  gl: GL,
  vertexSource: string,
  fragmentSource: string,
): Program {
  const prog = gl.createProgram()
  if (!prog) throw new Error('could not create a program')
  gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, vertexSource))
  gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, fragmentSource))
  gl.linkProgram(prog)
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    throw new Error(`program link failed: ${gl.getProgramInfoLog(prog)}`)
  }

  const setters = new Map<string, (v: UniformValue) => void>()
  const samplers: string[] = []
  const count = gl.getProgramParameter(prog, gl.ACTIVE_UNIFORMS) as number
  for (let i = 0; i < count; i++) {
    const info = gl.getActiveUniform(prog, i)
    if (!info) continue
    const name = info.name.replace(/\[0\]$/, '')
    const loc = gl.getUniformLocation(prog, info.name)
    if (!loc) continue
    const arr = (v: UniformValue) =>
      typeof v === 'number' ? [v] : (v as number[])
    switch (info.type) {
      case gl.FLOAT:
        setters.set(name, (v) => gl.uniform1fv(loc, arr(v)))
        break
      case gl.FLOAT_VEC2:
        setters.set(name, (v) => gl.uniform2fv(loc, arr(v)))
        break
      case gl.FLOAT_VEC3:
        setters.set(name, (v) => gl.uniform3fv(loc, arr(v)))
        break
      case gl.FLOAT_VEC4:
        setters.set(name, (v) => gl.uniform4fv(loc, arr(v)))
        break
      case gl.INT:
      case gl.BOOL:
        setters.set(name, (v) => gl.uniform1iv(loc, arr(v)))
        break
      case gl.SAMPLER_2D:
        samplers.push(name)
        setters.set(name, (v) => gl.uniform1i(loc, v as number))
        break
      default:
        throw new Error(
          `uniform ${name} has a type this painter does not set (0x${info.type.toString(16)})`,
        )
    }
  }

  return {
    prog,
    set(values) {
      gl.useProgram(prog)
      for (const [name, value] of Object.entries(values)) {
        const setter = setters.get(name)
        if (setter) setter(value)
      }
    },
    textures(bindings) {
      gl.useProgram(prog)
      let unit = 0
      for (const name of samplers) {
        const tex = bindings[name]
        if (!tex) throw new Error(`sampler ${name} has no texture bound`)
        gl.activeTexture(gl.TEXTURE0 + unit)
        gl.bindTexture(gl.TEXTURE_2D, tex)
        setters.get(name)?.(unit)
        unit++
      }
    },
    attrib(name) {
      return gl.getAttribLocation(prog, name)
    },
  }
}

/** Waits for the GPU to finish what has been queued, without blocking the page. */
export async function gpuSync(gl: GL): Promise<void> {
  const sync = gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE, 0)
  if (!sync) {
    gl.finish()
    return
  }
  gl.flush()
  for (;;) {
    const status = gl.clientWaitSync(sync, 0, 0)
    if (status === gl.ALREADY_SIGNALED || status === gl.CONDITION_SATISFIED)
      break
    if (status === gl.WAIT_FAILED) throw new Error('GPU wait failed')
    await new Promise((resolve) => setTimeout(resolve, 1))
  }
  gl.deleteSync(sync)
  if (gl.isContextLost()) throw new Error('WebGL context lost')
}

export const FULLSCREEN_VS = `${HEADER}
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
`

/** Draws a fullscreen triangle into `target`, split into `tiles` × `tiles` scissored pieces. */
export async function fullscreen(
  gl: GL,
  target: Target | null,
  width: number,
  height: number,
  tiles = 1,
): Promise<void> {
  gl.bindFramebuffer(gl.FRAMEBUFFER, target ? target.fbo : null)
  gl.viewport(0, 0, width, height)
  if (tiles <= 1) {
    gl.drawArrays(gl.TRIANGLES, 0, 3)
    await gpuSync(gl)
    return
  }
  gl.enable(gl.SCISSOR_TEST)
  const tw = Math.ceil(width / tiles)
  const th = Math.ceil(height / tiles)
  for (let ty = 0; ty < tiles; ty++) {
    for (let tx = 0; tx < tiles; tx++) {
      gl.scissor(tx * tw, ty * th, tw, th)
      gl.drawArrays(gl.TRIANGLES, 0, 3)
      await gpuSync(gl)
    }
  }
  gl.disable(gl.SCISSOR_TEST)
}
