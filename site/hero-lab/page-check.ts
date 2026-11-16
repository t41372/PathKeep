/**
 * Checks the landing page as a visitor loads it (nothing forced), in every
 * language, at desktop, wide desktop and phone width: the opening card
 * starts full-screen, stays so while its window rises to the middle of
 * the screen, and is framed to the column the panels below hang in once
 * the window moves on (and rests framed with reduced motion or without
 * script), every in-page link
 * lands, nothing scrolls sideways, every picture and request loads, no
 * script errors, every section head and panel that rises into view ends up
 * visible (and is never hidden without script), the retention chart draws
 * itself when scrolled to, and the tabs swap the window. Also fails on wording the page dropped on
 * purpose: the paintings are decoration, never exhibits, and sections
 * carry no numbered labels. Needs the preview server on 4391.
 *
 *   bun site/hero-lab/page-check.ts            # in Chrome
 *   bun site/hero-lab/page-check.ts --webkit   # in WebKit, Safari's engine
 */
import { chromium, webkit } from '@playwright/test'

/**
 * Where the opening card shows. Its frame is a clip, and the clip clips hit
 * testing too, so this probes just inside and outside the edges of the
 * column the panels below hang in.
 */
const probeCard = () => {
  const card = document.querySelector('[data-intro]')!
  const column = document.querySelector('.spaces')!.getBoundingClientRect()
  const on = (x: number) => card.contains(document.elementFromPoint(x, 240))
  return {
    fullScreen: on(1) && on(document.documentElement.clientWidth - 2),
    framed:
      !on(column.left - 2) &&
      on(column.left + 2) &&
      on(column.right - 2) &&
      !on(column.right + 2),
  }
}

/** Section heads and panels still hidden by their entrance (see Page.astro). */
const hiddenParts = () =>
  Array.from(document.querySelectorAll<HTMLElement>('[data-reveal]'))
    .flatMap((el) =>
      el.dataset.reveal === 'group' ? Array.from(el.children) : [el],
    )
    .filter((el) => {
      const style = getComputedStyle(el)
      return Number(style.opacity) < 0.99 || style.transform !== 'none'
    })
    .map((el) => el.textContent?.trim().slice(0, 32) || el.className)

const PAGES = ['/', '/zh-tw/', '/zh-cn/']
const DROPPED = [
  /圖版|图版/,
  /\bPlates?\b/,
  /版權頁|版权页/,
  /Colophon/i,
  /#[0-9a-f]{6}\b/i,
  /\bpk-[a-z]+/,
  /WebP\b/,
  /\bseed\b/i,
  /種子|种子/,
  /\b0\d — /,
  // A product token the copy failed to turn into its mark (Marked.astro).
  /\{\w+:[^}]+\}/,
]

const browser = process.argv.includes('--webkit')
  ? await webkit.launch()
  : await chromium.launch({ channel: 'chrome', headless: true })
let failed = false
const fail = (msg: string) => {
  failed = true
  console.log('  FAIL', msg)
}

