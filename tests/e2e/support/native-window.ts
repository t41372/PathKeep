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
 * minimizes or moves. On a display it may change (CI's, or one with none)
 * it runs mutter, GNOME's, then xfwm4, Xfce's. Anywhere else the specs that
 * use it are skipped, also when the app's window is not on the X display (a
 * Wayland session) or no window manager can be had; where the environment
 * says it provides them (`PATHKEEP_E2E_NATIVE_WINDOW=required`, CI's gate) a
 * missing piece fails them instead, so the check cannot quietly stop running.
 *
 * The window worked is the one of the app this run started, found by its
 * process: a PathKeep someone has open on the same display is never touched.
 *
 * Presses are held for 60 ms, as a person's are, except a tap's: released at
 * once, as a trackpad tap is (`expectTapsDoNotDrag`).
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

/** The running window manager's name, or `''` while none runs. */
function windowManager() {
  try {
    return /^Name: (.*)$/m.exec(run('wmctrl', ['-m']))?.[1] ?? ''
  } catch {
    return ''
  }
}

/**
 * Whether this run may replace the display's window manager. CI's gate says
 * the display is its own (`PATHKEEP_E2E_NATIVE_WINDOW=required`, under
 * xvfb-run), however many runs came before on it; elsewhere only a display
 * with no window manager when the spec starts is, so a person's own desktop
 * keeps theirs.
 */
let mayReplace = process.env.PATHKEEP_E2E_NATIVE_WINDOW === 'required'

/** Runs `command` as the window manager, unless one called `name` already runs. */
async function replaceWindowManager(
  command: string,
  args: string[],
  name: RegExp,
) {
  if (name.test(windowManager())) return
  if (!has(command)) throw new Error(`${command} is not installed.`)
  spawn(command, args, { detached: true, stdio: 'ignore' }).unref()
  await expect
    .poll(windowManager, { timeout: 30_000, intervals: [500] })
    .toMatch(name)
}

/**
 * Puts mutter, GNOME's window manager, on a display this run may change;
 * a display with a window manager of its own keeps it.
 */
export async function useMutter() {
  if (!mayReplace) {
    if (windowManager()) return
    mayReplace = true
  }
  await replaceWindowManager('mutter', ['--x11', '--replace'], /mutter/i)
}

/**
 * Swaps in xfwm4, Xfce's window manager. Returns why it did not, with
 * nothing changed: on a display with a window manager of its own (a person's
 * desktop), or without xfwm4.
 */
export async function switchToXfwm4(): Promise<string | null> {
  if (!mayReplace) return 'the display has a window manager of its own'
  if (!has('xfwm4')) return 'xfwm4 is not installed'
  await replaceWindowManager(
    'xfwm4',
    ['--replace', '--compositor=off'],
    /xfwm4/i,
  )
  return null
}

/** The visible main window of process `pid` on this display, or `''`. */
function windowOf(pid: number) {
  try {
    return (
      run('xdotool', [
        'search',
        '--all',
        '--onlyvisible',
        '--pid',
        String(pid),
        '--name',
        '^PathKeep$',
      ]).split('\n')[0] ?? ''
    )
  } catch {
    return ''
  }
}

/**
 * The id of the main window of the app this run started (process `pid`),
 * once it is on screen. Never another PathKeep that happens to be open on the
 * display: every press, drag and the final Close goes to this window.
 */
export async function appWindow(pid: number): Promise<string> {
  await expect
    .poll(() => windowOf(pid), { timeout: 120_000, intervals: [1_000] })
    .not.toBe('')
  return windowOf(pid)
}

/**
 * Whether the app's window shows up on this X display within a short while.
 * It does not where the app is a native Wayland window beside an XWayland
 * display that has the tools.
 */
export async function appWindowIsHere(pid: number) {
  for (let waited = 0; waited < 30; waited += 1) {
    if (windowOf(pid)) return true
    await new Promise((resolve) => setTimeout(resolve, 1_000))
  }
  return false
}

/** Whether a window manager runs here, or mutter can be started. */
export function windowManagerAvailable() {
  return windowManager() !== '' || has('mutter')
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

/** A press released at once, as a trackpad tap is: xdotool's own click. */
async function nativeTap(x: number, y: number) {
  run('xdotool', ['mousemove', String(x), String(y)])
  await pause(200)
  run('xdotool', ['click', '1'])
}

/**
 * Drags by (dx, dy), as a hand does: a small first move, then the rest. The
 * window follows from the first move on (`window-chrome/drag.ts` waits for
 * it), so it moves by up to that first move less than the pointer.
 */
export async function nativeDrag(x: number, y: number, dx: number, dy: number) {
  run('xdotool', ['mousemove', String(x), String(y)])
  await pause(200)
  run('xdotool', ['mousedown', '1'])
  await pause(250)
  run('xdotool', ['mousemove', String(x + 6), String(y + 4)])
  await pause(150)
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
    (n) =>
      Math.abs(n.x - (s.x + 120)) <= 10 && Math.abs(n.y - (s.y + 80)) <= 10,
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

/**
 * A tap on the window background does not leave the window following the
 * pointer (BACKLOG `WORK-DRAG-ON-MOVE`): after each tap the pointer moves with
 * no button down, and the window stays where it was. A drag asked for on the
 * press reaches xfwm4 after a tap's release only some of the time, so this
 * taps three spots twice each. Leaves the window reset.
 */
export async function expectTapsDoNotDrag(id: string) {
  const spots = (s: NativeState) => [
    { where: 'strip beside the window buttons', x: s.x + s.width - 200 },
    { where: 'middle of the top strip', x: s.x + Math.round(s.width / 2) },
    { where: 'left of the top strip', x: s.x + 120 },
  ]
  const followed: string[] = []
  for (const { where } of spots(nativeState(id)))
    for (let tap = 1; tap <= 2; tap++) {
      await resetWindow(id)
      const before = nativeState(id)
      const spot = spots(before).find((each) => each.where === where)!
      await nativeTap(spot.x, before.y + 16)
      await pause(800)
      run('xdotool', ['mousemove_relative', '--', '150', '90'])
      await pause(1_200)
      const after = nativeState(id)
      // Ends a move the window manager may be in; nothing else takes Escape.
      run('xdotool', ['key', 'Escape'])
      if (after.x !== before.x || after.y !== before.y)
        followed.push(
          `${where}, tap ${tap}: (${before.x}, ${before.y}) → (${after.x}, ${after.y})`,
        )
    }
  expect(followed, 'taps after which the window followed the pointer').toEqual(
    [],
  )
  await resetWindow(id)
}

/** Where the Close button is. */
export function closeButton(id: string) {
  const s = nativeState(id)
  return { x: s.x + s.width - 22, y: s.y + 16 }
}
