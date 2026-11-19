/**
 * The demo's entry: open in the landing page's language, load the
 * recording, start the clock where the recording was made, then start the
 * app itself, and tell the landing page once a screen is up. A recording
 * run starts the app straight away, on the real clock, against the real
 * backend.
 */
import { takeRecording, type Recording } from './backend'

/** The app keeps its language here (`src/lib/i18n/provider.tsx`). */
const LANGUAGE_KEY = 'pathkeep-language-preference'

/**
 * Shifts the page's clock so it reads `at` now and runs on from there, at
 * the real clock's pace. The recorded answers were computed on that day;
 * the app asks for "today" and "the last 7 days" by its own clock, and has
 * to ask for the same days.
 *
 * Written here rather than taken from @sinonjs/fake-timers: its clock moves
 * in fixed steps on a timer (20 ms by default, slower in a background tab),
 * which makes the app's charts stutter. This only needs a subclass and a
 * Proxy, the same in every browser.
 */
function startClockAt(at: number) {
  const Real = Date
  const offset = at - Real.now()
  class DemoDate extends Real {
    constructor(...args: ConstructorParameters<DateConstructor> | []) {
      if (args.length === 0) super(Real.now() + offset)
      else super(...(args as ConstructorParameters<DateConstructor>))
    }
    static override now() {
      return Real.now() + offset
    }
    // Dates the browser makes itself (structuredClone, for one) are still dates.
    static [Symbol.hasInstance](value: unknown) {
      return value instanceof Real
    }
  }
  globalThis.Date = new Proxy(DemoDate, {
    // `Date()` called without `new` is a string of now.
    apply: () => new DemoDate().toString(),
  }) as unknown as DateConstructor
}

void (async () => {
  const lang = new URLSearchParams(location.search).get('lang')
  if (lang === 'en' || lang === 'zh-CN' || lang === 'zh-TW') {
    try {
      localStorage.setItem(LANGUAGE_KEY, lang)
    } catch {
      // Without storage the app follows the browser's language.
    }
  }
  if (import.meta.env.MODE !== 'record') {
    const recording = (await (
      await fetch('./recording/index.json')
    ).json()) as Recording
    takeRecording(recording)
    startClockAt(Date.parse(recording.recordedAt))
  }
  await import('@/main')
  if (parent === window) return
  // The landing page shows the window's glass until the app has drawn a screen.
  const root = document.getElementById('root')
  const drawn = new MutationObserver(() => {
    if (!root?.querySelector('main')) return
    drawn.disconnect()
    requestAnimationFrame(() =>
      requestAnimationFrame(() =>
        parent.postMessage('pathkeep-demo-ready', location.origin),
      ),
    )
  })
  if (root) drawn.observe(root, { childList: true, subtree: true })
})()