for (const path of PAGES) {
  // 1920 is wider than the column, so the frame closes past the gutter;
  // 375×1366 (a phone-width window on a tall screen) brings the card's foot
  // up early, before a quarter of a screen has scrolled.
  const sizes = [
    [1440, 900],
    [1920, 900],
    [390, 844],
    [375, 1366],
  ]
  for (const [width, height] of sizes) {
    console.log(path, `${width}×${height}`)
    const page = await browser.newPage({ viewport: { width, height } })
    const problems: string[] = []
    page.on('requestfailed', (r) => problems.push(`failed ${r.url()}`))
    page.on(
      'response',
      (r) => r.status() >= 400 && problems.push(`${r.status()} ${r.url()}`),
    )
    page.on('pageerror', (e) => problems.push(`script error: ${e.message}`))
    page.on(
      'console',
      (m) => m.type() === 'error' && problems.push(`console: ${m.text()}`),
    )
    await page.goto(`http://127.0.0.1:4391${path}`, {
      waitUntil: 'networkidle',
    })

    const text = await page.evaluate(() => document.body.innerText)
    for (const re of DROPPED) {
      const m = text.match(re)
      if (m) fail(`page says ${JSON.stringify(m[0])}`)
    }
    const badAnchors = await page.evaluate(() =>
      Array.from(document.querySelectorAll<HTMLAnchorElement>('a[href^="#"]'))
        .map((a) => a.getAttribute('href')!)
        .filter((h) => h !== '#' && !document.getElementById(h.slice(1))),
    )
    if (badAnchors.length) fail(`links to nothing: ${badAnchors.join(', ')}`)
    const sideways = await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    )
    if (sideways > 0) fail(`scrolls sideways by ${sideways}px`)

    // Everything set in the column shares its edges. (A caption's margin
    // rule once undid the column's centring, pushing it out to the window's
    // edge on screens wider than the column.)
    const astray = await page.evaluate(() => {
      const column = document.querySelector('.spaces')!.getBoundingClientRect()
      return Array.from(document.querySelectorAll<HTMLElement>('.wrap'))
        .filter((el) => {
          const box = el.getBoundingClientRect()
          const style = getComputedStyle(el)
          const left = box.left + parseFloat(style.paddingLeft)
          const right = box.right - parseFloat(style.paddingRight)
          return (
            Math.abs(left - column.left) > 1 ||
            Math.abs(right - column.right) > 1
          )
        })
        .map((el) => el.className)
    })
    if (astray.length) fail(`off the column: ${astray.join(', ')}`)

    // Each language's windows and feature tiles show the screens taken in
    // that language.
    const foreign = await page.evaluate(
      (slug) =>
        Array.from(
          document.querySelectorAll<HTMLImageElement>(
            '.shot__screen, .tile__screen',
          ),
        )
          .map((i) => i.getAttribute('src')!)
          .filter((src) => !src.startsWith(`/screens/${slug}/`)),
      path.replaceAll('/', '') || 'en',
    )
    if (foreign.length)
      fail(`screens from another language: ${foreign.join(', ')}`)

    // The opening card is full-screen at the top, still full-screen when
    // its window sits in the middle of the screen, and framed to the column
    // once the window's foot is 30% of the way down. Positions come from
    // layout, as Motion measures them: the window's scaling does not count.
    const scrollTo = async (y: number) => {
      await page.evaluate(
        (top) => window.scrollTo({ top, behavior: 'instant' }),
        y,
      )
      await page.waitForTimeout(400)
    }
    const marks = await page.evaluate(() => {
      const view = document.querySelector<HTMLElement>('.intro .shot__window')!
      let top = 0
      for (
        let el: HTMLElement | null = view;
        el;
        el = el.offsetParent as HTMLElement | null
      )
        top += el.offsetTop
      return {
        centred: top + view.offsetHeight / 2 - innerHeight / 2,
        closed: top + view.offsetHeight - innerHeight * 0.3,
      }
    })
    const atTop = await page.evaluate(probeCard)
    await scrollTo(marks.centred)
    const centred = await page.evaluate(probeCard)
    await scrollTo(marks.closed + 8)
    const movedOn = await page.evaluate(probeCard)
    if (!atTop.fullScreen)
      fail('opening card does not fill the screen at the top of the page')
    if (!centred.fullScreen)
      fail('the wall closes in before the window reaches the middle')
    if (!movedOn.framed)
      fail('opening card not framed once its window has moved on')

    for (let y = 0; y < 16000; y += 300) {
      await page.mouse.wheel(0, 300)
      await page.waitForTimeout(40)
    }
    await page.waitForTimeout(2200)
    const state = await page.evaluate(() => ({
      chart: document.querySelector('.retention')?.classList.contains('is-in'),
      bars: Array.from(document.querySelectorAll('.retention__kept')).map(
        (b) => getComputedStyle(b).transform,
      ),
      paint: getComputedStyle(document.querySelector('.retention__paint')!)
        .clipPath,
      broken: Array.from(document.images)
        .filter(
          (i) =>
            i.offsetParent !== null && (!i.complete || i.naturalWidth === 0),
        )
        .map((i) => i.currentSrc || i.src),
      paintings: Array.from(
        document.querySelectorAll<HTMLImageElement>('.painting img'),
      ).map((i) => i.currentSrc.split('/').pop()),
    }))
    if (!state.chart) fail('the retention chart never drew itself')
    if (state.bars.some((t) => t !== 'none'))
      fail(`bars not grown: ${state.bars.join(' ')}`)
    if (state.paint !== 'inset(0px)' && state.paint !== 'none')
      fail(`PathKeep's row not painted in: ${state.paint}`)
    if (state.broken.length)
      fail(`pictures that did not load: ${state.broken.join(', ')}`)
    const hidden = await page.evaluate(hiddenParts)
    if (hidden.length)
      fail(`still hidden after scrolling past: ${hidden.join(' | ')}`)
    console.log('  paintings:', state.paintings.join(' '))

    // The demo: on a screen wide enough for the app it has loaded into its
    // window, scaled to fit, and drawn a screen; a phone keeps the picture
    // and says why. (`demo/check.ts` checks the app inside.)
    await page
      .locator('.demo__shot .shot__window.is-live')
      .waitFor({ timeout: 15_000 })
      .catch(() => {})
    const demo = await page.evaluate(() => {
      const window = document.querySelector<HTMLElement>(
        '.demo__shot .shot__window',
      )!
      const frame = window.querySelector('iframe')
      const narrow = document.querySelector<HTMLElement>('.demo__narrow')!
      return {
        live: window.classList.contains('is-live'),
        frame: frame !== null,
        fits:
          frame !== null &&
          Math.abs(frame.getBoundingClientRect().width - window.clientWidth) <
            2,
        narrow: getComputedStyle(narrow).display !== 'none',
      }
    })
    if (width >= 760) {
      if (!demo.live) fail('the demo never came up in its window')
      else if (!demo.fits) fail('the demo is not scaled to its window')
    } else if (demo.frame || !demo.narrow) {
      fail('a phone loads the demo instead of keeping the picture')
    }

    // Over the demo the wheel scrolls the page until the window is clicked;
    // clicked, the wheel and the keyboard go to the app; once the page has
    // scrolled away and back, the wheel scrolls the page again.
    if (width >= 760 && demo.live) {
      const centre = async () => {
        await page.evaluate(() =>
          document
            .querySelector('.demo__shot .shot__window')!
            .scrollIntoView({ block: 'center', behavior: 'instant' }),
        )
        await page.waitForTimeout(400)
      }
      const app = page.frames().find((f) => f.url().includes('/app/'))!
      const appScrolled = () =>
        app.evaluate(() =>
          Array.from(document.querySelectorAll('*')).some(
            (el) => el.scrollTop > 0,
          ),
        )
      const wheelOver = async () => {
        const box = (await page
          .locator('.demo__shot .shot__window')
          .boundingBox())!
        await page.mouse.move(
          box.x + box.width * 0.6,
          box.y + box.height * 0.55,
        )
        const before = await page.evaluate(() => scrollY)
        await page.mouse.wheel(0, 400)
        await page.waitForTimeout(600)
        return (await page.evaluate(() => scrollY)) - before
      }
      await centre()
      const idle = await wheelOver()
      const idleApp = await appScrolled()
      await centre()
      await page.locator('.demo__shot .shot__start').click()
      const started = await page.evaluate(() => {
        const frame = document.querySelector('.demo__shot iframe')
        return (
          frame instanceof HTMLIFrameElement &&
          !frame.inert &&
          document.activeElement === frame
        )
      })
      const using = await wheelOver()
      const usingApp = await appScrolled()
      await scrollTo(0)
      await centre()
      const after = await wheelOver()
      if (idle < 200 || idleApp)
        fail('the wheel over the demo scrolls the app before it is clicked')
      if (!started) fail('clicking the demo does not hand it the keyboard')
      if (using !== 0 || !usingApp)
        fail('the wheel over the clicked demo does not scroll the app')
      if (after < 200)
        fail('the demo keeps the wheel after the page scrolled away from it')
    }

    await page.evaluate(() => window.scrollTo(0, 0))
    await page.waitForTimeout(300)
    await page.click('[data-tab="agent"]')
    await page.waitForTimeout(600)
    const agent = await page.evaluate(() =>
      document
        .querySelector('#shot-window .is-active')
        ?.getAttribute('data-screen'),
    )
    await page.focus('[data-tab="agent"]')
    await page.keyboard.press('ArrowLeft')
    await page.waitForTimeout(600)
    const back = await page.evaluate(() => ({
      screen: document
        .querySelector('#shot-window .is-active')
        ?.getAttribute('data-screen'),
      focused: document.activeElement?.getAttribute('data-tab'),
    }))
    if (
      agent !== 'agent' ||
      back.screen !== 'browsing' ||
      back.focused !== 'browsing'
    ) {
      fail(
        `tabs: click → ${agent}; arrow key → ${back.screen}, focus on ${back.focused}`,
      )
    }

    // A question opens to its whole answer, and closes again.
    const question = page.locator('.faq__item').nth(1)
    await question.locator('summary').click()
    await page.waitForTimeout(600)
    const opened = await question.evaluate((d) => {
      const answer = d.querySelector('p')!.getBoundingClientRect()
      const item = d.getBoundingClientRect()
      return (
        (d as HTMLDetailsElement).open &&
        answer.height > 0 &&
        answer.bottom <= item.bottom + 1
      )
    })
    await question.locator('summary').click()
    await page.waitForTimeout(600)
    const closed = await question.evaluate(
      (d) => !(d as HTMLDetailsElement).open,
    )
    if (!opened || !closed)
      fail(`a question does not open and close: ${opened}, ${closed}`)
    if (problems.length) fail(problems.join('; '))
    await page.close()
  }
}

