/**
 * The window's frame as a person on Windows, Linux or macOS gets it, checked
 * on whatever screen the app shows. The page stands in for the desktop window
 * (`windowFrame` in `src/lib/runtime.ts`, set through the dev bridge's
 * `pathkeep-dev-desktop-window` key) of the platform the browser reports, so
 * the frame comes from the platform the same way it does in the app.
 *
 * What this proves: every screen has the window buttons where the title bar
 * would be, nothing covers them or sits under them, and the window
 * background marks where the window drags from: on macOS by the rules of
 * Tauri's `drag.js` (tauri 2.12, `src/window/scripts/drag.js`), on Windows and
 * Linux by PathKeep's own mark (`src/app/window-chrome/drag.ts`).
 *
 * What it cannot prove: what the buttons and the drag do to a real window
 * (the page has none). That is checked on a Linux desktop by hand
 * (`docs/design/back-forward-window-frame-tradeoff.md` §落地).
 */
import { expect, type Browser, type Page } from '@playwright/test'

export type FramePlatform = 'linux' | 'windows' | 'macos'

/** The smallest window `tauri.conf.json` allows. */
export const SMALLEST_WINDOW = { width: 1080, height: 760 }

const platforms: Record<
  FramePlatform,
  { navigatorPlatform: string; userAgent: string }
> = {
  linux: {
    navigatorPlatform: 'Linux x86_64',
    userAgent:
      'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15',
  },
  windows: {
    navigatorPlatform: 'Win32',
    userAgent:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0',
  },
  macos: {
    navigatorPlatform: 'MacIntel',
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)',
  },
}

/**
 * A page that stands in for PathKeep's window on `platform`, at the smallest
 * window size. Close its context when done.
 */
export async function desktopWindow(
  browser: Browser,
  platform: FramePlatform,
): Promise<Page> {
  const { navigatorPlatform, userAgent } = platforms[platform]
  const context = await browser.newContext({
    viewport: SMALLEST_WINDOW,
    userAgent,
    locale: 'en-US',
  })
  await context.addInitScript((value) => {
    Object.defineProperty(Navigator.prototype, 'platform', {
      get: () => value,
    })
    try {
      localStorage.setItem('pathkeep-dev-desktop-window', '1')
    } catch {
      // about:blank has no storage; the app's own pages do.
    }
  }, navigatorPlatform)
  return context.newPage()
}

/**
 * The window buttons, found even while a dialog hides the rest of the window
 * from assistive technology (as it should: the system's own title bar is not
 * part of the page either).
 */
export function windowButtons(page: Page) {
  return page.getByRole('group', { name: 'Window', includeHidden: true })
}

type ButtonName = 'Minimize' | 'Maximize' | 'Close'

function windowButton(page: Page, name: ButtonName) {
  return windowButtons(page).getByRole('button', {
    name,
    exact: true,
    includeHidden: true,
  })
}

/** Where a person would click the button, or `null` if it is not there. */
export async function windowButtonCenter(page: Page, name: ButtonName) {
  const box = await windowButton(page, name).boundingBox()
  return box ? { x: box.x + box.width / 2, y: box.y + box.height / 2 } : null
}

/**
 * Windows and Linux: one set of buttons at the top right, each one the thing a
 * click there lands on, and nothing of the screen under them. Soft: a walk
 * through several screens reports every screen that falls short.
 */
