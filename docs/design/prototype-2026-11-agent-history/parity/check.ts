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
 * It only sees keys written as literals or as a template with a literal
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
const proto = sources(protoSrc)
  .map((path) => readFileSync(path, 'utf8'))
  .join('\n')
  .replace(/\s+/g, '')

// Two lists excuse a text from appearing in the prototype. `changed-on-purpose`
// is the design: this text goes away or is reworded. `kept-not-drawn` is the
// opposite: the app keeps it exactly as it is and the prototype does not draw
// that state (a failure message, an in-between state). Controls and features
// never belong on the second list.
const listed = ['changed-on-purpose.json', 'kept-not-drawn.json'].flatMap(
  (name) => JSON.parse(readFileSync(join(here, name), 'utf8')) as { key: string; reason: string }[],
)
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
for (const file of files) {
  const keys = keysOf(file)
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
    }
  }
  console.log(`${relative(repo, file)}: ${keys.length} keys${stringsOnly ? '' : `, ${lines.filter((line) => line.includes('MISSING')).length} missing`}`)
  if (lines.length > 0) console.log(lines.join('\n'))
}
if (!stringsOnly) {
  console.log(missing === 0 ? '\nEvery text is in the prototype or on one of the two lists.' : `\n${missing} texts are neither in the prototype nor on a list.`)
  process.exit(missing === 0 ? 0 : 1)
}
