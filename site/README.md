# PathKeep's landing page

A static site built with Astro: one page in English (`/`), Traditional
Chinese (`/zh-tw/`) and Simplified Chinese (`/zh-cn/`). It has its own
`package.json` and lockfile; nothing here is part of the app.

```sh
bun install
bun run dev       # http://127.0.0.1:4391
bun run check     # astro check: types in .astro and .ts files
bun run build     # writes dist/, with the demo already built in public/app/
bun run preview   # serves dist/ on 4391
```

The demo in `public/app/` is built and committed (see The demo), so
building the site needs nothing outside this folder.

## Where things are

- `src/content/copy.ts` is the shape of every word on the page; the words
  are in `copy-en.ts`, `copy-zh-tw.ts` and `copy-zh-cn.ts`. A `|` marks
  where a line may break (see `src/components/Phrases.astro`), so Chinese
  headings never end on a single character.
- `src/components/` holds one component per section, plus `Painting.astro`
  (a painting behind a panel's content) and `Shot.astro` (the app's window
  over a painting).
- `src/styles/site.css` is the whole look. `fonts.css` loads the faces
  from Fontsource; body Chinese uses the system sans.
- The motion runs on [Motion](https://motion.dev) from the script in
  `src/layouts/Page.astro`: `scroll()` scrubs the opening card (the frame
  closing in, the window growing, the painting drifting) and `inView()`
  lets section heads and panels rise into place once. `site.css` holds
  every resting state, so with reduced motion or without script the page
  is simply still and fully visible.
- `public/paintings/` holds the paintings, `public/screens/` the app
  screenshots in the windows and on the feature wall, `public/brand/` the
  mark.
- `demo/` builds the demo that runs in the last window (see below).

## The paintings

Every painting is drawn in code by the lab in `hero-lab/`: a WebGL2
painter that mixes pigments with Kubelka–Munk on a simulated ribbed linen
(`hero-lab/src/presets.ts` has the recipes). It needs a Chrome install,
which renders on the GPU.

```sh
bun hero-lab/export-panels.ts    # paintings → public/paintings/<cut>-<width>.webp
bun hero-lab/export-screens.ts   # hero-lab/screens-in/<slug>/*.png → public/screens/<slug>/*.webp
bun hero-lab/export-social.ts    # share pictures → public/social/<slug>.jpg (needs the preview server)
bun hero-lab/gallery.ts --set pk # every preset on one page, in hero-lab/out/
```

`export-panels.ts` cuts each panel's picture out of a larger render, so
the calm band of each painting sits under the words set on it: ink on the
light bands, paper on the dark ones. If a panel's shape or its text
changes, check the text still lands on its band before changing the cut.

To paint a new one, add a recipe to `hero-lab/src/presets.ts` (a support,
the layers in the order a painter would lay them, then the light and the
grade), look at it with `bun hero-lab/render.ts <preset> --size 3840x2160`
or beside the others with `gallery.ts`, then give it a source and its cuts
in `export-panels.ts`. Every render uses a fixed seed, so a recipe always
paints the same picture. The recipes that did not make it onto the page
(the `cf-`, `wc-` and `pk-dawn` rounds) stay as starting points. Renders
land in `hero-lab/out/`, which git ignores.

## The demo

The window in the demo section runs the real app: `demo/vite.config.ts`
builds `src/` as it is, and swaps the three modules that reach the desktop
backend (`src/lib/ipc/bridge.ts`, `events.ts`, `search-stream.ts`) for
`demo/transport/`, matched by the file they resolve to. Nothing in `src/`
changes, and the desktop build never contains the swap. The page loads the
app (`/app/?lang=…`) into its window as the section comes near, on screens
at least 760 px wide; phones keep the picture and say the demo needs a
computer's screen. The loaded app stays inert, so the wheel scrolls the
page past it, until the visitor clicks the window; once the section has
scrolled away it is inert again.

The answers come from the recording in `public/app/recording/`: what the real backend
said while `demo/walk.ts` clicked through every screen, over synthetic
browsers set up the way onboarding's defaults leave them. `demo/backend.ts`
answers, in this order: a batch lookup (icons, preview images, stars) item
by item; the exact call; a History list or ⌘K word search worked out from
the recorded visits and pages; a page opened in a new tab; a change; an
Insights card's section from the batch it came in (a card reads its own
once its copy goes stale, after a backup, say); the same command with the
closest arguments. A search the walk never typed is
answered over every page the archive holds (the recording keeps the list).

Nothing a visitor does is saved, and nothing says so either: a change (a
setting, a star, a note, the backup schedule, Back up now) is answered the
way the backend answers one that went through, built from the request over
what the recording says now (`demo/writes.ts`). The app shows it taken;
the next time it reads the same thing it gets the recording again, and a
reload starts over. Each answer is typed with the reply the app's client
declares, so a version that changes one fails `tsc -p demo`. A command the
demo cannot answer that way, such as importing, restoring or deleting the
archive, says in the visitor's language that the demo can't do it.

The demo's clock starts where the recording was made, because the app
asks for "today" and "the last 7 days" by its own clock. Insights use the
archive's time zone and match anywhere; the demo's settings say its owner
kept them there on the visitor's computer, so the app never offers to move
them. History's date presets follow the visitor's own zone, so elsewhere
they are worked out in the page.

