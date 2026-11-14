/**
 * PathKeep's real desktop window, the one the E2E launcher starts beside the
 * browser the other specs drive, worked with real input on a Linux X display.
 * The pointer goes through xdotool, and the window manager's answer is read
 * back from the window (`_NET_WM_STATE`, its geometry), so what is checked is
 * the whole way from a press on the WebKitGTK page through Tauri's IPC and
 * permissions to the window manager.
 *
 * Needs Linux, an X display (CI runs the gate under `xvfb-run`), xdotool,
 * xprop and wmctrl, and a window manager: without one nothing maximizes,
 * minimizes or moves, so if none is running this starts mutter, GNOME's.
 * Anywhere else the specs that use it are skipped; where the environment
 * says it provides them (`PATHKEEP_E2E_NATIVE_WINDOW=required`, CI's gate) a
 * missing piece fails them instead, so the check cannot quietly stop running.
 *
 * Presses are held for 60 ms, as a person's are. A press released at once
 * (xdotool's own click) can reach the window manager after the release, and
 * some window managers then move the window until the next click
 * (BACKLOG `WORK-DRAG-ON-MOVE`).
 */
import { execFileSync, spawn } from 'node:child_process'
import { expect } from '@playwright/test'

const run = (command: string, args: string[]) =>
  execFileSync(command, args, { encoding: 'utf8', timeout: 20_000 }).trim()

function has(command: string) {
  try {
    run('sh', ['-c', `command -v ${command}`])
    return true
  } catch {
    return false
  }
}

const tools = ['xdotool', 'xprop', 'wmctrl']

/** Why the real window cannot be worked here, or `null` when it can. */
export function nativeWindowMissing(): string | null {
  if (process.platform !== 'linux') return 'needs Linux'
  if (!process.env.DISPLAY) return 'needs an X display'
  const missing = tools.filter((tool) => !has(tool))
  return missing.length ? `needs ${missing.join(', ')}` : null
}

function windowManagerRunning() {
  try {
    run('wmctrl', ['-m'])
    return true
  } catch {
    return false
  }
}

/** Starts mutter unless a window manager already runs on the display. */
export async function ensureWindowManager() {
  if (windowManagerRunning()) return
  if (!has('mutter'))
    throw new Error(
      'No window manager runs on the display, and mutter is not installed.',
    )
  spawn('mutter', ['--x11', '--replace'], {
    detached: true,
    stdio: 'ignore',
  }).unref()
  await expect
    .poll(windowManagerRunning, { timeout: 30_000, intervals: [500] })
    .toBe(true)
}

/** The id of PathKeep's main window once it is on screen. */
export async function appWindow(): Promise<string> {
  let id = ''
  await expect
    .poll(
      () => {
        try {
          id =
            run('xdotool', [
              'search',
              '--onlyvisible',
              '--name',
              '^PathKeep$',
            ]).split('\n')[0] ?? ''
        } catch {
          id = ''
        }
        return id
      },
      { timeout: 120_000, intervals: [1_000] },
    )
    .not.toBe('')
  return id
}

export type NativeState = {
  x: number
  y: number
  width: number
  height: number
  maximized: boolean
  minimized: boolean
}

export function nativeState(id: string): NativeState {
  const shell = run('xdotool', ['getwindowgeometry', '--shell', id])
  const value = (key: string) =>
    Number(new RegExp(`^${key}=(\\d+)`, 'm').exec(shell)?.[1])
  const flags = run('xprop', ['-id', id, '_NET_WM_STATE'])
  return {
    x: value('X'),
    y: value('Y'),
    width: value('WIDTH'),
    height: value('HEIGHT'),
    maximized:
      flags.includes('_NET_WM_STATE_MAXIMIZED_HORZ') &&
      flags.includes('_NET_WM_STATE_MAXIMIZED_VERT'),
    minimized: flags.includes('_NET_WM_STATE_HIDDEN'),
  }
}

