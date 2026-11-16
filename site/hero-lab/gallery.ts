/**
 * Builds the comparison gallery: every candidate as a hero (with a sample
 * nav and headline over it), a 100% crop, a phone-width crop, its dominant
 * colours, and the size of the full 4K WebP.
 *
 *   bun site/hero-lab/gallery.ts [--seed N] [--set first|pk]
 *
 * `first` is the first round (headline over the painting, 16:10); `pk` is
 * the dragged-band round in PathKeep's colours, shown with the app's
 * translucent window floating over it (16:9).
 * Writes the gallery's index.html and images under `site/hero-lab/out/`.
 */

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { openRenderer } from './driver'
import { presets } from './src/presets'

const argv = process.argv.slice(2)
const flag = (name: string) =>
  argv.includes(`--${name}`) ? argv[argv.indexOf(`--${name}`) + 1] : undefined
const seed = Number(flag('seed') ?? 1)
const set = flag('set') ?? 'first'
const W = 3840
const windowed = set === 'pk'
const H = windowed ? 2176 : 2400

interface Look {
  /** 100% detail crop in full-resolution pixels. */
  detail: [number, number, number, number]
  /** Text colour of the sample headline; for the window mock, light or dark app theme. */
  tone: 'ink' | 'paper'
}

const looks: Record<string, Look> = {
  'cf-dawn': { detail: [900, 1250, 1200, 750], tone: 'ink' },
  'cf-ember': { detail: [2050, 1300, 1200, 750], tone: 'paper' },
  'cf-spaces': { detail: [2450, 1650, 1200, 750], tone: 'ink' },
  'cf-fresco': { detail: [1700, 1150, 1200, 750], tone: 'ink' },
  'wc-dawn': { detail: [1250, 1350, 1200, 750], tone: 'ink' },
  'wc-dusk': { detail: [700, 1500, 1200, 750], tone: 'paper' },
  'wc-tide': { detail: [700, 1150, 1200, 750], tone: 'ink' },
  'wc-mist': { detail: [1100, 1700, 1200, 750], tone: 'ink' },
  'wc-stain': { detail: [2300, 650, 1200, 750], tone: 'ink' },
  'pk-dawn': { detail: [60, 900, 1200, 750], tone: 'ink' },
  'pk-dusk': { detail: [60, 1100, 1200, 750], tone: 'paper' },
  'pk-ember': { detail: [60, 900, 1200, 750], tone: 'ink' },
  'pk-spaces': { detail: [60, 900, 1200, 750], tone: 'ink' },
  'pk-paper': { detail: [60, 1100, 1200, 750], tone: 'ink' },
}

const ids = Object.keys(presets).filter((id) =>
  windowed ? id.startsWith('pk-') : !id.startsWith('pk-'),
)

const outDir = join(import.meta.dir, 'out', windowed ? 'gallery-pk' : 'gallery')
await mkdir(outDir, { recursive: true })
const renderer = await openRenderer()

interface Entry {
  id: string
  title: string
  note: string
  tone: Look['tone']
  palette: { hex: string; share: number }[]
  webpBytes: number
  ms: number
}
const entries: Entry[] = []
const phoneW = Math.round((H * 9) / 19.5)

try {
  for (const id of ids) {
    const preset = presets[id]
    const look = looks[id] ?? { detail: [1300, 800, 1200, 750], tone: 'ink' }
    const [dx, dy, dw, dh] = look.detail
    const res = await renderer.render({
      preset: id,
      seed,
      width: W,
      height: H,
      webpQuality: 0.88,
      previews: [2000],
      palette: 6,
      crops: [
        { name: 'detail', x: dx, y: dy, w: dw, h: dh, type: 'webp' },
        {
          name: 'phone',
          x: Math.round((W - phoneW) / 2),
          y: 0,
          w: phoneW,
          h: H,
          outW: 520,
          type: 'webp',
        },
      ],
    })
    const dir = join(outDir, id)
    await mkdir(dir, { recursive: true })
    let webpBytes = 0
    for (const f of res.files) {
      if (f.name.endsWith('.jpg')) continue
      const bytes = Buffer.from(f.base64, 'base64')
      if (f.name === 'full.webp') webpBytes = bytes.length
      await writeFile(join(dir, f.name), bytes)
    }
    entries.push({
      id,
      title: preset.title,
      note: preset.note,
      tone: look.tone,
      palette: res.palette,
      webpBytes,
      ms: res.ms,
    })
    console.log(
      `${id}: ${(webpBytes / 1048576).toFixed(2)} MB webp, ${res.ms.toFixed(0)} ms`,
    )
  }
} finally {
  await renderer.close()
}

const esc = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
const mb = (n: number) => `${(n / 1048576).toFixed(1)} MB`

