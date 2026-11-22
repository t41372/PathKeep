/**
 * Prototype-only check: does the prototype carry what the real screens say?
 *
 * For each real component file given, it collects the message keys the file
 * uses, resolves them to zh-TW from the app's message catalog, and looks for
 * each text in the prototype's sources. A text that is missing must be added
 * to the prototype, or listed with a reason in `changed-on-purpose.json` (the
 * design removes or rewords it) or `kept-not-drawn.json` (the app keeps it as
 * it is; the prototype does not draw that failure or in-between state).
 *
 *   bun docs/design/prototype-2026-11-agent-history/parity/check.ts <real file>...
 *   bun …/check.ts --strings <real file>...   (print key → zh-TW, no check)
 *
 * What it proves: no sentence of the real screen is absent from the part of
 * the prototype that stands for it. What it does not: that the control or
 * state the sentence belongs to is drawn, wired or on the right page. It sees
 * keys written as literals anywhere in the file and templates with a literal
 * prefix; a key assembled another way has to be checked by reading.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { messages } from '../../../../src/lib/i18n/messages'

const here = dirname(new URL(import.meta.url).pathname)
const protoSrc = resolve(here, '../src')
const repo = resolve(here, '../../../..')

type Tree = { [key: string]: string | Tree }
const flat = new Map<string, string>()
function walk(tree: Tree, prefix: string) {
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') flat.set(path, value)
    else walk(value, path)
  }
}
walk(messages['zh-TW'] as unknown as Tree, '')

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sources(path)
    return /\.(tsx?|json)$/.test(name) ? [path] : []
  })
}
const protoFiles = sources(protoSrc).map((path) => ({ path: relative(protoSrc, path), text: readFileSync(path, 'utf8').replace(/\s+/g, '') }))

// A real file is looked for only in the part of the prototype that stands for it, so
// a text that sits on some other page does not count.
const scopes: [RegExp, string[]][] = [
  [/features\/backup\/(schedule-|interval|path-actions)/, ['pages/schedule/', 'pages/settings/background-section.tsx', 'pages/browsing-backup/']],
  [/features\/backup\//, ['pages/browsing-backup/']],
  [/features\/settings\/background\//, ['pages/tasks', 'pages/schedule/', 'pages/settings/background-section.tsx', 'pages/settings/browsing-section.tsx']],
  [/features\/settings\/(online-rows|export-row)/, ['pages/settings/browsing-section.tsx']],
  [/features\/settings\/(ai-section|semantic-|provider-|mcp-log-row|rebuild-dialog|use-rebuild|search-tuning-row)/, ['pages/settings/ai-section.tsx', 'pages/settings/ai/']],
  [/features\/settings\/settings-page/, ['pages/settings/settings.tsx']],
  [/features\/settings\//, ['pages/settings/general', 'pages/settings/security', 'pages/settings/storage', 'pages/settings/about', 'pages/settings/pathkeep-kit/']],
  [/features\/onboarding\//, ['pages/onboarding', 'pages/settings/ai/', 'pages/browsing-backup/progress']],
]
function protoFor(file: string) {
  const scope = scopes.find(([pattern]) => pattern.test(file))
  const parts = scope ? protoFiles.filter((f) => scope[1].some((prefix) => f.path.startsWith(prefix))) : protoFiles
  return parts.map((f) => f.text).join('\n')
}

// Two lists excuse a text from appearing in the prototype. `changed-on-purpose`
// is the design: this text goes away or is reworded. `kept-not-drawn` is the
// opposite: the app keeps it exactly as it is and the prototype does not draw
// that state (a failure message, an in-between state). Controls and features
// never belong on the second list.
// Each part of the prototype keeps its own pair of files (`changed-on-purpose.<part>.json`,
// `kept-not-drawn.<part>.json`) so they can be edited side by side.
const listed = readdirSync(here)
  .filter((name) => /^(changed-on-purpose|kept-not-drawn)(\.[a-z-]+)?\.json$/.test(name))
  .flatMap((name) => JSON.parse(readFileSync(join(here, name), 'utf8')) as { key: string; reason: string }[])
const isAllowed = (key: string) => listed.some((entry) => key === entry.key || key.startsWith(`${entry.key}.`))

/** Keys a file names: literals, and every leaf under a template's literal prefix. */
function keysOf(file: string): string[] {
  const text = readFileSync(file, 'utf8')
  const found = new Set<string>()
  for (const match of text.matchAll(/\bt\(\s*(['"`])([A-Za-z0-9_.]+)(\$\{)?/g)) {
    const [, , key, template] = match
    if (template) {
      for (const path of flat.keys()) if (path.startsWith(key)) found.add(path)
    } else if (flat.has(key)) found.add(key)
    else {
      // plural forms and keys used as a prefix of several leaves
      const forms = [...flat.keys()].filter((path) => path === `${key}_one` || path === `${key}_other` || path.startsWith(`${key}.`))
      forms.forEach((path) => found.add(path))
      if (forms.length === 0) found.add(`?? ${key}`)
    }
  }
  // Keys that are not written inside t(…): picked by a condition or kept in a table.
  for (const match of text.matchAll(/['"`]([A-Za-z][A-Za-z0-9]*(?:\.[A-Za-z0-9_]+)+)['"`]/g)) {
    const key = match[1]
    if (flat.has(key)) found.add(key)
    for (const form of [`${key}_one`, `${key}_other`]) if (flat.has(form)) found.add(form)
  }
  return [...found].sort()
}

/** The literal parts of a message, without placeholders, markup or spacing. */
function parts(message: string): string[] {
  return message
    .split(/\{[^}]*\}|<[^>]+>/)
    .map((part) => part.replace(/\s+/g, ''))
    .filter((part) => part.replace(/[\p{P}\p{S}]/gu, '').length >= 2)
}

const args = process.argv.slice(2)
const stringsOnly = args[0] === '--strings'
const files = (stringsOnly ? args.slice(1) : args).map((file) => resolve(file))
let missing = 0
// Texts of three characters or fewer: finding them proves little, a longer text can hide them.
let weak = 0
for (const file of files) {
  const keys = keysOf(file)
  const proto = protoFor(relative(repo, file))
  const lines: string[] = []
  for (const key of keys) {
    const message = flat.get(key)
    if (stringsOnly) {
      lines.push(`  ${key} = ${message ?? '(no such key)'}`)
      continue
    }
    if (message === undefined) {
      lines.push(`  UNRESOLVED ${key}`)
      continue
    }
    if (isAllowed(key)) continue
    const absent = parts(message).filter((part) => !proto.includes(part))
    if (absent.length > 0) {
      missing += 1
      lines.push(`  MISSING ${key} = ${message}`)
    } else if (parts(message).join('').length <= 3) weak += 1
  }
  console.log(`${relative(repo, file)}: ${keys.length} keys${stringsOnly ? '' : `, ${lines.filter((line) => line.includes('MISSING')).length} missing`}`)
  if (lines.length > 0) console.log(lines.join('\n'))
}
if (!stringsOnly) {
  console.log(`\n${weak} texts are three characters or fewer; finding those proves little.`)
  console.log(missing === 0 ? 'Every text is in its part of the prototype or on one of the two lists.' : `\n${missing} texts are neither in the prototype nor on a list.`)
  process.exit(missing === 0 ? 0 : 1)
}
