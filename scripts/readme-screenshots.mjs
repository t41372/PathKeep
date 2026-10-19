/**
 * The README's pictures, made the same way each time, from the demo archive:
 * synthetic browsers, never anyone's own history.
 *
 *   bun run dev:demo -- --fresh                  (until it says ready)
 *   node scripts/readme-screenshots.mjs          (every picture)
 *   node scripts/readme-screenshots.mjs ask      (only the shots named)
 *
 * Writes docs/screenshots/<language>/<shot>.jpg for each language a README
 * comes in.
 *
 * 1. Each shot is the real app on the demo backend, in Chromium at the
 *    window's size and 2×. Its wall is left out and its layers take the glass
 *    window's values (index.css), so what is under the window shows through,
 *    and the nav rail leaves room for the traffic lights as on macOS.
 * 2. It is laid over a painting from docs/screenshots/backdrops/, the way
 *    macOS shows a glass window over a wallpaper: the painting behind the
 *    window blurred and tinted, with the window's corners, shadow and traffic
 *    lights.
 *
 * Ask's answer comes from the fake model the E2E tests use
 * (tests/e2e/support/fake-model.ts), scripted here: no model runs. It searches
 * the demo archive through PathKeep's real tool, and the pages it cites are
 * what that search found.
 *
 * A new picture is an entry in SHOTS, a new backdrop a JPEG in the backdrops
 * folder. path-dawn.jpg and path-dusk.jpg were painted by OpenAI's image
 * model through Codex (2026-10-18).
 */
import { mkdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from '@playwright/test'
import { resolveDesktopBridgeEnv } from './pathkeep-dev-desktop-bridge.mjs'
import {
  lastUserText,
  offersTools,
  startFakeModel,
} from '../tests/e2e/support/fake-model.ts'

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
)
const out = path.join(repoRoot, 'docs', 'screenshots')
const { devServerUrl, devIpcUrl } = resolveDesktopBridgeEnv()