const windowHero = (e: Entry) => `
  <div class="mock ${e.tone === 'paper' ? 'dark' : 'light'}">
    <img src="${e.id}/w2000.webp" alt="">
    <div class="window"><div class="dots"><i></i><i></i><i></i></div><div class="panel"></div></div>
    <div class="tabs"><span class="on">Browsing</span><span>Agents</span><span>Insights</span></div>
  </div>`

const sections = entries
  .map(
    (e, i) => `
<section class="candidate" id="${e.id}">
  <div class="label"><span class="num">${String(i + 1).padStart(2, '0')}</span><h2>${esc(e.title)}</h2><code>${e.id}</code></div>
  ${windowed ? windowHero(e) : ''}
  <div class="hero tone-${e.tone}${windowed ? ' bare' : ''}">
    <img src="${e.id}/w2000.webp" alt="${esc(e.title)}">
    <nav><span class="mark">PathKeep</span><span class="links"><a>Features</a><a>Demo</a><a>Privacy</a><a>GitHub</a></span><a class="nav-cta">Download</a></nav>
    <div class="copy">
      <h1>Keep the path<br>you’ve walked.</h1>
      <p>Your browsers quietly delete your history. PathKeep keeps all of it, on your computer, and helps you find your way back.</p>
      <div class="actions"><a class="cta">Download for macOS</a><a class="ghost">Try the demo →</a></div>
    </div>
    <div class="plate">Pl. ${String(i + 1).padStart(2, '0')} — ${esc(e.title)}</div>
  </div>
  <div class="details">
    <figure class="detail"><img src="${e.id}/crop-detail.webp" alt=""><figcaption>100% crop of the 3840 × 2400 render</figcaption></figure>
    <figure class="phone"><img src="${e.id}/crop-phone.webp" alt=""><figcaption>Phone (centre crop)</figcaption></figure>
    <div class="meta">
      <p class="note">${esc(e.note)}</p>
      <ul class="swatches">${e.palette
        .map(
          (c) =>
            `<li><span style="background:${c.hex}"></span><code>${c.hex}</code><em>${Math.round(c.share * 100)}%</em></li>`,
        )
        .join('')}</ul>
      <dl><dt>4K WebP</dt><dd>${mb(e.webpBytes)}</dd><dt>Render</dt><dd>${(e.ms / 1000).toFixed(1)} s on the GPU</dd><dt>Seed</dt><dd>${seed}</dd></dl>
    </div>
  </div>
</section>`,
  )
  .join('\n')

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Hero candidates</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500&family=Instrument+Serif:ital@0;1&display=swap" rel="stylesheet">
<style>
  :root { --bg: #131211; --fg: #ece6dc; --muted: #9a9288; --line: rgb(236 230 220 / 0.12); }
  * { box-sizing: border-box; }
  body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.5 Geist, system-ui, sans-serif; }
  header.page { max-width: 1440px; margin: 0 auto; padding: 72px 32px 24px; }
  header.page h1 { font: 400 64px/1 'Instrument Serif', serif; margin: 0 0 16px; letter-spacing: -0.01em; }
  header.page p { color: var(--muted); max-width: 70ch; margin: 0; }
  header.page ol { color: var(--muted); columns: 3; padding-left: 20px; margin-top: 24px; font-family: 'Geist Mono', monospace; font-size: 13px; }
  header.page a { color: var(--fg); }
  .candidate { max-width: 1440px; margin: 0 auto; padding: 56px 32px; border-top: 1px solid var(--line); }
  .label { display: flex; align-items: baseline; gap: 16px; margin-bottom: 18px; }
  .label .num { font-family: 'Geist Mono', monospace; color: var(--muted); }
  .label h2 { font: 400 34px/1.1 'Instrument Serif', serif; margin: 0; }
  .label code { color: var(--muted); font-size: 13px; }
  .hero { position: relative; aspect-ratio: ${W} / ${H}; overflow: hidden; border-radius: 4px; }
  .hero > img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  .hero nav { position: absolute; left: 0; right: 0; top: 0; display: flex; align-items: center; gap: 32px; padding: 22px 36px; font-size: 14px; }
  .hero .mark { font-weight: 600; letter-spacing: -0.01em; font-size: 16px; }
  .hero .links { display: flex; gap: 26px; margin-left: auto; opacity: 0.85; }
  .hero .nav-cta { padding: 7px 14px; border-radius: 999px; border: 1px solid currentColor; }
  .hero .copy { position: absolute; left: 6.5%; top: 24%; max-width: 46%; }
  .hero h1 { font: 400 clamp(40px, 6.4vw, 92px)/0.98 'Instrument Serif', serif; letter-spacing: -0.015em; margin: 0 0 22px; }
  .hero .copy p { font-size: clamp(14px, 1.25vw, 18px); max-width: 34em; margin: 0 0 26px; }
  .hero .actions { display: flex; gap: 18px; align-items: center; font-size: 15px; }
  .hero .cta { padding: 11px 20px; border-radius: 999px; font-weight: 500; }
  .hero .plate { position: absolute; right: 28px; bottom: 22px; font: 12px 'Geist Mono', monospace; opacity: 0.7; }
  .mock { position: relative; aspect-ratio: ${W} / ${H}; overflow: hidden; border-radius: 4px; margin-bottom: 18px; }
  .mock > img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  .mock .window { position: absolute; left: 10.25%; right: 10.15%; top: 5.1%; bottom: 12.2%; border-radius: 14px; backdrop-filter: blur(40px); -webkit-backdrop-filter: blur(40px); box-shadow: 0 24px 70px rgb(0 0 0 / 0.28), inset 0 0 0 1px rgb(255 255 255 / 0.22); }
  .mock.light .window { background: rgb(250 250 252 / 0.62); }
  .mock.dark .window { background: rgb(24 24 34 / 0.62); }
  .mock .panel { position: absolute; left: 5.6%; right: 0.8%; top: 1.4%; bottom: 1.4%; border-radius: 12px; }
  .mock.light .panel { background: rgb(255 255 255 / 0.66); }
  .mock.dark .panel { background: rgb(30 30 42 / 0.55); }
  .mock .dots { position: absolute; left: 1.4%; top: 2.2%; display: flex; gap: 7px; }
  .mock .dots i { width: 11px; height: 11px; border-radius: 50%; background: rgb(0 0 0 / 0.18); }
  .mock .tabs { position: absolute; left: 50%; bottom: 3.4%; transform: translateX(-50%); display: flex; gap: 4px; padding: 4px; border-radius: 999px; background: rgb(255 255 255 / 0.16); backdrop-filter: blur(16px); -webkit-backdrop-filter: blur(16px); font: 500 12px 'Geist Mono', monospace; letter-spacing: 0.08em; text-transform: uppercase; color: rgb(255 255 255 / 0.9); }
  .mock .tabs span { padding: 7px 16px; border-radius: 999px; }
  .mock .tabs .on { background: #f6f4f0; color: #1b1714; }
  .hero.bare nav, .hero.bare .copy, .hero.bare .plate { display: none; }
  .tone-ink { color: #1b1714; }
  .tone-ink .cta { background: #1b1714; color: #f4efe6; }
  .tone-paper { color: #f4efe6; }
  .tone-paper .cta { background: #f4efe6; color: #1b1714; }
  .details { display: grid; grid-template-columns: minmax(0, 1fr) 200px minmax(0, 0.8fr); gap: 28px; margin-top: 28px; align-items: start; }
  figure { margin: 0; }
  figure img { width: 100%; display: block; border-radius: 3px; }
  figcaption { color: var(--muted); font-size: 12px; margin-top: 8px; font-family: 'Geist Mono', monospace; }
  .note { margin: 0 0 18px; }
  .swatches { list-style: none; padding: 0; margin: 0 0 18px; display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
  .swatches li { display: grid; grid-template-columns: 28px 1fr; grid-template-rows: auto auto; column-gap: 8px; font-size: 12px; }
  .swatches span { grid-row: span 2; width: 28px; height: 28px; border-radius: 50%; border: 1px solid var(--line); }
  .swatches em { color: var(--muted); font-style: normal; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 14px; font-size: 13px; margin: 0; }
  dt { color: var(--muted); }
  dd { margin: 0; }
  @media (max-width: 900px) {
    .details { grid-template-columns: 1fr; }
    .hero .copy { max-width: 80%; top: 18%; }
    .hero .links { display: none; }
  }
</style>
</head>
<body>
<header class="page">
  <h1>Hero candidates</h1>
  <p>${
    windowed
      ? 'Dragged-band paintings in PathKeep’s own colours: the light and dark walls, the orange heat scale, agent teal with browsing orange, and a quiet paper one. Full-width bands feathered into each other and dragged sideways on a ribbed canvas. Each is shown under PathKeep’s translucent window, then bare, with a 100% crop and its colours.'
      : 'Nine paintings drawn in WebGL: pigments mixed with Kubelka–Munk, laid on linen, plaster or cold-pressed paper. Each one is shown as the top of the page would look, with a 100% crop, a phone crop and the colours a site would take from it. The nav and headline are placeholders to judge legibility, not the final type.'
  }</p>
  <ol>${entries.map((e) => `<li><a href="#${e.id}">${esc(e.title)}</a></li>`).join('')}</ol>
</header>
${sections}
</body>
</html>
`
await writeFile(join(outDir, 'index.html'), html)
console.log(join(outDir, 'index.html'))