export async function expectWindowButtons(page: Page) {
  await expect.soft(windowButtons(page), 'window buttons').toHaveCount(1)
  if ((await windowButtons(page).count()) !== 1) return
  // Windows 11 shows its snap layouts over the button marked this way.
  await expect
    .soft(windowButton(page, 'Maximize'))
    .toHaveAttribute('data-window-maximize')

  const found = await page.evaluate(() => {
    const group = document.querySelector('[role="group"][aria-label="Window"]')
    if (!group) return ['no window buttons']
    const box = group.getBoundingClientRect()
    if (box.top !== 0 || Math.round(box.right) !== window.innerWidth)
      return [`window buttons at ${box.left},${box.top}, not the top right`]
    const problems: string[] = []
    // What a click on each button's center lands on.
    for (const button of group.querySelectorAll('button')) {
      const rect = button.getBoundingClientRect()
      const hit = document.elementFromPoint(
        rect.left + rect.width / 2,
        rect.top + rect.height / 2,
      )
      if (!hit || !button.contains(hit))
        problems.push(
          `${button.getAttribute('aria-label')} is covered by ${hit?.tagName.toLowerCase()}.${hit?.className}`,
        )
    }
    // What of the screen lies under the buttons.
    const content = document.querySelectorAll(
      'main, button, a[href], input, select, textarea, [role="button"], [role="link"], [role="progressbar"], h1, h2, h3, p',
    )
    for (const element of content) {
      if (group.contains(element)) continue
      const rect = element.getBoundingClientRect()
      if (rect.width === 0 || rect.height === 0) continue
      const style = getComputedStyle(element)
      if (style.visibility === 'hidden' || style.display === 'none') continue
      const apart =
        rect.right <= box.left ||
        rect.left >= box.right ||
        rect.bottom <= box.top ||
        rect.top >= box.bottom
      if (!apart)
        problems.push(
          `${element.tagName.toLowerCase()} "${(element.textContent ?? '').trim().slice(0, 40)}" under the buttons at ${Math.round(rect.left)},${Math.round(rect.top)}–${Math.round(rect.right)},${Math.round(rect.bottom)}`,
        )
    }
    return problems
  })
  expect.soft(found, 'window buttons covered or covering').toEqual([])
}

/** macOS: no buttons of ours; the traffic lights are the system's. */
export async function expectNoWindowButtons(page: Page) {
  await expect.soft(windowButtons(page), 'window buttons').toHaveCount(0)
}

/**
 * Whether a press at (x, y) drags the window. PathKeep's own mark (Windows,
 * Linux) drags only from the marked element itself. Otherwise Tauri's rule:
 * walk up from what is under the pointer; a control without the mark stops
 * it; `deep` drags from anywhere inside; a bare mark only from the marked
 * element itself.
 */
export async function dragsAt(page: Page, x: number, y: number) {
  return page.evaluate(
    ([px, py]) => {
      const own = document.elementFromPoint(px, py)
      if (own?.hasAttribute('data-window-drag')) return true
      const clickable = new Set([
        'A',
        'BUTTON',
        'INPUT',
        'SELECT',
        'TEXTAREA',
        'LABEL',
        'SUMMARY',
      ])
      const roles = new Set([
        'button',
        'link',
        'menuitem',
        'tab',
        'checkbox',
        'radio',
        'switch',
        'option',
      ])
      const target = document.elementFromPoint(px, py)
      for (
        let element: Element | null = target;
        element;
        element = element.parentElement
      ) {
        if (!(element instanceof HTMLElement)) continue
        const mark = element.getAttribute('data-tauri-drag-region')
        const isClickable =
          clickable.has(element.tagName) ||
          (element.hasAttribute('contenteditable') &&
            element.getAttribute('contenteditable') !== 'false') ||
          (element.hasAttribute('tabindex') &&
            element.getAttribute('tabindex') !== '-1') ||
          roles.has(element.getAttribute('role') ?? '')
        if (isClickable && mark === null) return false
        if (mark === null) continue
        if (mark === 'false') return false
        if (mark === 'deep') return true
        if (mark === '' || mark === 'true') return element === target
      }
      return false
    },
    [x, y] as const,
  )
}

/** Each point drags the window. */
export async function expectDragsAt(
  page: Page,
  points: { x: number; y: number; where: string }[],
) {
  const missing: string[] = []
  for (const { x, y, where } of points)
    if (!(await dragsAt(page, x, y))) missing.push(`${where} (${x}, ${y})`)
  expect.soft(missing, 'places that should drag the window').toEqual([])
}

/**
 * Windows and Linux: the strip of window background left of the buttons, as
 * a title bar would be.
 */
export function titleStrip(page: Page) {
  const width = page.viewportSize()?.width ?? SMALLEST_WINDOW.width
  return [
    { x: width - 132 - 24, y: 16, where: 'strip beside the window buttons' },
    { x: Math.round(width / 2), y: 16, where: 'middle of the top strip' },
  ]
}
