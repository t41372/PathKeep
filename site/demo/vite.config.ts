/**
 * Builds the real app (`src/`) for the landing page's demo, with the modules
 * that reach the desktop backend swapped for the demo's own: answers
 * recorded from the real backend, replayed in the browser. Nothing in `src/`
 * changes, and the desktop build cannot pick up the swap: it only happens
 * in this build.
 *
 * `--mode record` builds the bundle `record.ts` walks to make the recording:
 * the app's own transport to a running backend, with every answer logged.
 * The demo build never contains that code.
 *
 * Run from the repository root, with the app's own Vite and plugins:
 *   ./node_modules/.bin/vite build --config site/demo/vite.config.ts
 */
import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import type { ConfigEnv, Plugin, UserConfig } from 'vite'

const here = path.dirname(fileURLToPath(import.meta.url))
const repo = path.resolve(here, '../..')
const src = path.join(repo, 'src')
const ipc = (name: string) => path.join(src, 'lib/ipc', `${name}.ts`)

/** Each backend-facing module of the app, and what replaces it in each build. */
const swapsFor = (mode: string) =>
  mode === 'record'
    ? new Map([
        [ipc('bridge'), path.join(here, 'record/bridge.ts')],
        [ipc('search-stream'), path.join(here, 'record/search-stream.ts')],
      ])
    : new Map(
        ['bridge', 'events', 'search-stream'].map((name) => [
          ipc(name),
          path.join(here, 'transport', `${name}.ts`),
        ]),
      )

/**
 * Replaces a module wherever it is imported from, whatever the import path
 * says (`./bridge`, `../ipc/bridge`, `@/lib/ipc/bridge`), by what it resolves
 * to. The demo's own modules still reach the originals.
 */
function swapTransport(swaps: Map<string, string>): Plugin {
  return {
    name: 'pathkeep-demo-transport',
    enforce: 'pre',
    async resolveId(source, importer, options) {
      if (!importer || importer.startsWith(here)) return null
      const resolved = await this.resolve(source, importer, {
        ...options,
        skipSelf: true,
      })
      return (resolved && swaps.get(resolved.id)) ?? null
    },
  }
}

/**
 * The app's stylesheet leaves Tailwind to find class names on its own, from
 * where the build runs: the repository root for the app, this folder here.
 * Point it at the repository, so the demo gets the app's classes.
 */
function scanTheApp(): Plugin {
  const stylesheet = path.join(src, 'index.css')
  return {
    name: 'pathkeep-demo-sources',
    enforce: 'pre',
    transform(code, id) {
      if (id !== stylesheet) return null
      return `${code}\n@source ${JSON.stringify(repo)};\n`
    },
  }
}

/**
 * The demo pairs this checkout's app with the recording's answers, so both
 * must come from one commit. Stops before anything is built (and before the
 * built demo in `site/public/app/` is cleared) when there is no recording
 * here, or when `src/` is not as it was at the recording's commit: an app
 * that asks differently would meet answers that no longer fit. Record again
 * then (README, The demo).
 */
function sameAppAsTheRecording() {
  const file = path.join(here, 'public/recording/index.json')
  if (!existsSync(file))
    throw new Error(
      `No recording at ${file}: record one first (site/README.md, The demo).`,
    )
  const recording = JSON.parse(readFileSync(file, 'utf8')) as {
    calls: { command: string; response: unknown }[]
  }
  const commit = (
    recording.calls.find((call) => call.command === 'app_build_info')
      ?.response as { gitCommitFull?: string } | undefined
  )?.gitCommitFull
  if (!commit)
    throw new Error(`${file} does not say which commit it was recorded from.`)
  const git = (...args: string[]) => {
    const run = spawnSync('git', args, { cwd: repo, encoding: 'utf8' })
    if (run.status !== 0)
      throw new Error(`git ${args.join(' ')}: ${run.stderr.trim()}`)
    return run.stdout.split('\n').filter(Boolean)
  }
  const changed = [
    ...git('diff', '--name-only', commit, '--', 'src'),
    ...git('ls-files', '--others', '--exclude-standard', '--', 'src'),
  ]
  if (changed.length > 0)
    throw new Error(
      `The recording was made from ${commit.slice(0, 8)}, and src/ differs from it (${changed.slice(0, 4).join(', ')}${changed.length > 4 ? ', …' : ''}). Record again (site/README.md, The demo).`,
    )
}

export default ({ mode }: ConfigEnv) => {
  const bridge = process.env.PATHKEEP_DEMO_BRIDGE
  if (mode === 'record' && !bridge)
    throw new Error(
      'Set PATHKEEP_DEMO_BRIDGE to the running backend’s bridge URL.',
    )
  if (mode !== 'record') sameAppAsTheRecording()
  return {
    root: here,
    base: './',
    plugins: [
      swapTransport(swapsFor(mode)),
      scanTheApp(),
      react(),
      tailwindcss(),
    ],
    resolve: { alias: { '@': src } },
    define: {
      // Sends the app down the bridge's path (commands over a transport, no
      // Tauri window): to the real bridge when recording, to the swapped
      // modules in the demo.
      'import.meta.env.VITE_PATHKEEP_DEV_IPC_URL': JSON.stringify(
        mode === 'record' ? bridge : 'demo',
      ),
    },
    // The recording rides along with the demo; a recording run makes it.
    publicDir: mode === 'record' ? false : path.join(here, 'public'),
    build: {
      outDir: path.join(repo, 'site/public/app'),
      emptyOutDir: true,
    },
    server: { fs: { allow: [repo] } },
  } satisfies UserConfig
}