/** A command straight to the backend, for clearing what an earlier shot left. */
async function backend(command, payload) {
  const response = await fetch(`${devIpcUrl}/commands/${command}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  })
  const body = await response.json()
  if (!response.ok) throw new Error(`${command}: ${JSON.stringify(body)}`)
  return body
}

/** The window's size, as the user would have it. */
const WINDOW = { width: 1440, height: 900 }
/** The picture: the window with some of the painting around it, 16:10. */
const STAGE = { width: 1680, height: 1050 }

/**
 * What macOS does behind a glass window: a wide blur that keeps the colors,
 * and a light or dark tint.
 */
const MATERIAL = {
  light: {
    filter: 'blur(30px) saturate(1.5) brightness(1.08)',
    tint: 'rgb(246 246 250 / 0.34)',
  },
  dark: {
    filter: 'blur(30px) saturate(1.4) brightness(0.72)',
    tint: 'rgb(22 20 34 / 0.42)',
  },
}

/**
 * The glass window's layers (index.css) without its Tauri check, and the
 * nav rail's room for the traffic lights (`hasMacOverlayTitlebar`).
 */
const WINDOW_CSS = `
  body { background: transparent !important; }
  :root { --window: rgb(250 250 252 / 0.14); --panel: rgb(255 255 255 / 0.42); }
  :root.dark { --window: rgb(24 24 34 / 0.18); --panel: rgb(30 30 42 / 0.3); }
  nav > div[data-tauri-drag-region]:first-child { height: 46px !important; }
`

/** The few words a shot looks for, in each language. */
const WORDS = {
  en: {
    time: 'When you browse',
    message: 'Message',
    send: 'Send',
    question: 'Which camera was I leaning toward last year, and why?',
    reasoning:
      'The camera research. Search the archive for the bodies that were compared.',
    answer: [
      'You kept coming back to the **Sony A7C II**, and autofocus was the question.',
      '',
      '- You searched “a7c ii autofocus” again and again, and read DPReview’s autofocus test of it against the **Nikon Z f**.',
      '- You read *A7C II after 6 months* on r/SonyAlpha.',
      '- The **Fujifilm X-T5** came up as a comparison, next to its review.',
      '',
      'The pages are below.',
    ].join('\n'),
  },
  'zh-CN': {
    time: '什么时候浏览',
    message: '消息',
    send: '发送',
    question: '去年看相机的时候，我比较倾向哪一台？为什么？',
    reasoning: '是看相机那段时间。先在存档里搜一下比较过的机身。',
    answer: [
      '你一直回头看的是 **Sony A7C II**，在意的是对焦。',
      '',
      '- 你反复搜索“a7c ii autofocus”，读了 DPReview 拿它和 **Nikon Z f** 比较对焦的测试。',
      '- 你读了 r/SonyAlpha 上的 *A7C II after 6 months*。',
      '- **Fujifilm X-T5** 是拿来比较的那台，你也看了它的评测。',
      '',
      '相关的页面在下面。',
    ].join('\n'),
  },
}

const LANGUAGES = /** @type {const} */ (['en', 'zh-CN'])

/** Opens a screen from the nav rail, as a user would. */
const rail = (page, to) =>
  page.locator(`nav a[href$="#/${to}"]`).first().click()

// Light ones first, so the theme changes once per language.
const SHOTS = [
  { name: 'home', theme: 'light', backdrop: 'path-dawn', show: () => {} },
  {
    name: 'history',
    theme: 'light',
    backdrop: 'path-dawn',
    show: async (page) => {
      await rail(page, 'history')
      // A page from today's first session, with its detail panel open.
      await page.getByRole('option').nth(3).click()
    },
  },
  {
    name: 'insights',
    theme: 'light',
    backdrop: 'path-dawn',
    show: (page) => rail(page, 'insights'),
  },
  {
    name: 'page',
    theme: 'light',
    backdrop: 'path-dawn',
    // A page the demo keeps reopening, and why Insights lists it.
    show: (page) =>
      page.goto(
        `${devServerUrl}/#/insights/page/${encodeURIComponent('https://jlptsensei.com/n3-listening/14')}`,
      ),
  },
  {
    name: 'backup',
    theme: 'light',
    backdrop: 'path-dawn',
    show: (page) => rail(page, 'backup'),
  },
  { name: 'home-dark', theme: 'dark', backdrop: 'path-dusk', show: () => {} },
  {
    name: 'rhythm',
    theme: 'dark',
    backdrop: 'path-dusk',
    show: async (page, words) => {
      await rail(page, 'insights')
      await page
        .getByRole('heading', { name: words.time, exact: true })
        .evaluate((heading) => {
          heading.scrollIntoView({ block: 'start' })
          // A little of the section above, so the heading has room.
          let list = heading.parentElement
          while (list && list.scrollHeight <= list.clientHeight)
            list = list.parentElement
          list?.scrollBy(0, -20)
        })
    },
  },
  {
    name: 'ask',
    theme: 'dark',
    backdrop: 'path-dusk',
    show: async (page, words) => {
      // Only this language's conversation in the list.
      const { conversations } = await backend('list_ai_conversations', {
        request: {},
      })
      for (const { id } of conversations)
        await backend('delete_ai_conversation', { conversationId: id })
      await rail(page, 'ask')
      const composer = page.getByRole('textbox', { name: words.message })
      await composer.fill(words.question)
      await composer.press('Enter')
      await page.getByText(words.answer.split('**')[0]).first().waitFor()
      await page
        .getByRole('button', { name: words.send, exact: true })
        .waitFor({ timeout: 60_000 })
    },
  },
]

const only = process.argv.slice(2)
const shots = only.length
  ? SHOTS.filter((shot) => only.includes(shot.name))
  : SHOTS

const model = await startFakeModel({
  apiKey: null,
  script: (body) => {
    const words = /[一-鿿]/.test(lastUserText(body)) ? WORDS['zh-CN'] : WORDS.en
    if (!offersTools(body)) return { content: 'OK' }
    if (body.messages?.at(-1)?.role === 'tool') return { content: words.answer }
    return {
      reasoning: words.reasoning,
      toolCalls: [
        { name: 'search_history', arguments: { query: 'a7c ii nikon z f' } },
      ],
    }
  },
})
const browser = await chromium.launch()
const stage = await browser.newPage({
  viewport: STAGE,
  deviceScaleFactor: 2,
})

