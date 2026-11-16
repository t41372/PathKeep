/**
 * Which version of PathKeep the demo in `public/app/` was recorded from, as
 * its About screen says. The demo stays at that version until it is
 * recorded again (README, The demo), so the page names it beside the window.
 * Read at build time, from the folder the site is built in.
 */
import { readFileSync } from 'node:fs'
import path from 'node:path'

const file = path.resolve('public/app/recording/index.json')
const recording = JSON.parse(readFileSync(file, 'utf8')) as {
  calls: { command: string; response: unknown }[]
}
const info = recording.calls.find((call) => call.command === 'app_build_info')
  ?.response as { version?: string } | undefined
if (!info?.version)
  throw new Error(`${file} has no app_build_info answer to name the demo by.`)

export const demoVersion = info.version
