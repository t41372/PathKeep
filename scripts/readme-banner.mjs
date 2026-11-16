/**
 * Compose each README banner from the existing synthetic Home screenshot.
 * Run `node scripts/readme-banner.mjs` after refreshing the Home screenshots.
 * This renders the banner only; it doesn't launch PathKeep or read an archive.
 */
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { chromium } from '@playwright/test'

const screenshots = new URL('../docs/screenshots/', import.meta.url)
const artwork = new URL('../docs/design/brand-icon/source/', import.meta.url)

const COPY = {
  en: {
    headline: 'Your browser deletes old history.',
    description:
      'PathKeep backs it up before it’s gone. Search years of history across browsers in one archive on your own computer.',
  },
  'zh-CN': {
    headline: '浏览器会悄悄删除旧记录',
    description:
      'PathKeep 在记录被清理前备份到你的电脑。多年以后，你仍能在同一份存档里找回用不同浏览器看过的网页。',
  },
}

const BANNER = { width: 1440, height: 1160 }
// The Home image is a 1440 × 900 window centered on a 1680 × 1050 stage
// (see readme-screenshots.mjs). Both are rendered at the same pixel density.
const STAGE = { width: 1680, height: 1050 }
const WINDOW = { width: 1440, height: 900 }

function dataUrl(file, type) {
  return `data:image/${type};base64,${readFileSync(file).toString('base64')}`
}

function banner(language, { headline, description }) {
  const home = dataUrl(new URL(`${language}/home.jpg`, screenshots), 'jpeg')
  const painting = dataUrl(
    new URL('backdrops/path-dawn.jpg', screenshots),
    'jpeg',
  )
  const icon = dataUrl(new URL('icon-1024.png', artwork), 'png')

  return `<!doctype html>
<html lang="${language}">
<meta charset="utf-8">
<style>
  * { box-sizing: border-box; }
  body {
    margin: 0;
    min-height: 100vh;
    padding: 45px 80px 0;
    color: #201a18;
    font: 32px system-ui, 'PingFang SC', 'Microsoft YaHei', sans-serif;
    background:
      linear-gradient(180deg, #fff9f2 0%, #fff9f2 16%, #fff5e7 22%, transparent 56%),
      url('${painting}') center bottom / cover no-repeat #eed9bd;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 13px;
    font-weight: 650;
    letter-spacing: -0.6px;
  }
  .brand img { width: 50px; height: 50px; }
  h1 {
    margin: 18px 0 9px;
    font-size: 72px;
    font-weight: 720;
    letter-spacing: -2px;
    line-height: 1.14;
  }
  html[lang='zh-CN'] h1 { letter-spacing: -1px; }
  p { margin: 0; max-width: 1090px; color: #594a41; line-height: 1.35; text-wrap: balance; }
  .demo {
    position: absolute;
    inset: auto 80px 33px;
    aspect-ratio: ${WINDOW.width} / ${WINDOW.height};
    border-radius: 12px;
    background-image: url('${home}');
    background-size: ${(100 * STAGE.width) / WINDOW.width}% ${(100 * STAGE.height) / WINDOW.height}%;
    background-position: center;
    box-shadow: 0 20px 54px #1e120d55, 0 0 0 1px #1e120d26;
  }
</style>
<body>
  <div class="brand"><img src="${icon}" alt="">PathKeep</div>
  <h1>${headline}</h1>
  <p>${description}</p>
  <div class="demo"></div>
</body>
</html>`
}

const browser = await chromium.launch()
try {
  const page = await browser.newPage({ viewport: BANNER })
  for (const [language, copy] of Object.entries(COPY)) {
    const file = new URL(`${language}/banner.jpg`, screenshots)
    await page.setContent(banner(language, copy))
    await page.screenshot({ path: fileURLToPath(file), quality: 92 })
    console.log(`docs/screenshots/${language}/banner.jpg`)
  }
} finally {
  await browser.close()
}