try {
  if (shots.some((shot) => shot.name === 'ask')) await useFakeModel(model.url)
  for (const language of LANGUAGES) {
    const words = WORDS[language]
    const context = await browser.newContext({
      viewport: WINDOW,
      deviceScaleFactor: 2,
      locale: language === 'en' ? 'en-US' : language,
    })
    await context.addInitScript((css) => {
      document.addEventListener('DOMContentLoaded', () => {
        const style = document.createElement('style')
        style.textContent = css
        document.head.append(style)
      })
    }, WINDOW_CSS)
    const page = await context.newPage()

    mkdirSync(path.join(out, language), { recursive: true })
    let theme = null
    for (const shot of shots) {
      if (shot.theme !== theme) {
        theme = shot.theme
        await page.goto(`${devServerUrl}/#/`)
        await page.evaluate((value) => {
          localStorage.setItem('pathkeep-theme', value)
        }, theme)
        await page.emulateMedia({ colorScheme: theme })
        await page.reload()
      } else {
        await page.goto(`${devServerUrl}/#/`)
      }
      await page.locator('nav a').first().waitFor()
      await shot.show(page, words)
      // Off the page, so whatever was clicked last isn't shown hovered.
      await page.mouse.move(WINDOW.width - 1, WINDOW.height - 1)
      // Lets the page settle: data arriving, charts drawing and animating,
      // folds opening. A chart caught while it moves shows its bars half way.
      await page.waitForLoadState('networkidle')
      await page.waitForTimeout(3000)
      const file = path.join(out, language, `${shot.name}.jpg`)
      await compose(
        stage,
        await page.screenshot({ omitBackground: true }),
        shot,
        file,
      )
      console.log(path.relative(repoRoot, file))
    }
    await context.close()
  }
} finally {
  await browser.close()
  await model.close()
}

/**
 * Adds the fake model in Settings → AI as a user would, which makes it the
 * service Ask uses. Each run's model has a port of its own, so each run adds
 * one: start from a fresh demo.
 */
async function useFakeModel(address) {
  const page = await browser.newPage({ locale: 'en-US' })
  await page.goto(`${devServerUrl}/#/settings/ai`)
  await page.getByRole('combobox', { name: 'AI service' }).click()
  await page.getByRole('option', { name: 'Add provider…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Add an AI service' })
  await dialog.getByRole('combobox', { name: 'Service' }).click()
  await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
  await dialog.getByRole('textbox', { name: 'Address' }).fill(address)
  await dialog.getByRole('textbox', { name: 'Model' }).fill('qwen3-32b')
  await dialog.getByRole('textbox', { name: 'Name' }).fill('Demo model')
  await dialog.getByRole('button', { name: 'Add service' }).click()
  await page.getByText('Demo model is ready.').waitFor()
  await page.close()
}

async function compose(stage, window, shot, file) {
  const backdrop = readFileSync(
    path.join(out, 'backdrops', `${shot.backdrop}.jpg`),
  ).toString('base64')
  const { filter, tint } = MATERIAL[shot.theme]
  const light = shot.theme === 'light'
  const button = (color, x) =>
    `<span style="position:absolute;left:${x}px;top:0;width:12px;height:12px;border-radius:50%;background:${color};box-shadow:inset 0 0 0 0.5px rgb(0 0 0 / 0.2)"></span>`
  await stage.setContent(`<!doctype html>
<body style="margin:0;overflow:hidden">
  <div style="position:relative;width:${STAGE.width}px;height:${STAGE.height}px;background:url(data:image/jpeg;base64,${backdrop}) center/cover">
    <div style="position:absolute;left:${(STAGE.width - WINDOW.width) / 2}px;top:${(STAGE.height - WINDOW.height) / 2}px;width:${WINDOW.width}px;height:${WINDOW.height}px;
      border-radius:14px;overflow:hidden;box-shadow:0 28px 70px rgb(0 0 0 / 0.38), 0 0 0 0.5px rgb(0 0 0 / ${light ? 0.18 : 0.6})">
      <div style="position:absolute;inset:0;backdrop-filter:${filter};background:${tint}"></div>
      <img src="data:image/png;base64,${window.toString('base64')}" style="position:absolute;inset:0;width:100%;height:100%">
      <div style="position:absolute;left:18px;top:17px">${button('#ff5f57', 0)}${button('#febc2e', 20)}${button('#28c840', 40)}</div>
      <div style="position:absolute;inset:0;border-radius:14px;box-shadow:inset 0 0 0 0.5px rgb(255 255 255 / ${light ? 0.55 : 0.14})"></div>
    </div>
  </div>
</body>`)
  await stage.waitForFunction(() =>
    [...document.images].every((image) => image.complete),
  )
  await stage.screenshot({ path: file, type: 'jpeg', quality: 88 })
}