The demo is frozen at the version it was recorded from: the built app and
its recording sit in git under `public/app/`, and a release does not touch
them. Record and build again only when the app's screens have changed
enough that the demo no longer looks like the app, or when a new feature
is worth showing (the Agent screens, for one). The About screen in the
demo shows which version and commit it is.

Record from a clean checkout of a released version: `record.ts` refuses a
backend built from another commit or from uncommitted changes in `src/` or
`src-tauri/`, and `build:demo` refuses to build when `src/` is not as it
was at the recording's commit (the app would ask for answers the
recording does not have), before it clears `public/app/`. It needs a debug
build of the app with the dev bridge, its worker beside it:

```sh
# From the repository root: the worker, staged where the app's build looks
# for it, then the app with the dev bridge.
cargo build --manifest-path src-tauri/Cargo.toml -p vault-worker --bin pathkeep-worker
cp src-tauri/target/debug/pathkeep-worker "src-tauri/binaries/pathkeep-worker-$(rustc --print host-tuple)"
cargo build --manifest-path src-tauri/Cargo.toml --features devtools-bridge

# Then here: about a minute. The recording lands in demo/public/ (ignored
# by git) and build:demo copies it into public/app/ with the app.
bun run record:demo -- --backend ../src-tauri/target/debug/pathkeep-desktop
bun run build:demo && bun run check:demo
```

Commit `public/app/` afterwards: the old build's hashed files go, the new
ones come. Building the demo needs the repository's own dependencies
(`bun install` at the root).

`record.ts` runs everything in a fresh folder under the system's temp
folder (a debug PathKeep window may open meanwhile), rewrites where things
were to a Mac user's own folders, and refuses to write the recording if
anything about the machine is left in it: home folder, user name, host
name, Git e-mail, this checkout. `check.ts` walks the built demo again and
fails on any answer the recording does not have, then makes a visitor's
changes and fails on any error the app shows; `--zone <IANA zone>` runs it
as a visitor elsewhere, where Insights must not offer to move. The walk goes by the English labels; the other
languages open every screen by its address.

## Checks

With `bun run preview` running:

```sh
bun hero-lab/page-check.ts           # every language at desktop, wide desktop and phone, as a visitor loads it
bun hero-lab/page-check.ts --webkit  # the same in WebKit, Safari's engine
bun hero-lab/pageshot.ts /zh-tw/     # full-page and phone screenshots into hero-lab/out/pages/
bun run check:demo                   # the built demo against its recording
../node_modules/.bin/tsc -p demo     # the demo's types, with the app's settings
```

The repository's root Prettier formats the `.ts`, `.css` and Markdown files
here. Root ESLint skips this folder: `astro check` types the site itself
(`src/`), and the lab's Bun scripts in `hero-lab/` are checked by running
them.

## Publishing

On Cloudflare Pages, with the Git repository connected:

- Root directory: `site`
- Build command:
  `bun install --frozen-lockfile && bun run build`
- Build output directory: `dist`
- Environment variable `BUN_VERSION`: the root `package.json`'s
  `packageManager` version.

`public/_headers` sets the response headers: a year's cache for built
files whose names carry their content's hash, and two basic safety
headers everywhere.

## Notes

- `typescript` is held at 6.x: `astro check` needs TypeScript's JavaScript
  API, which the native TypeScript 7 compiler does not ship yet
  ([withastro/roadmap#1321](https://github.com/withastro/roadmap/discussions/1321)).
  Move to 7 once `@astrojs/check` supports it.
- Set `site` in `astro.config.mjs` once the domain is known: the page then
  emits its canonical URL and the `hreflang` alternates between languages.
- The opening card's window shows each space's records page (what people
  keep PathKeep for); the demo's window, until the app loads, the browsing
  home page the demo opens on. The feature wall's browsing pictures are cut
  from the demo itself by `hero-lab/capture-features.ts` (with the preview
  server running), so their words are the app's own in each language; its
  Agent picture is cut from the prototype's Agent insights.
- The windows show screenshots of the v0.6 design prototype, one set per
  language: `hero-lab/capture-screens.ts <url> --hide <selector>` swaps
  their Traditional Chinese for the entries in `hero-lab/screens-text.ts`
  (marked where they carry the app's own wording) and stops on anything
  without one; `export-screens.ts` then writes the WebP files. Capture them
  from the real app once its Agent screens exist; the prototype is
  temporary. For the prototype, this leaves out its notes and its options
  button:

  ```sh
  --hide 'div.shrink-0:has(> .border-dashed:only-child), .rounded-xl.border-dashed.text-muted-foreground, button.fixed.border-dashed'
  ```

- The opening card's frame variable `--p` is written to the element's
  style on every scroll frame, not animated natively. In WebKit (seen in
  Playwright's WebKit 2248), when Motion's native animation drove the
  registered `--p`, the frame did not reliably redraw:
  `getComputedStyle(card).getPropertyValue('--p')` read `1` while the
  computed `clip-path` still held `calc(0 * …)`, so at 1920 px and on
  phones the card stayed full-screen past its foot.
- `hero-lab/page-check.ts --webkit` needs Playwright's own WebKit build
  (`bunx playwright install webkit` at the repository root). Firefox is not
  covered. Two things only WebKit did, both checked there: a message the
  app posts from an animation frame names the landing page itself as its
  `event.source` (so the page knows the demo by origin), and an inert frame
  still scrolls under the wheel (so `.shot__app[inert]` takes no pointer
  events).