// The files beside the pages: the Markdown for language models (whole, and
// with every product token turned back into a name), the crawler rules, and
// a lost address answered with 404 and a way home in every language.
console.log('/llms.txt, /robots.txt, an unknown address')
const fetched = async (path: string) => {
  const res = await fetch(`http://127.0.0.1:4391${path}`)
  return { status: res.status, text: await res.text() }
}
const llms = await fetched('/llms.txt')
if (llms.status !== 200 || !llms.text.startsWith('# PathKeep'))
  fail(`llms.txt: ${llms.status}`)
if (/[{}|]/.test(llms.text.replace(/\]\(.*?\)/g, '')))
  fail('llms.txt still holds copy markup')
if ((await fetched('/robots.txt')).status !== 200) fail('robots.txt missing')
const lost = await fetched('/zh-tw/catalogue/')
if (lost.status !== 404) fail(`an unknown address answers ${lost.status}`)
for (const home of ['href="/"', 'href="/zh-tw/"', 'href="/zh-cn/"'])
  if (!lost.text.includes(home)) fail(`the 404 page has no link ${home}`)

// With reduced motion nothing moves: the card rests framed from the start.
console.log('/ 1920, reduced motion')
const still = await browser.newPage({
  viewport: { width: 1920, height: 900 },
  reducedMotion: 'reduce',
})
await still.goto('http://127.0.0.1:4391/', { waitUntil: 'networkidle' })
if (!(await still.evaluate(probeCard)).framed)
  fail('with reduced motion the opening card is not resting framed')
const stillHidden = await still.evaluate(hiddenParts)
if (stillHidden.length)
  fail(`hidden with reduced motion: ${stillHidden.join(' | ')}`)
await still.close()

// Without script every word is on the page (people and crawlers alike) and
// the card rests framed.
console.log('/ 1440, no script')
const bare = await browser.newPage({
  viewport: { width: 1440, height: 900 },
  javaScriptEnabled: false,
})
await bare.goto('http://127.0.0.1:4391/', { waitUntil: 'networkidle' })
const bareHidden = await bare.evaluate(hiddenParts)
if (bareHidden.length) fail(`hidden without script: ${bareHidden.join(' | ')}`)
if (!(await bare.evaluate(probeCard)).framed)
  fail('without script the opening card is not resting framed')
await bare.close()
await browser.close()
console.log(failed ? 'SOME CHECKS FAILED' : 'all checks passed')
process.exit(failed ? 1 : 0)