/** Waits until the window is in the state `check` looks for. */
export async function expectNativeState(
  id: string,
  check: (state: NativeState) => boolean,
  what: string,
) {
  await expect
    .poll(() => check(nativeState(id)), {
      message: `${what}: window is ${JSON.stringify(nativeState(id))}`,
      timeout: 10_000,
      intervals: [250],
    })
    .toBe(true)
}

/** Back to the smallest window size at a known place, restored and in front. */
export async function resetWindow(id: string) {
  run('wmctrl', ['-i', '-r', id, '-b', 'remove,maximized_vert,maximized_horz'])
  run('xdotool', ['windowmap', id])
  run('xdotool', ['windowsize', id, '1080', '760'])
  run('xdotool', ['windowmove', id, '40', '40'])
  run('xdotool', ['windowactivate', '--sync', id])
  await expectNativeState(
    id,
    (s) =>
      !s.maximized &&
      !s.minimized &&
      Math.abs(s.x - 40) <= 4 &&
      Math.abs(s.y - 40) <= 4 &&
      s.width === 1080,
    'reset',
  )
}

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

async function press() {
  run('xdotool', ['mousedown', '1'])
  await pause(60)
  run('xdotool', ['mouseup', '1'])
}

export async function nativeClick(x: number, y: number) {
  run('xdotool', ['mousemove', String(x), String(y)])
  await pause(200)
  await press()
}

export async function nativeDoubleClick(x: number, y: number) {
  run('xdotool', ['mousemove', String(x), String(y)])
  await pause(200)
  await press()
  await pause(120)
  await press()
}

export async function nativeDrag(x: number, y: number, dx: number, dy: number) {
  run('xdotool', ['mousemove', String(x), String(y)])
  await pause(200)
  run('xdotool', ['mousedown', '1'])
  await pause(250)
  for (let step = 1; step <= 4; step++) {
    run('xdotool', [
      'mousemove',
      String(x + (dx * step) / 4),
      String(y + (dy * step) / 4),
    ])
    await pause(80)
  }
  await pause(200)
  run('xdotool', ['mouseup', '1'])
}

/**
 * The window's own buttons and the strip beside them, worked as a person
 * would, on whatever screen the window shows. Leaves the window reset.
 */
export async function expectWorkingWindowFrame(id: string) {
  await resetWindow(id)
  const top = (s: NativeState) => s.y + 16
  const maximize = (s: NativeState) => s.x + s.width - 66
  const minimize = (s: NativeState) => s.x + s.width - 110
  const strip = (s: NativeState) => s.x + s.width - 200

  // The window shows before its page has drawn anything: press Maximize until
  // the page is there to take it (nothing else lies under that spot).
  let s = nativeState(id)
  await expect
    .poll(
      async () => {
        await nativeClick(maximize(s), top(s))
        await pause(1_500)
        return nativeState(id).maximized
      },
      {
        message: 'Maximize never took effect',
        timeout: 90_000,
        intervals: [0],
      },
    )
    .toBe(true)
  s = nativeState(id)
  await nativeClick(maximize(s), top(s))
  await expectNativeState(id, (n) => !n.maximized, 'after Restore')

  await resetWindow(id)
  s = nativeState(id)
  await nativeClick(minimize(s), top(s))
  await expectNativeState(id, (n) => n.minimized, 'after Minimize')

  await resetWindow(id)
  s = nativeState(id)
  await nativeDrag(strip(s), top(s), 120, 80)
  await expectNativeState(
    id,
    (n) => Math.abs(n.x - (s.x + 120)) <= 4 && Math.abs(n.y - (s.y + 80)) <= 4,
    'after dragging the strip by (120, 80)',
  )

  await resetWindow(id)
  s = nativeState(id)
  await nativeDoubleClick(strip(s), top(s))
  await expectNativeState(
    id,
    (n) => n.maximized,
    'after a double click on the strip',
  )
  s = nativeState(id)
  await nativeDoubleClick(strip(s), top(s))
  await expectNativeState(
    id,
    (n) => !n.maximized,
    'after a second double click',
  )
  await resetWindow(id)
}

/** Where the Close button is. */
export function closeButton(id: string) {
  const s = nativeState(id)
  return { x: s.x + s.width - 22, y: s.y + 16 }
}
